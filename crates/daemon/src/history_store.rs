use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, RwLock};

use common::{ClipboardEntry, Config, EntryId};

use crate::persistence::{PersistenceCommand, PersistenceHandle};

pub struct HistoryStore {
    entries: VecDeque<ClipboardEntry>,
    /// Maps content hash → EntryId so duplicate detection is O(1) instead of
    /// recomputing SHA-256 for every existing entry on each push.
    hash_index: HashMap<String, EntryId>,
    id_counter: u64,
    config: Arc<RwLock<Config>>,
    persistence: Option<PersistenceHandle>,
}

impl HistoryStore {
    pub fn new(config: Arc<RwLock<Config>>, persistence: Option<PersistenceHandle>) -> Self {
        Self {
            entries: VecDeque::new(),
            hash_index: HashMap::new(),
            id_counter: 0,
            config,
            persistence,
        }
    }

    pub fn push(&mut self, mut entry: ClipboardEntry) {
        let hash = entry.content_hash();
        let now = std::time::SystemTime::now();

        // O(1) duplicate check via hash index — no per-entry rehashing.
        if let Some(&existing_id) = self.hash_index.get(&hash) {
            // Duplicate found: move it to the front and update timestamp.
            if let Some(pos) = self.entries.iter().position(|e| e.id == existing_id) {
                let mut existing = self.entries.remove(pos).expect("position was valid");
                existing.captured_at = now;
                if let Some(p) = &self.persistence {
                    p.send(PersistenceCommand::Upsert(existing.clone()));
                }
                self.entries.push_front(existing);
            }
        } else {
            // New entry: assign id and prepend.
            entry.id = EntryId(self.id_counter);
            self.id_counter += 1;
            entry.captured_at = now;

            // Evict the last unpinned entry if over capacity.
            let max_entries = self.config.read().map(|c| c.max_entries).unwrap_or(200);
            if self.entries.len() >= max_entries {
                // Find the last (oldest) unpinned entry and remove it.
                if let Some(evict_pos) = self.entries.iter().rposition(|e| !e.pinned) {
                    let evicted = self.entries.remove(evict_pos).expect("position was valid");
                    self.hash_index.remove(&evicted.content_hash());
                    if let Some(p) = &self.persistence {
                        p.send(PersistenceCommand::Delete(evicted.id));
                    }
                }
                // If all are pinned, do not evict — just insert anyway.
            }

            self.hash_index.insert(hash, entry.id);
            if let Some(p) = &self.persistence {
                p.send(PersistenceCommand::Upsert(entry.clone()));
            }
            self.entries.push_front(entry);
        }
    }

    /// Look up a single entry by id without cloning the entire store.
    pub fn get_by_id(&self, id: EntryId) -> Option<ClipboardEntry> {
        self.entries.iter().find(|e| e.id == id).cloned()
    }

    pub fn delete(&mut self, id: EntryId) {
        if let Some(pos) = self.entries.iter().position(|e| e.id == id) {
            let removed = self.entries.remove(pos).expect("position was valid");
            self.hash_index.remove(&removed.content_hash());
            if let Some(p) = &self.persistence {
                p.send(PersistenceCommand::Delete(id));
            }
        }
    }

    pub fn pin(&mut self, id: EntryId) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.pinned = true;
            if let Some(p) = &self.persistence {
                p.send(PersistenceCommand::UpdatePin(id, true));
            }
        }
    }

    pub fn unpin(&mut self, id: EntryId) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.id == id) {
            entry.pinned = false;
            if let Some(p) = &self.persistence {
                p.send(PersistenceCommand::UpdatePin(id, false));
            }
        }
    }

    pub fn clear(&mut self, include_pinned: bool) {
        if include_pinned {
            self.entries.clear();
            self.hash_index.clear();
        } else {
            let removed: Vec<_> = self.entries.iter()
                .filter(|e| !e.pinned)
                .map(|e| e.content_hash())
                .collect();
            self.entries.retain(|e| e.pinned);
            for h in removed {
                self.hash_index.remove(&h);
            }
        }
        if let Some(p) = &self.persistence {
            p.send(PersistenceCommand::Clear(include_pinned));
        }
    }

    pub fn get_page(&self, offset: usize, limit: usize) -> Vec<ClipboardEntry> {
        // Build sorted view: pinned entries first (in VecDeque order),
        // then unpinned entries (in VecDeque order, newest-first).
        let pinned: Vec<&ClipboardEntry> = self.entries.iter().filter(|e| e.pinned).collect();
        let unpinned: Vec<&ClipboardEntry> = self.entries.iter().filter(|e| !e.pinned).collect();

        pinned
            .into_iter()
            .chain(unpinned)
            .skip(offset)
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn load_initial(&mut self, entries: Vec<ClipboardEntry>) {
        self.hash_index = entries.iter()
            .map(|e| (e.content_hash(), e.id))
            .collect();
        self.id_counter = entries.iter()
            .map(|e| e.id.0)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);
        self.entries = VecDeque::from(entries);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::ContentPayload;

    fn make_entry(text: &str) -> ClipboardEntry {
        ClipboardEntry {
            id: EntryId(0),
            payload: ContentPayload::PlainText(text.to_string()),
            captured_at: std::time::SystemTime::now(),
            pinned: false,
        }
    }

    fn default_store() -> HistoryStore {
        let config = Arc::new(RwLock::new(Config::default()));
        HistoryStore::new(config, None)
    }

    fn store_with_max(max_entries: usize) -> HistoryStore {
        let config = Arc::new(RwLock::new(Config {
            max_entries,
            ..Config::default()
        }));
        HistoryStore::new(config, None)
    }

    #[test]
    fn push_deduplicates() {
        let mut store = default_store();
        store.push(make_entry("hello"));
        let first_id = store.get_page(0, 1)[0].id;

        store.push(make_entry("hello"));
        let page = store.get_page(0, 100);

        assert_eq!(page.len(), 1, "duplicate should not create a second entry");
        assert_eq!(page[0].id, first_id, "id should be preserved after dedup");
    }

    #[test]
    fn push_evicts_oldest_unpinned() {
        let mut store = store_with_max(2);
        store.push(make_entry("a"));
        store.push(make_entry("b"));
        store.push(make_entry("c"));

        let page = store.get_page(0, 100);
        assert_eq!(page.len(), 2, "should only keep max_entries entries");

        let texts: Vec<&str> = page
            .iter()
            .map(|e| match &e.payload {
                ContentPayload::PlainText(t) => t.as_str(),
                _ => "",
            })
            .collect();
        assert!(
            !texts.contains(&"a"),
            "\"a\" (oldest) should have been evicted"
        );
    }

    #[test]
    fn push_never_evicts_pinned() {
        let mut store = store_with_max(2);
        store.push(make_entry("a"));
        let id_a = store.get_page(0, 1)[0].id;
        store.pin(id_a);

        store.push(make_entry("b"));
        store.push(make_entry("c"));

        let page = store.get_page(0, 100);
        let has_a = page.iter().any(|e| e.id == id_a);
        assert!(has_a, "pinned entry \"a\" should never be evicted");
    }

    #[test]
    fn get_page_ordering() {
        let mut store = default_store();
        store.push(make_entry("b")); // pushed first, older
        store.push(make_entry("a")); // pushed second, newer — newest unpinned

        // Pin "b" (it is at position 1 in the deque since "a" is at front)
        let id_b = store
            .get_page(0, 100)
            .iter()
            .find(|e| matches!(&e.payload, ContentPayload::PlainText(t) if t == "b"))
            .expect("b should exist")
            .id;
        store.pin(id_b);

        let page = store.get_page(0, 100);
        assert_eq!(page.len(), 2);
        // Pinned "b" should come first.
        assert_eq!(page[0].id, id_b, "pinned entry should be first");
        assert!(page[0].pinned);
        assert!(!page[1].pinned);
    }

    #[test]
    fn clear_false_retains_pinned() {
        let mut store = default_store();
        store.push(make_entry("a"));
        store.push(make_entry("b"));
        let id_b = store.get_page(0, 1)[0].id; // "b" is at front (newest)
        store.pin(id_b);

        store.clear(false);

        let page = store.get_page(0, 100);
        assert_eq!(page.len(), 1, "only the pinned entry should remain");
        assert_eq!(page[0].id, id_b);
    }

    #[test]
    fn clear_true_removes_all() {
        let mut store = default_store();
        store.push(make_entry("a"));
        store.push(make_entry("b"));
        let id_b = store.get_page(0, 1)[0].id;
        store.pin(id_b);

        store.clear(true);

        assert!(
            store.get_page(0, 100).is_empty(),
            "all entries should be removed"
        );
    }

    #[test]
    fn delete_removes_by_id() {
        let mut store = default_store();
        store.push(make_entry("a")); // id 0
        store.push(make_entry("b")); // id 1

        let id_a = store
            .get_page(0, 100)
            .iter()
            .find(|e| matches!(&e.payload, ContentPayload::PlainText(t) if t == "a"))
            .expect("a should exist")
            .id;

        store.delete(id_a);

        let page = store.get_page(0, 100);
        assert_eq!(page.len(), 1, "only one entry should remain");
        assert!(
            page.iter().all(|e| e.id != id_a),
            "deleted entry should not be present"
        );
    }
}
