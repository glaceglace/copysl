# Module: `daemon::history_store`

File: `crates/daemon/src/history_store.rs`
Authoritative in-memory store of clipboard history. All reads and writes go through this module.

---

## Tasks

### Data Structure
- [ ] Define `HistoryStore` struct:
  - `entries: VecDeque<ClipboardEntry>`
  - `id_counter: u64`
  - `config: Arc<RwLock<Config>>`
  - `persistence: Option<PersistenceHandle>`
- [ ] Implement `HistoryStore::new(config, persistence) -> Self`

### ID Assignment
- [ ] On each new entry accepted by `push`, assign `id = id_counter`, then increment `id_counter`

### `push(entry)` — Insert with deduplication and eviction
- [ ] Compute content hash of incoming entry
- [ ] Search existing entries for a matching content hash
- [ ] If duplicate found: move that entry to the front (index 0), update its `captured_at`; do not insert a new entry
- [ ] If no duplicate: prepend the new entry at index 0
- [ ] After insert, count unpinned entries; if `entries.len() > max_entries`, remove the last unpinned entry (oldest unpinned)
- [ ] If persistence is enabled, call `PersistenceHandle::upsert(entry)`

### `delete(id)`
- [ ] Find entry by `id`, remove it from `VecDeque`
- [ ] If persistence is enabled, call `PersistenceHandle::delete(id)`

### `pin(id)` / `unpin(id)`
- [ ] Find entry by `id`, set `pinned = true / false`
- [ ] If persistence is enabled, call `PersistenceHandle::update_pin(id, pinned)`

### `clear(include_pinned: bool)`
- [ ] If `include_pinned = false`: retain only `pinned = true` entries
- [ ] If `include_pinned = true`: drain all entries
- [ ] If persistence is enabled, call `PersistenceHandle::clear(include_pinned)`

### `get_page(offset: usize, limit: usize) -> Vec<ClipboardEntry>`
- [ ] Produce sorted view: pinned entries first (in insertion order), then unpinned (insertion order, newest first)
- [ ] Apply `offset` and `limit` to the sorted result
- [ ] Return cloned entries (do not mutate the VecDeque order)

### Thread Safety
- [ ] Ensure `HistoryStore` is owned by a single async task
- [ ] Expose a `HistoryStoreHandle` (mpsc sender or similar) for other modules to send commands
- [ ] Process all commands sequentially in the owning task

### Tests
- [ ] Unit test: `push` deduplicates — same content, entry moves to front, count unchanged
- [ ] Unit test: `push` evicts oldest unpinned when at capacity; pinned entries are never evicted
- [ ] Unit test: `get_page` ordering — pinned before unpinned, newest-first within each group
- [ ] Unit test: `clear(false)` retains pinned entries
- [ ] Unit test: `clear(true)` removes all entries
- [ ] Unit test: `delete` removes correct entry by id
