# Module: `daemon::ipc_server`

File: `crates/daemon/src/ipc_server.rs`
Accepts IPC connections from UI processes and routes requests to the appropriate subsystems.

---

## Tasks

### Dependencies
- [ ] Add `tokio` with features `net`, `io-util`, `sync`, `rt-multi-thread` to daemon `Cargo.toml`

### Socket Path
- [ ] Implement `socket_path() -> PathBuf`:
  - [ ] Use `$XDG_RUNTIME_DIR/copieur.sock` if `XDG_RUNTIME_DIR` is set
  - [ ] Fall back to `/tmp/copieur-{uid}.sock`
- [ ] Remove stale socket file on daemon start before binding

### Server Startup
- [ ] Implement `IpcServer::bind(path) -> Result<IpcServer>` using `tokio::net::UnixListener`
- [ ] Expose `socket_path` for the daemon to pass to spawned UI processes via env var

### Connection Handling
- [ ] Accept connections in a loop: `listener.accept().await`
- [ ] Spawn a new async task per connection: `tokio::spawn(handle_connection(stream, handles))`
- [ ] Pass `HistoryStoreHandle`, `PasteExecutorHandle`, `ConfigHandle`, `AutostartHandle` into each connection task

### Request Routing (`handle_connection`)
- [ ] Read length-prefixed frames using `common::read_frame`
- [ ] Decode `DaemonRequest` with bincode
- [ ] Route each request:
  - [ ] `GetHistory` → call `HistoryStore::get_page(offset, limit)`, respond with `DaemonResponse::History`
  - [ ] `PasteEntry` → call `PasteExecutor::paste(id)`, respond `Ok` or `Err`
  - [ ] `DeleteEntry` → call `HistoryStore::delete(id)`, respond `Ok` or `Err`
  - [ ] `PinEntry` → call `HistoryStore::pin(id)`, respond `Ok` or `Err`
  - [ ] `UnpinEntry` → call `HistoryStore::unpin(id)`, respond `Ok` or `Err`
  - [ ] `ClearHistory` → call `HistoryStore::clear(include_pinned)`, respond `Ok` or `Err`
  - [ ] `GetConfig` → respond with `DaemonResponse::Config(current_config)`
  - [ ] `UpdateConfig` → validate, update in-memory config, write TOML, trigger `AutostartManager` if `autostart` changed, activate/deactivate persistence if `persist_history` changed; respond `Ok` or `Err`
- [ ] Encode and write responses using `common::write_frame`

### Push Notifications
- [ ] After responding to `GetHistory`, keep the connection open
- [ ] Subscribe to a broadcast channel on `HistoryStore` for new entries
- [ ] When a new entry arrives, send `DaemonResponse::NewEntry(entry)` to all open connections
- [ ] Handle write errors (UI disconnected) by dropping the connection task

### Tests
- [ ] Integration test: connect a mock client, send `GetHistory`, receive `History` response
- [ ] Integration test: `UpdateConfig` round-trip updates in-memory config
- [ ] Integration test: push notification received after new entry is pushed to store
