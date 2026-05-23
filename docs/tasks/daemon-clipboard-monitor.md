# Module: `daemon::clipboard_monitor`

File: `crates/daemon/src/clipboard_monitor.rs`
Detects clipboard changes every 200 ms and forwards new entries to `HistoryStore`.

---

## Tasks

### Dependencies
- [ ] Add `arboard` to daemon `Cargo.toml`
- [ ] Add `sha2` (or reuse from `common`) for content hashing

### Thread Setup
- [ ] Implement `ClipboardMonitor::spawn(tx: mpsc::Sender<ClipboardEntry>) -> JoinHandle<()>`
- [ ] Run on a dedicated OS thread (not async) since `arboard::Clipboard` is not `Send`-safe across await points
- [ ] Use a loop with `std::thread::sleep(Duration::from_millis(200))` between polls

### Clipboard Reading
- [ ] On each poll, attempt to read in priority order: Image → HTML (RichText) → PlainText
- [ ] Construct `ContentPayload::Image` if image data is available via `arboard`
- [ ] Construct `ContentPayload::RichText` if HTML format is available
- [ ] Construct `ContentPayload::PlainText` for plain text fallback
- [ ] Handle the case where clipboard is empty (skip without error)

### Change Detection
- [ ] Compute SHA-256 hash of the raw content bytes after each successful read
- [ ] Compare with last captured hash; skip if identical
- [ ] Update stored hash only when a new entry is sent

### Entry Construction
- [ ] Assign a placeholder `EntryId(0)` — the real ID is assigned by `HistoryStore`
- [ ] Set `captured_at = SystemTime::now()`
- [ ] Set `pinned = false`
- [ ] Send constructed `ClipboardEntry` via `tx.send(entry)`

### Error Handling
- [ ] Log clipboard read errors (e.g. no owner) and continue the poll loop; do not crash
- [ ] If the sender is disconnected, exit the thread cleanly

### Tests
- [ ] Unit test: two identical clipboard reads do not produce two sends
- [ ] Unit test: changing content between polls produces a send
