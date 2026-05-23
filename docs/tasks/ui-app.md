# Module: `ui::app`

File: `crates/ui/src/app.rs`
Top-level `eframe::App` implementation. Holds all UI state and orchestrates components.

---

## Tasks

### `CopieurApp` State
- [ ] Define `CopieurApp` struct:
  - `ipc: IpcClient`
  - `entries: Vec<ClipboardEntry>`
  - `filtered: Vec<usize>`
  - `search_bar: SearchBar`
  - `card_list: CardList`
  - `show_settings: bool`
  - `settings_panel: SettingsPanel`
  - `config: Config`
  - `error_banner: Option<String>` — transient error message
- [ ] Implement `CopieurApp::new(cc: &eframe::CreationContext) -> Self`:
  - [ ] Connect `IpcClient` (exit with error message if daemon unreachable)
  - [ ] Send `GetHistory { offset: 0, limit: 200 }` and `GetConfig`; populate `entries` and `config`
  - [ ] Initialize `filtered` to all indices
  - [ ] Initialize `settings_panel` with the received config

### `eframe::App::update` Implementation
- [ ] Check for pushed `NewEntry` via `ipc.try_recv_push()` each frame:
  - [ ] Prepend new entry to `entries`
  - [ ] Re-apply search filter
- [ ] Render the top gear icon button; toggle `show_settings` on click
- [ ] If `show_settings`: render `SettingsPanel` overlay; hide card list
- [ ] Otherwise: render `SearchBar` then `CardList`

### Search Integration
- [ ] On any printable key pressed outside a text edit: redirect to search bar (set query character, request focus)
- [ ] Whenever `search_bar.query` changes: recompute `filtered` via `search_bar::filter`
- [ ] Pass `filtered` indices to `CardList::show`

### Action Handling
- [ ] On `CardListAction::Paste(id)`:
  - [ ] Send `IpcRequest::PasteEntry { id }` to daemon
  - [ ] Close window (`frame.close()`)
- [ ] On `CardListAction::Delete(id)`:
  - [ ] Send `IpcRequest::DeleteEntry { id }`
  - [ ] Remove entry from local `entries` and update `filtered`
- [ ] On `CardListAction::Pin(id)` / `Unpin(id)`:
  - [ ] Send `IpcRequest::PinEntry / UnpinEntry { id }`
  - [ ] Update `entry.pinned` in local `entries`
  - [ ] Re-sort display order
- [ ] On `CardListAction::Copy(id)`:
  - [ ] Write entry content to system clipboard via `arboard` (local, no daemon round-trip)
- [ ] On `CardListAction::Close`: close window
- [ ] On `SettingsAction::SaveConfig(config)`:
  - [ ] Send `IpcRequest::UpdateConfig(config)`
  - [ ] Update local `self.config` on `Ok` response
- [ ] On `SettingsAction::Close`: set `show_settings = false`

### Window Close Triggers
- [ ] Escape key pressed: close window
- [ ] Click outside window rect (`ctx.input().pointer.any_click()` when outside): close window
- [ ] On close: clear `search_bar.query` before window is destroyed

### Error Banner
- [ ] If IPC call returns `Err`, set `error_banner = Some(message)`
- [ ] Render banner at top of window if set; auto-dismiss after 3 s or on next successful IPC call

### Daemon-Not-Running Startup Error
- [ ] If `IpcClient::connect` fails, show a modal: "Copieur daemon is not running. Start with: `copieur --daemon`" and exit

### Tests
- [ ] Integration test: `CopieurApp::new` with a mock daemon socket populates entries correctly
- [ ] Integration test: `NewEntry` push prepends to entries list
