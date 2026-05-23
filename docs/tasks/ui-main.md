# Module: `ui::main` — UI Entry Point

File: `crates/ui/src/main.rs`
UI entry point. Initializes the eframe window and runs the event loop.

---

## Tasks

### Crate Setup
- [ ] Create `crates/ui/` with `Cargo.toml` (binary crate, name `copieur-ui`)
- [ ] Add `eframe`, `egui`, `arboard`, `common`, `config` dependencies

### `ui_main()` Function
- [ ] Call `window::get_cursor_pos()` to retrieve current cursor position
- [ ] Load config from `IpcClient::send(GetConfig)` (or fall back to `config::load()` if IPC fails early)
- [ ] Build `eframe::NativeOptions` via `window::build_native_options(cursor_pos, &config)`
- [ ] Call `eframe::run_native("Copieur", options, Box::new(|cc| Box::new(CopieurApp::new(cc))))`
- [ ] Handle `run_native` errors with a user-visible error message

### Top-Level Binary Dispatch
- [ ] File `src/main.rs` (workspace root) reads `std::env::args()`
- [ ] If `--daemon` arg present: call `daemon::daemon_main()`
- [ ] Otherwise: call `ui::ui_main()`
- [ ] Add `src/main.rs` to root `Cargo.toml` as the single binary

### Tests
- [ ] Unit test: argument dispatch selects daemon vs UI path correctly (mock both `daemon_main` and `ui_main`)
