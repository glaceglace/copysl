use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::UNIX_EPOCH;

use anyhow::Result;
use common::{ClipboardEntry, ContentPayload, EntryId};
use rusqlite::{params, Connection};

pub fn db_path() -> PathBuf {
    dirs::data_local_dir()
        .expect("Could not determine data directory")
        .join("copysl")
        .join("history.db")
}

#[allow(dead_code)]
pub enum PersistenceCommand {
    Upsert(ClipboardEntry),
    Delete(EntryId),
    UpdatePin(EntryId, bool),
    Clear(bool),
    LoadAll(mpsc::SyncSender<Vec<ClipboardEntry>>),
    Shutdown,
}

#[derive(Clone)]
pub struct PersistenceHandle {
    sender: mpsc::Sender<PersistenceCommand>,
}

impl PersistenceHandle {
    pub fn open() -> Result<(PersistenceHandle, thread::JoinHandle<()>)> {
        Self::open_at(db_path())
    }

    pub fn open_at(path: PathBuf) -> Result<(PersistenceHandle, thread::JoinHandle<()>)> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Self::open_with_connection(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<(PersistenceHandle, thread::JoinHandle<()>)> {
        Self::open_with_connection(Connection::open_in_memory()?)
    }

    fn open_with_connection(
        conn: Connection,
    ) -> Result<(PersistenceHandle, thread::JoinHandle<()>)> {
        init_db(&conn)?;
        let (tx, rx) = mpsc::channel::<PersistenceCommand>();
        let handle = thread::spawn(move || {
            run_persistence_task(conn, rx);
        });
        Ok((PersistenceHandle { sender: tx }, handle))
    }

    pub fn send(&self, cmd: PersistenceCommand) {
        let _ = self.sender.send(cmd);
    }

    pub fn load_all(&self) -> Vec<ClipboardEntry> {
        let (tx, rx) = mpsc::sync_channel(1);
        let _ = self.sender.send(PersistenceCommand::LoadAll(tx));
        rx.recv().unwrap_or_default()
    }
}

fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch("PRAGMA journal_mode=WAL;")?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS entries (
            id          INTEGER PRIMARY KEY,
            content_type TEXT NOT NULL,
            payload     BLOB NOT NULL,
            captured_at INTEGER NOT NULL,
            pinned      INTEGER NOT NULL DEFAULT 0,
            content_hash TEXT NOT NULL
        );",
    )?;
    Ok(())
}

fn run_persistence_task(conn: Connection, rx: mpsc::Receiver<PersistenceCommand>) {
    for cmd in rx {
        match cmd {
            PersistenceCommand::Upsert(entry) => {
                let _ = upsert_entry(&conn, &entry);
            }
            PersistenceCommand::Delete(id) => {
                let _ = conn.execute("DELETE FROM entries WHERE id = ?1", params![id.0]);
            }
            PersistenceCommand::UpdatePin(id, pinned) => {
                let _ = conn.execute(
                    "UPDATE entries SET pinned = ?1 WHERE id = ?2",
                    params![pinned as i64, id.0],
                );
            }
            PersistenceCommand::Clear(include_pinned) => {
                if include_pinned {
                    let _ = conn.execute("DELETE FROM entries", []);
                } else {
                    let _ = conn.execute("DELETE FROM entries WHERE pinned = 0", []);
                }
            }
            PersistenceCommand::LoadAll(reply) => {
                let entries = load_all_entries(&conn).unwrap_or_default();
                let _ = reply.send(entries);
            }
            PersistenceCommand::Shutdown => break,
        }
    }
}

fn upsert_entry(conn: &Connection, entry: &ClipboardEntry) -> Result<()> {
    let content_type = match &entry.payload {
        ContentPayload::PlainText(_) => "text",
        ContentPayload::RichText { .. } => "richtext",
        ContentPayload::Image { .. } => "image",
    };
    let payload_bytes = bincode::serialize(&entry.payload)?;
    let captured_at_ms = entry
        .captured_at
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;
    let hash = entry.content_hash();
    conn.execute(
        "INSERT OR REPLACE INTO entries (id, content_type, payload, captured_at, pinned, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            entry.id.0 as i64,
            content_type,
            payload_bytes,
            captured_at_ms,
            entry.pinned as i64,
            hash,
        ],
    )?;
    Ok(())
}

fn load_all_entries(conn: &Connection) -> Result<Vec<ClipboardEntry>> {
    let mut stmt = conn.prepare(
        "SELECT id, payload, captured_at, pinned FROM entries ORDER BY pinned DESC, captured_at DESC",
    )?;
    let entries = stmt
        .query_map([], |row| {
            let id: i64 = row.get(0)?;
            let payload_bytes: Vec<u8> = row.get(1)?;
            let captured_at_ms: i64 = row.get(2)?;
            let pinned: i64 = row.get(3)?;
            Ok((id, payload_bytes, captured_at_ms, pinned))
        })?
        .filter_map(|r| r.ok())
        .filter_map(|(id, payload_bytes, captured_at_ms, pinned)| {
            let payload: ContentPayload = bincode::deserialize(&payload_bytes).ok()?;
            let captured_at =
                UNIX_EPOCH + std::time::Duration::from_millis(captured_at_ms as u64);
            Some(ClipboardEntry {
                id: EntryId(id as u64),
                payload,
                captured_at,
                pinned: pinned != 0,
            })
        })
        .collect();
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::ContentPayload;
    use std::time::SystemTime;
    use tempfile::tempdir;

    fn make_entry(id: u64, text: &str, pinned: bool) -> ClipboardEntry {
        ClipboardEntry {
            id: EntryId(id),
            payload: ContentPayload::PlainText(text.to_string()),
            captured_at: SystemTime::now(),
            pinned,
        }
    }

    fn open_test_db() -> (PersistenceHandle, std::thread::JoinHandle<()>) {
        PersistenceHandle::open_in_memory().expect("in-memory DB failed")
    }

    #[test]
    fn upsert_then_load_all() {
        let (handle, thread) = open_test_db();
        let entry = make_entry(1, "hello", false);
        handle.send(PersistenceCommand::Upsert(entry.clone()));
        let entries = handle.load_all();
        handle.send(PersistenceCommand::Shutdown);
        thread.join().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, entry.id);
    }

    #[test]
    fn delete_removes_entry() {
        let (handle, thread) = open_test_db();
        handle.send(PersistenceCommand::Upsert(make_entry(1, "hello", false)));
        handle.send(PersistenceCommand::Delete(EntryId(1)));
        let entries = handle.load_all();
        handle.send(PersistenceCommand::Shutdown);
        thread.join().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn clear_false_retains_pinned() {
        let (handle, thread) = open_test_db();
        handle.send(PersistenceCommand::Upsert(make_entry(1, "pinned", true)));
        handle.send(PersistenceCommand::Upsert(make_entry(2, "unpinned", false)));
        handle.send(PersistenceCommand::Clear(false));
        let entries = handle.load_all();
        handle.send(PersistenceCommand::Shutdown);
        thread.join().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].pinned);
    }

    #[test]
    fn clear_true_removes_all() {
        let (handle, thread) = open_test_db();
        handle.send(PersistenceCommand::Upsert(make_entry(1, "pinned", true)));
        handle.send(PersistenceCommand::Upsert(make_entry(2, "unpinned", false)));
        handle.send(PersistenceCommand::Clear(true));
        let entries = handle.load_all();
        handle.send(PersistenceCommand::Shutdown);
        thread.join().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn update_pin_changes_flag() {
        let (handle, thread) = open_test_db();
        handle.send(PersistenceCommand::Upsert(make_entry(1, "hello", false)));
        handle.send(PersistenceCommand::UpdatePin(EntryId(1), true));
        let entries = handle.load_all();
        handle.send(PersistenceCommand::Shutdown);
        thread.join().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].pinned);
    }

    // open_at creates parent directories that do not yet exist
    #[test]
    fn open_at_creates_missing_parent_dirs() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a").join("b").join("c").join("history.db");
        assert!(!nested.parent().unwrap().exists());
        let (handle, thread) = PersistenceHandle::open_at(nested.clone()).unwrap();
        handle.send(PersistenceCommand::Shutdown);
        thread.join().unwrap();
        assert!(nested.exists());
    }

    // open_at on an already-existing directory does not fail
    #[test]
    fn open_at_existing_dir_succeeds() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.db");
        let (handle, thread) = PersistenceHandle::open_at(path).unwrap();
        handle.send(PersistenceCommand::Shutdown);
        thread.join().unwrap();
    }

    // data survives close + reopen (round-trip through the real SQLite file)
    #[test]
    fn data_survives_reopen() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.db");

        {
            let (handle, thread) = PersistenceHandle::open_at(path.clone()).unwrap();
            handle.send(PersistenceCommand::Upsert(make_entry(1, "persistent", false)));
            let _ = handle.load_all(); // flush: LoadAll is synchronous
            handle.send(PersistenceCommand::Shutdown);
            thread.join().unwrap();
        }

        let (handle, thread) = PersistenceHandle::open_at(path).unwrap();
        let entries = handle.load_all();
        handle.send(PersistenceCommand::Shutdown);
        thread.join().unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, EntryId(1));
        assert!(matches!(&entries[0].payload, ContentPayload::PlainText(t) if t == "persistent"));
    }
}
