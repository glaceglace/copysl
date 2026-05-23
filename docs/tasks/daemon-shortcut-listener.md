# Module: `daemon::shortcut_listener`

Files:
- `crates/daemon/src/shortcut_listener/mod.rs` — `ShortcutListener` trait
- `crates/daemon/src/shortcut_listener/x11.rs` — XGrabKey implementation
- `crates/daemon/src/shortcut_listener/wayland.rs` — ext-global-shortcuts / evdev / notify fallback

Captures the global keyboard shortcut (default: Super+V) and spawns / toggles the UI process.

---

## Tasks

### Trait (`mod.rs`)
- [ ] Define `ShortcutListener` trait:
  - `fn start(&self, key: KeyCombo, on_trigger: impl Fn() + Send + 'static) -> Result<()>`
  - `fn stop(&self)`
- [ ] Implement platform selection: check `WAYLAND_DISPLAY` / `DISPLAY`, return boxed trait object

### X11 Implementation (`x11.rs`)
- [ ] Parse `KeyCombo` string into X11 modifier mask and keycode using `xcb::keysyms`
- [ ] Call `xcb::grab_key` on the root window for the configured key combo
- [ ] Run an X11 event loop on a dedicated thread
- [ ] On `KeyPress` event matching the grabbed key: call `on_trigger()`
- [ ] Handle `XGrabKey` failure gracefully (log error, do not crash daemon)

### Wayland Implementation (`wayland.rs`)
- [ ] Attempt to bind `ext-global-shortcuts-v1` compositor protocol
  - [ ] If available: register shortcut, call `on_trigger()` on activation event
- [ ] If `ext-global-shortcuts-v1` unavailable, attempt `evdev` input reading:
  - [ ] Open `/dev/input/event*` devices (requires `input` group permission)
  - [ ] Filter for the configured key combo
  - [ ] Call `on_trigger()` on match
- [ ] If evdev also unavailable: send desktop notification once via `notify-rust`: "Copieur: global shortcut not available on this compositor. Use `copieur` CLI to open clipboard history."

### UI Process Spawning (`mod.rs`)
- [ ] Track whether a UI process is currently running (store `Option<Child>`)
- [ ] On trigger:
  - [ ] If no UI running: `std::process::Command::new(current_exe()).env("COPIEUR_SOCKET", socket_path).spawn()`
  - [ ] If UI running: send SIGTERM to child process (toggle/close behavior)
- [ ] Reap the child process (handle `Child::try_wait`) to avoid zombies

### Tests
- [ ] Unit test: toggle logic — first trigger spawns, second trigger kills
- [ ] Unit test: `KeyCombo` parse produces correct modifier/keycode values
