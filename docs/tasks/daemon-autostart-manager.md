# Module: `daemon::autostart_manager`

File: `crates/daemon/src/autostart_manager.rs`
Registers or deregisters the daemon as an XDG autostart application.

---

## Tasks

### Dependencies
- [ ] No new dependencies required (uses `std::fs`, `std::env::current_exe`)

### Desktop File Path
- [ ] Implement `desktop_file_path() -> PathBuf` returning `~/.config/autostart/copieur.desktop`
- [ ] Create parent directory if it does not exist

### `enable()`
- [ ] Get the absolute path of the current executable via `std::env::current_exe()`
- [ ] Write `~/.config/autostart/copieur.desktop` with content:
  ```ini
  [Desktop Entry]
  Type=Application
  Name=Copieur
  Exec=/path/to/copieur --daemon
  Hidden=false
  X-GNOME-Autostart-enabled=true
  ```
- [ ] Return `Ok(())` on success, `Err` on file write failure

### `disable()`
- [ ] Delete `~/.config/autostart/copieur.desktop` if it exists
- [ ] Ignore `NotFound` errors (already disabled)
- [ ] Return `Ok(())` on success, `Err` on unexpected failure

### Integration with `IpcServer`
- [ ] `IpcServer` calls `AutostartManager::enable()` or `::disable()` when `UpdateConfig` changes the `autostart` field
- [ ] Wrap call in error logging; do not propagate failure to UI as fatal

### Tests
- [ ] Unit test: `enable()` writes a file with correct `Exec` path
- [ ] Unit test: `disable()` removes the file; calling `disable()` twice does not error
- [ ] Unit test: `enable()` then `disable()` leaves no file
