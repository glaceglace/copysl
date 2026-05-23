# Module: `daemon::persistence`

File: `crates/daemon/src/persistence.rs`
Optional SQLite-backed history storage. Activated when `Config::persist_history = true`.

---

## Tasks

### Dependencies
- [ ] Add `rusqlite` with feature `bundled` to daemon `Cargo.toml`
- [ ] Add `dirs` for resolving `~/.local/share/copieur/`

### Database Path
- [ ] Implement `db_path() -> PathBuf` returning `~/.local/share/copieur/history.db`
- [ ] Create parent directories if they do not exist

### Schema Initialization
- [ ] On open, run `CREATE TABLE IF NOT EXISTS entries (...)` with columns:
  - `id INTEGER PRIMARY KEY`
  - `content_type TEXT NOT NULL` — `'text'` | `'richtext'` | `'image'`
  - `payload BLOB NOT NULL` — bincode-encoded `ContentPayload`
  - `captured_at INTEGER NOT NULL` — Unix timestamp ms
  - `pinned INTEGER NOT NULL DEFAULT 0`
  - `content_hash TEXT NOT NULL`
- [ ] Enable WAL mode: `PRAGMA journal_mode=WAL`

### `PersistenceHandle` and Task
- [ ] Define `PersistenceCommand` enum: `Upsert(ClipboardEntry)`, `Delete(EntryId)`, `UpdatePin(EntryId, bool)`, `Clear(bool)`, `LoadAll(oneshot::Sender<Vec<ClipboardEntry>>)`
- [ ] Implement `PersistenceHandle` as an mpsc sender of `PersistenceCommand`
- [ ] Run `PersistenceTask` on a **single dedicated thread** (`std::thread::spawn`), since `rusqlite::Connection` is not `Send`
- [ ] Process commands from the receiver sequentially

### Write Operations
- [ ] `Upsert`: `INSERT OR REPLACE INTO entries (...)` using the entry's id and fields
- [ ] `Delete`: `DELETE FROM entries WHERE id = ?`
- [ ] `UpdatePin`: `UPDATE entries SET pinned = ? WHERE id = ?`
- [ ] `Clear(false)`: `DELETE FROM entries WHERE pinned = 0`
- [ ] `Clear(true)`: `DELETE FROM entries`

### Read Operation (startup)
- [ ] `LoadAll`: `SELECT * FROM entries ORDER BY pinned DESC, captured_at DESC`
- [ ] Deserialize `payload` blob with bincode into `ContentPayload`
- [ ] Reconstruct `ClipboardEntry` and send back via the oneshot channel

### Activation / Deactivation
- [ ] Implement `PersistenceHandle::open() -> Result<PersistenceHandle>` — creates thread and channel
- [ ] Implement `PersistenceHandle::close()` — sends a shutdown signal, joins thread
- [ ] On deactivation (user turns off persistence), do NOT delete the database file; just stop writing

### Error Handling
- [ ] Log SQLite write errors without crashing
- [ ] In-memory history continues to work even if persistence fails

### Tests
- [ ] Unit test: `Upsert` then `LoadAll` returns the same entry
- [ ] Unit test: `Delete` removes entry by id
- [ ] Unit test: `Clear(false)` retains pinned entries
- [ ] Unit test: `Clear(true)` removes all entries
- [ ] Unit test: `UpdatePin` changes pinned flag persisted to disk
