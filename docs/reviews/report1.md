# Code Review Report — Copieur
_Date: 2026-05-25_

---

## crates/common/src/lib.rs

**File**: `crates/common/src/lib.rs:204-206`
**Severity**: Critical
**Category**: IPC Protocol
**Issue**: `read_frame` allocates `vec![0u8; len]` with no upper bound — a client that sends a 4-byte length prefix of `0xFFFFFFFF` causes a 4 GB allocation attempt before the read fails.
**Fix**: Reject frames above a sanity limit before allocating: `if len > 64 * 1024 * 1024 { return Err(...); }`

**File**: `crates/common/src/lib.rs` (Tests)
**Severity**: Minor
**Category**: Test Quality
**Issue**: No tests for malformed IPC frames — truncated 3-byte length prefix, oversized-length payload that exceeds the buffer, or zero-length payload — despite the checklist explicitly requiring them.
**Fix**: Add `#[test] fn read_frame_truncated_length_returns_err()` feeding 3 bytes to `read_frame` and asserting `Err`, and `fn read_frame_oversized_rejects()` once the max-size guard is added.

---

## crates/daemon/src/ipc_server.rs

**File**: `crates/daemon/src/ipc_server.rs:75-81`
**Severity**: Critical
**Category**: Security
**Issue**: The Unix socket is bound with no explicit permission restriction; the default mode (subject to umask, often 0o755) lets any local user connect and read full clipboard history or trigger paste.
**Fix**: After `UnixListener::bind(&path)?`, apply `std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;`

**File**: `crates/daemon/src/ipc_server.rs:136-138`
**Severity**: Critical
**Category**: IPC Protocol
**Issue**: Same missing frame-size guard as in `common::read_frame` — a client can send a 4 GB length prefix and OOM the daemon.
**Fix**: Same as above — reject `len > 64_000_000` before allocating.

**File**: `crates/daemon/src/ipc_server.rs:21-24`
**Severity**: Major
**Category**: Platform-Specific Code
**Issue**: `std::env::var("UID")` is shell-specific (bash sets it; it is absent in other execution contexts and trivially spoofed via `UID=0 ./copieur`). Falls back to `1000` as hardcoded UID.
**Fix**: Replace with `nix::unistd::getuid().as_raw()` (or `unsafe { libc::getuid() }`).

**File**: `crates/daemon/src/ipc_server.rs:85-88`
**Severity**: Major
**Category**: Concurrency and Async
**Issue**: `store: Arc<Mutex<HistoryStore>>` is passed into every IPC connection task, violating the design-doc requirement that `HistoryStore` is owned by a single task with message-passing access.
**Fix**: Move `HistoryStore` into its own `tokio::spawn` task and communicate via `mpsc` channels per the design doc.

**File**: `crates/daemon/src/ipc_server.rs:247-249`
**Severity**: Major
**Category**: Memory and Performance
**Issue**: `s.get_page(0, usize::MAX)` allocates a full `Vec<ClipboardEntry>` — cloning all image payloads — while holding the async Mutex, on every `PasteEntry` request. With 200 multi-MB image entries this blocks all other store access for the duration of the heap allocation.
**Fix**: Add a `get_by_id(id: EntryId) -> Option<ClipboardEntry>` method to `HistoryStore` that does a direct O(n) scan without cloning the entire store.

---

## crates/daemon/src/lib.rs

**File**: `crates/daemon/src/lib.rs:38`
**Severity**: Critical
**Category**: Concurrency and Async
**Issue**: `Ok((handle, _thread)) =>` — the `JoinHandle` for the persistence thread is immediately dropped (not stored). On SIGTERM, the daemon exits without joining the thread; any pending `Upsert`/`Delete` commands buffered in the mpsc channel may never be written to SQLite.
**Fix**: Store the `JoinHandle`, send `PersistenceCommand::Shutdown` before exiting, then `thread.join().ok()` to drain remaining commands.

**File**: `crates/daemon/src/lib.rs:188-190`
**Severity**: Minor
**Category**: Concurrency and Async
**Issue**: `server_task.abort()` terminates the IPC server without first waiting for in-flight requests to complete. A client mid-request receives an abrupt connection close.
**Fix**: Use a `CancellationToken` or send a shutdown message and `server_task.await` instead of `abort()`.

---

## crates/daemon/src/history_store.rs

**File**: `crates/daemon/src/history_store.rs:30`
**Severity**: Major
**Category**: Memory and Performance
**Issue**: `self.entries.iter().position(|e| e.content_hash() == hash)` recomputes SHA-256 of every existing entry's payload on every clipboard-change event. For 200 image entries of ~2 MB each, that is 400 MB hashed every 200 ms — a CPU-bound hot loop.
**Fix**: Cache the hash as a field in `ClipboardEntry` (computed once at capture time) and compare `e.cached_hash == hash`. Alternatively maintain a `HashMap<String, EntryId>` as a secondary index in `HistoryStore`.

**File**: `crates/daemon/src/history_store.rs:34`
**Severity**: Minor
**Category**: Rust Idioms and Ownership
**Issue**: `push_front(existing.clone())` clones the full payload (potentially several MB of image bytes) only to immediately pass the original to `PersistenceCommand::Upsert(existing)`. The clone is the one stored; the original is sent to persistence.
**Fix**: Swap the order: `p.send(PersistenceCommand::Upsert(existing.clone()))` and `self.entries.push_front(existing)` to eliminate the heap allocation of image bytes.

**File**: `crates/daemon/src/history_store.rs:117`
**Severity**: Minor
**Category**: Code Smell
**Issue**: `#[allow(dead_code)]` on the `pub fn load_initial` method — silences a real warning on a public API item.
**Fix**: Remove the attribute; if the function is unused at compile time, that is a sign it should either be called or deleted.

---

## crates/daemon/src/persistence.rs

**File**: `crates/daemon/src/persistence.rs:88-105`
**Severity**: Major
**Category**: Error Propagation
**Issue**: Every write operation (`Upsert`, `Delete`, `UpdatePin`, `Clear`) wraps its result in `let _ = ...`, silently discarding all SQLite errors. History data can be permanently lost with no log message.
**Fix**: Replace `let _ = upsert_entry(...)` with `if let Err(e) = upsert_entry(...) { log::error!("Persistence upsert failed: {e}"); }`.

**File**: `crates/daemon/src/persistence.rs:40-41`
**Severity**: Major
**Category**: Security
**Issue**: `Connection::open(path)?` creates the SQLite file with default permissions (typically 0o644), making clipboard history (passwords, tokens, etc.) world-readable.
**Fix**: After creating the file, call `std::fs::set_permissions(&path, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;`

**File**: `crates/daemon/src/persistence.rs:17`
**Severity**: Minor
**Category**: Code Smell
**Issue**: `#[allow(dead_code)]` on `pub enum PersistenceCommand` suppresses a legitimate warning on a public item.
**Fix**: Remove the attribute; if it triggers, delete unused variants rather than silencing the warning.

---

## crates/daemon/src/focus_tracker/x11.rs

**File**: `crates/daemon/src/focus_tracker/x11.rs:43`
**Severity**: Minor
**Category**: Rust Idioms and Ownership
**Issue**: `setup.roots().nth(screen_num as usize).unwrap()` in a background thread — if the screen number returned by XCB is out of range, the focus-tracker thread panics silently (no log, daemon continues running without focus tracking).
**Fix**: Replace `.unwrap()` with `let Some(screen) = setup.roots().nth(screen_num as usize) else { return; };` and log the error.

---

## crates/ui/src/app.rs

**File**: `crates/ui/src/app.rs:218`
**Severity**: Minor
**Category**: Memory and Performance
**Issue**: `ctx.request_repaint()` schedules an immediate repaint every frame, causing the idle clipboard window to spin at the display refresh rate (~60 fps) and burn a full CPU core.
**Fix**: Replace with `ctx.request_repaint_after(std::time::Duration::from_millis(50))` when there is no pending animation or active input.

---

## Summary

**Overall Risk: High.**

The most important issue to fix first is the missing frame-size guard in both `common::read_frame` (`crates/common/src/lib.rs:204`) and `handle_connection` (`crates/daemon/src/ipc_server.rs:136`) — an unauthenticated local process can OOM the daemon with a single malformed packet. The IPC socket permissions bug is a close second, as without it any local user can read clipboard history including passwords. The persistence thread join-handle drop is the third Critical issue: it means clipboard history silently fails to persist under any abrupt shutdown. After those three, the DB file permissions and the silent swallowing of all SQLite errors should be addressed before any data-at-rest feature is considered safe.

**The code is not ready to merge in its current state** — the three Critical issues (OOM via IPC, socket open to all local users, persistence data loss on shutdown) each represent independent production-impact bugs.

### Issue Summary Table

| Severity | Count | Files affected |
|---|---|---|
| Critical | 3 | common/lib.rs, ipc_server.rs, lib.rs |
| Major | 5 | ipc_server.rs, history_store.rs, persistence.rs |
| Minor | 7 | lib.rs, history_store.rs, persistence.rs, x11.rs, app.rs, common/lib.rs |
