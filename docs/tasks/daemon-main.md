# Module: `daemon::main` — Daemon Entry Point

File: `crates/daemon/src/main.rs`
Wires all daemon subsystems together and blocks on the async runtime.

---

## Tasks

### Crate Setup
- [ ] Create `crates/daemon/` with `Cargo.toml` (binary crate, name `copieur-daemon`)
- [ ] Add `tokio` runtime dependency with `rt-multi-thread`, `macros` features
- [ ] Add all internal crate dependencies: `common`, `config`

### `daemon_main()` Function
- [ ] Load `Config` via `config::load()`
- [ ] Initialize `PersistenceHandle` if `config.persist_history = true`; load history from DB into `HistoryStore`
- [ ] Initialize `HistoryStore` with loaded config and optional persistence handle
- [ ] Initialize `FocusTracker` (platform-selected), start it
- [ ] Bind `IpcServer` on the computed socket path
- [ ] Start `ClipboardMonitor` thread, wire its sender to `HistoryStore`
- [ ] Initialize `PasteExecutor` with arboard + focus tracker + selected paste backend
- [ ] Initialize `ShortcutListener` (platform-selected), start it with the UI-spawn callback
- [ ] Initialize `AutostartManager`; call `enable()` if `config.autostart = true` on first run (only if `.desktop` file not already present)
- [ ] Log "Copieur daemon started" with socket path
- [ ] Block on tokio runtime until SIGTERM / SIGINT

### Signal Handling
- [ ] Register SIGTERM and SIGINT handlers using `tokio::signal`
- [ ] On signal: log "Shutting down", gracefully stop subsystems (close socket, join threads), then exit

### Environment Variable Export
- [ ] After binding the socket, store the socket path in memory so `ShortcutListener` can pass it as `COPIEUR_SOCKET` when spawning the UI

### Tests
- [ ] Integration smoke test: daemon starts, socket is reachable, a basic `GetHistory` returns an empty list
