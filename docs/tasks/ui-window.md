# Module: `ui::window`

File: `crates/ui/src/window.rs`
Builds the eframe `NativeOptions` for the clipboard history window.

---

## Tasks

### Dependencies
- [ ] `eframe` crate with `default_fonts` feature
- [ ] `egui` (re-exported by eframe)

### `build_native_options`
- [ ] Implement `fn build_native_options(cursor_pos: Option<(i32, i32)>, config: &Config) -> eframe::NativeOptions`
- [ ] Set `decorated: false` (no OS title bar / border)
- [ ] Set `resizable: false`
- [ ] Set `always_on_top: true`
- [ ] Set `skip_taskbar: true` (exclude from Alt+Tab / taskbar)
- [ ] Set `initial_window_size: Some(egui::Vec2::new(380.0, 520.0))`
- [ ] Call `compute_window_pos` for `initial_window_pos`

### `compute_window_pos`
- [ ] Implement `fn compute_window_pos(cursor_pos: Option<(i32, i32)>, config: &Config) -> Option<egui::Pos2>`:
  - [ ] If `config.window_position == WindowPos::NearCursor` and `cursor_pos` is `Some`: return position at cursor, clamped to screen bounds (assume 1920×1080 fallback if screen size unknown)
  - [ ] If `config.window_position == WindowPos::Fixed(x, y)`: return that fixed position
  - [ ] If cursor position is unavailable: return `None` (eframe centers the window)

### Cursor Position Detection
- [ ] Implement `fn get_cursor_pos() -> Option<(i32, i32)>`:
  - [ ] X11: use `xcb` to query pointer position on the root window
  - [ ] Wayland: currently returns `None` (compositor does not expose cursor position to non-focused clients)

### Tests
- [ ] Unit test: `compute_window_pos` with `NearCursor` near screen edge clamps position so window stays on screen
- [ ] Unit test: `compute_window_pos` with `Fixed` returns exact coordinates
