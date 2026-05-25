# Code Review Prompt — Copieur (AI-Generated Code)

## Context

You are reviewing AI-generated Rust code for **Copieur**, a Linux clipboard history daemon + egui UI. The workspace has four crates:

- `crates/common` — shared types, IPC wire protocol (bincode frames), `Config`
- `crates/config` — TOML read/write for `~/.config/copieur/config.toml`
- `crates/daemon` — async tokio background process: `ClipboardMonitor`, `HistoryStore`, `FocusTracker`, `ShortcutListener`, `IpcServer`, `PasteExecutor`, `Persistence`, `AutostartManager`
- `crates/ui` — short-lived egui UI process: `CopyslApp`, `CardList`, `Card`, `SearchBar`, `SettingsPanel`, `IpcClient`

The UI exits after one user interaction; the daemon is the single source of truth and runs indefinitely. Communication is over a Unix domain socket using length-prefixed bincode frames.

Your goal: **find real problems**. Do not praise style. Report only issues that matter.

---

## Review Checklist

### 1 — Correctness and Logic

- [ ] Does the code match the documented contract in `docs/detailed-design.md`? Flag any deviation (e.g. wrong sort order, missing deduplication, wrong eviction policy).
- [ ] Are all arms of every `match` handled? AI often adds a catch-all `_ => unreachable!()` that will panic in production.
- [ ] Do edge cases produce correct results?
  - Empty history, history at max capacity, all entries pinned.
  - IPC socket not present when UI starts.
  - Daemon receives a malformed frame (truncated, oversized).
  - Clipboard read returns no data or an error mid-poll.
- [ ] Is deduplication in `HistoryStore::push` correct? The rule is: same content hash → move existing entry to front and update `captured_at`, do NOT create a new id.
- [ ] Is `get_page` sort order correct: pinned entries first (insertion order), then unpinned (newest first)?
- [ ] Does `PasteExecutor` write to the clipboard **before** injecting Ctrl+V? Reversed order is a silent bug.
- [ ] Does `FocusTracker` correctly ignore the Copieur window itself when recording the target window?
- [ ] Is the `paste_delay_ms` field respected on Wayland and ignored on X11/XWayland?

### 2 — Rust Idioms and Ownership

- [ ] No `.unwrap()` or `.expect()` on anything that can fail in production. Allowed only in tests or truly unreachable invariants.
- [ ] No unnecessary `clone()`. Check: are large `Vec<u8>` image payloads cloned where a reference or `Arc` would suffice?
- [ ] `RwLock` / `Mutex` usage: is the guard dropped before any `await` point? (Holding a lock across `.await` deadlocks under tokio.)
- [ ] Are `mpsc` channel sends checked for errors? A `send` to a closed receiver is silently dropped unless handled.
- [ ] Is `VecDeque` used correctly in `HistoryStore`? Random removal (`remove(pos)`) on a `VecDeque` is O(n) — acceptable given max 200 entries, but verify no large allocation reallocs happen on each push.
- [ ] Lifetime annotations: are they the shortest possible? AI tends to over-constrain with `'static` where `'_` is correct.
- [ ] No dead `#[allow(dead_code)]` annotations on public API items — those are silencing real warnings.

### 3 — Error Propagation

- [ ] All `Result`-returning functions propagate errors with `?` or explicitly handle them. AI often adds `let _ = ...` or silently swallows errors.
- [ ] `anyhow::Error` is used in application-layer code (daemon main, UI main). `thiserror` custom errors are used in library-layer code (common, config, crate public APIs). Do not mix them arbitrarily.
- [ ] IPC errors: if the daemon sends `DaemonResponse::Err(msg)`, the UI must surface that message to the user, not silently fall back to a default.
- [ ] Config parse errors: daemon must log the error and fall back to defaults. It must NOT overwrite the corrupt config file.

### 4 — Concurrency and Async

- [ ] Is `HistoryStore` owned by a single async task and accessed only via message passing? No `Arc<Mutex<HistoryStore>>` should exist — that design causes lock contention and is not what the design doc specifies.
- [ ] The `ClipboardMonitor` runs on a dedicated OS thread (arboard is not async-friendly). Verify it uses `std::thread::spawn`, not `tokio::spawn`.
- [ ] The SQLite persistence task runs on a dedicated OS thread (rusqlite is not `Send`). Verify it uses `std::thread::spawn`.
- [ ] `IpcClient::try_recv_push` is called non-blocking every egui frame. Verify it uses `set_nonblocking(true)` on the `UnixStream` and handles `WouldBlock` correctly without logging it as an error.
- [ ] No `tokio::time::sleep` inside the egui render loop. That blocks the frame.
- [ ] Graceful shutdown: when the daemon receives SIGTERM, it must flush the persistence channel before exiting. Check if there is a shutdown signal handler.

### 5 — Platform-Specific Code

- [ ] X11 `XGrabKey`: is the key grabbed on all screens? Is the grab released on daemon shutdown?
- [ ] Wayland `ext-global-shortcuts-v1` fallback sequence is correct: try compositor protocol → try evdev → send desktop notification. Not skipping levels.
- [ ] `PasteExecutor` backend selection is tried in this order on X11: `xdotool` → `XSendEvent` (xcb built-in) → notification fallback. On Wayland: `ydotool` → notification fallback. The XSendEvent path must not be used on native Wayland.
- [ ] `FocusTracker` on X11 subscribes to `_NET_ACTIVE_WINDOW` on the root window. Verify the correct xcb property atom is used.
- [ ] Autostart `.desktop` file path is `~/.config/autostart/copieur.desktop`, not a system-wide path. Verify `dirs::config_dir()` is used, not a hardcoded `/etc` path.
- [ ] Socket path uses `$XDG_RUNTIME_DIR` first, then falls back to `/tmp/copieur-<UID>.sock`. Verify UID is obtained via `nix::unistd::getuid()` or equivalent, not a hardcoded value.

### 6 — IPC Protocol

- [ ] Frame framing: 4-byte little-endian length prefix, then bincode payload. Both sides must agree on endianness.
- [ ] Is there a maximum frame size check on the receiving side? An attacker or a buggy client can send a 4 GB length prefix and cause the daemon to allocate 4 GB before the socket closes. Add a sanity limit (e.g. 64 MB for image payloads).
- [ ] Push notifications (`NewEntry`) are sent by the daemon to all connected UI clients on every `HistoryStore::push`. Verify the daemon does not block on a slow UI client — use a bounded channel or `try_send` to drop the push if the buffer is full.
- [ ] On connection close, the daemon must clean up the client's task and not leak file descriptors.

### 7 — Memory and Performance

- [ ] Image payloads (`Vec<u8>`) are potentially large (several MB). Are they stored by value in `VecDeque<ClipboardEntry>`? If so, verify that deduplication moves by reference (no re-allocation of image bytes on duplicate detection).
- [ ] `content_hash()` is computed on every deduplication check. For image payloads this hashes potentially MBs on every clipboard poll. Is the hash cached or computed once at capture time?
- [ ] egui renders at the display refresh rate. Is `ctx.request_repaint_after(duration)` used to throttle repaints when no user input is received? An idle clipboard window should not burn 100% of one CPU core.
- [ ] `filter_entries` (search) runs on every frame when the search query changes. Verify it does not re-allocate unnecessarily — filtering `Vec<usize>` indices is correct and cheap.

### 8 — Security

- [ ] The IPC socket file permissions must restrict access to the current user only (`0o600`). Verify `std::os::unix::fs::PermissionsExt` is used after binding.
- [ ] No shell command injection: `xdotool` and `ydotool` must be called with `std::process::Command` using separate arguments, never via `sh -c "xdotool " + user_data`.
- [ ] The persistence warning in the settings panel must appear **before** `UpdateConfig` is sent, not after. Verify the UI enforces a confirmation step before toggling `persist_history = true`.
- [ ] History data written to SQLite must not be world-readable. Verify the database file is created with restricted permissions (`0o600`).

### 9 — Test Quality

- [ ] Unit tests cover: `HistoryStore::push` deduplication, eviction when at capacity, `get_page` sort order, `clear(include_pinned=false)`.
- [ ] IPC frame tests (already in `common/src/lib.rs`): verify malformed frames (truncated length, empty payload) are tested.
- [ ] No tests that pass because they never actually assert (`assert!(true)`, empty test bodies, `let _ = result`).
- [ ] AI-generated tests often test only the happy path. Check that error paths (daemon returns `Err(...)`, socket disconnects mid-message) have test coverage.
- [ ] Integration tests (if present): verify they use a real Unix socket, not a mock, since mock/real divergence has caused past incidents.

### 10 — Code Smell Patterns Specific to AI-Generated Code

Flag these patterns explicitly when found:

| Pattern | Why it matters |
|---|---|
| Over-abstraction: traits with a single implementation | Adds indirection with no benefit; remove the trait and use the concrete type |
| Unnecessary `Box<dyn Trait>` where an enum would suffice | Dynamic dispatch overhead and heap allocation for a fixed set of variants |
| Repeated `Arc::clone` inside hot loops | Clone is cheap but repeated `clone` inside a 200ms poll loop is a code smell |
| `todo!()` / `unimplemented!()` left in non-test code | Will panic in production; must be replaced or removed |
| Comments that restate what the code already says | Delete them |
| Deeply nested `if let` chains instead of `?` operator | Harder to read; use `?` or combinators |
| `collect::<Vec<_>>()` followed by immediate iteration | Allocates unnecessarily; chain iterators instead |
| `format!("{}", x)` where `x.to_string()` suffices | Micro-inefficiency; not critical but shows AI verbosity |

---

## Output Format

For each issue found, report:

```
**File**: crates/<crate>/src/<file>.rs:<line>
**Severity**: Critical | Major | Minor
**Category**: (one of the section names above)
**Issue**: one sentence describing the problem
**Fix**: one sentence or short code snippet showing the correct approach
```

Severity definitions:
- **Critical** — causes data loss, panic in production, security vulnerability, or IPC protocol breakage
- **Major** — wrong behavior visible to the user, or a design-doc deviation that will cause a regression later
- **Minor** — code quality, performance, or test coverage gap that does not affect correctness today

Group issues by file. Omit files with no issues.

At the end, add a one-paragraph summary: overall risk level (Low / Medium / High), the single most important issue to fix first, and whether the code is ready to merge.
