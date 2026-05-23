# Module: `daemon::paste_executor`

Files:
- `crates/daemon/src/paste_executor/mod.rs` — `PasteBackend` trait + orchestration
- `crates/daemon/src/paste_executor/xdotool.rs`
- `crates/daemon/src/paste_executor/xsendevent.rs`
- `crates/daemon/src/paste_executor/ydotool.rs`
- `crates/daemon/src/paste_executor/notification_fallback.rs`

Restores clipboard content and injects a synthetic Ctrl+V keystroke into the previously focused window.

---

## Tasks

### `PasteBackend` Trait (`mod.rs`)
- [ ] Define `PasteError` enum (variants: `InjectionFailed`, `ToolNotFound`, etc.)
- [ ] Define `PasteBackend` trait:
  - `fn inject_paste(&self, focus: &FocusHandle) -> Result<(), PasteError>`
- [ ] Implement `select_backend(display: DisplayServer) -> Box<dyn PasteBackend>`:
  - X11: try `XdotoolBackend`, fall back to `XSendEventBackend`
  - Wayland: try `YdotoolBackend`, fall back to `NotificationFallbackBackend`

### `PasteExecutor` Orchestration (`mod.rs`)
- [ ] Implement `PasteExecutor::paste(id: EntryId) -> Result<()>`:
  1. Look up `ClipboardEntry` by id in `HistoryStore`
  2. Write entry content to system clipboard via `arboard::Clipboard::set_*`
  3. Sleep 50 ms to allow UI window to close and focus to return
  4. Call `focus_tracker.current_focus()` to get the target window
  5. Call `backend.inject_paste(focus)`
  6. On error: fall through to `NotificationFallbackBackend`

### `XdotoolBackend` (`xdotool.rs`)
- [ ] Check if `xdotool` is in `$PATH` using `which::which` or `Command::new("which")`
- [ ] If available: run `xdotool key --clearmodifiers ctrl+v`
- [ ] If not available: return `PasteError::ToolNotFound`

### `XSendEventBackend` (`xsendevent.rs`)
- [ ] Using `xcb`, construct a synthetic `KeyPress` + `KeyRelease` event for Ctrl+V
- [ ] Send the event to the focused X11 window via `xcb::send_event`
- [ ] Handle the case where focus window id is `None` (return error)

### `YdotoolBackend` (`ydotool.rs`)
- [ ] Check if `ydotool` is in `$PATH`
- [ ] If available: run `ydotool key 29:1 47:1 47:0 29:0`
- [ ] If not available: return `PasteError::ToolNotFound`

### `NotificationFallbackBackend` (`notification_fallback.rs`)
- [ ] Add `notify-rust` dependency
- [ ] Send a desktop notification: "Copieur: content copied to clipboard — press Ctrl+V to paste."
- [ ] Always returns `Ok(())`

### Tests
- [ ] Unit test: `select_backend` returns correct type based on display server and available tools
- [ ] Unit test: fallback chain triggers notification when both primary and secondary backends fail
