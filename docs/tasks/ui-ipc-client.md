# Module: `ui::ipc_client`

File: `crates/ui/src/ipc_client.rs`
IPC client used by the UI to communicate with the daemon over the Unix socket.

---

## Tasks

### Dependencies
- [ ] Uses `std::os::unix::net::UnixStream` (no extra crate)
- [ ] Uses `common::{DaemonRequest, DaemonResponse, read_frame, write_frame}`

### `IpcClient` Struct
- [ ] Define `IpcClient { stream: UnixStream }`
- [ ] Implement `IpcClient::connect() -> Result<IpcClient>`:
  - [ ] Read socket path from `COPIEUR_SOCKET` environment variable
  - [ ] Connect with `UnixStream::connect(path)`
  - [ ] Set the stream to non-blocking mode (`stream.set_nonblocking(true)`) for push reads only
  - [ ] Keep a second blocking clone or toggle mode per operation
- [ ] Return a descriptive error if `COPIEUR_SOCKET` is not set or daemon is not reachable

### `send` (blocking request-response)
- [ ] Implement `fn send(&mut self, req: DaemonRequest) -> Result<DaemonResponse>`:
  - [ ] Temporarily set stream to blocking mode
  - [ ] Call `write_frame` to send the request
  - [ ] Call `read_frame` to receive the response
  - [ ] Return deserialized `DaemonResponse`

### `try_recv_push` (non-blocking, called each UI frame)
- [ ] Implement `fn try_recv_push(&mut self) -> Option<DaemonResponse>`:
  - [ ] Use non-blocking read; return `None` on `WouldBlock`
  - [ ] Return `Some(DaemonResponse)` if a complete frame is available
  - [ ] Buffer partial reads across frames

### Error Handling
- [ ] If the daemon disconnects mid-session, `send` and `try_recv_push` return errors
- [ ] The UI (caller) is responsible for displaying an error banner

### Tests
- [ ] Unit test: `connect` fails with a clear error when `COPIEUR_SOCKET` points to a nonexistent socket
- [ ] Unit test: `send` and `try_recv_push` correctly frame and decode messages against a mock server
