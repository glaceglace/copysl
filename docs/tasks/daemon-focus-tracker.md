# Module: `daemon::focus_tracker`

Files:
- `crates/daemon/src/focus_tracker/mod.rs` — `FocusTracker` trait + `FocusHandle`
- `crates/daemon/src/focus_tracker/x11.rs` — X11 implementation
- `crates/daemon/src/focus_tracker/wayland.rs` — Wayland implementation

Continuously tracks the focused window so `PasteExecutor` knows where to restore focus and inject Ctrl+V.

---

## Tasks

### Trait and Types (`mod.rs`)
- [ ] Define `FocusHandle` struct with `inner: PlatformFocusHandle`
- [ ] Define `PlatformFocusHandle` enum: `X11(u32)` (window id), `Wayland(/* toplevel handle type */)`, `Unknown`
- [ ] Define `FocusTracker` trait:
  - `fn start(&self) -> Result<()>` — begin tracking
  - `fn current_focus(&self) -> Option<FocusHandle>` — return last known focused window
- [ ] Implement a `NoopFocusTracker` for testing

### X11 Implementation (`x11.rs`)
- [ ] Add `xcb` dependency to daemon `Cargo.toml`
- [ ] Connect to X server via `xcb::Connection::connect`
- [ ] Subscribe to `PropertyNotify` events on the root window
- [ ] On each event, check if the changed atom is `_NET_ACTIVE_WINDOW`
- [ ] If yes, read the new window id via `xcb::get_property`
- [ ] Store the id atomically (e.g. `Arc<Mutex<Option<u32>>>`)
- [ ] Ignore changes where the new active window matches the Copieur UI window id (set after UI spawns)
- [ ] Run event loop on a dedicated thread

### Wayland Implementation (`wayland.rs`)
- [ ] Add `wayland-client` dependency
- [ ] Attempt to bind `ext-foreign-toplevel-list-v1` protocol
- [ ] If bound: subscribe to toplevel focus events, store active toplevel handle
- [ ] If protocol not available: fall back to recording focused window at the moment the shortcut fires (best-effort, may return `None`)
- [ ] Run Wayland event dispatch on a dedicated thread

### Platform Selection (`mod.rs`)
- [ ] At runtime, detect display server (check `WAYLAND_DISPLAY` env var first, then `DISPLAY`)
- [ ] Instantiate the correct implementation and box it as `Box<dyn FocusTracker>`

### Tests
- [ ] Unit test: `NoopFocusTracker::current_focus()` returns `None`
- [ ] Integration smoke test (X11, skipped on CI without display): subscribing and reading `_NET_ACTIVE_WINDOW`
