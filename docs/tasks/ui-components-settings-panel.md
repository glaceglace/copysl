# Module: `ui::components::settings_panel`

File: `crates/ui/src/components/settings_panel.rs`
Settings overlay panel opened via the gear icon in the main window.

---

## Tasks

### `SettingsPanel` State
- [ ] Define `SettingsPanel` struct:
  - `draft: Config` — local editable copy
  - `show_persist_warning: bool` — controls the privacy confirmation dialog
  - `rebinding: bool` — true while capturing a new shortcut key
- [ ] Implement `SettingsPanel::new(config: Config) -> Self`

### Rendering (`show`)
- [ ] Implement `fn show(&mut self, ui: &mut egui::Ui) -> Option<SettingsAction>`
- [ ] Render as an overlay panel on top of the card list (not a separate window) using `egui::Area` or `egui::Window` with no title bar
- [ ] Show a "×" close button in the top-right corner

### Global Shortcut Field
- [ ] Render a text label showing current shortcut (e.g. `"Super+V"`)
- [ ] On click, set `rebinding = true` and show "Press new shortcut..."
- [ ] Capture the next key press event from `ui.input().events` while `rebinding = true`
- [ ] Update `draft.shortcut` and clear `rebinding = false`

### Max History Entries Field
- [ ] Render a numeric `egui::DragValue` or text input, min = 10
- [ ] Clamp entered value to ≥ 10
- [ ] Update `draft.max_entries` on change

### Persist History Toggle
- [ ] Render a toggle switch (checkbox or toggle widget) bound to `draft.persist_history`
- [ ] When toggled from OFF → ON: set `show_persist_warning = true` (do NOT update draft yet)
- [ ] Render inline warning box when `show_persist_warning = true`:
  - [ ] Warning text: "⚠️ Enabling this saves clipboard contents (including passwords and tokens) to disk. Confirm to proceed."
  - [ ] "I understand, enable" button: set `draft.persist_history = true`, clear `show_persist_warning`
  - [ ] "Cancel" button: revert toggle, clear `show_persist_warning`
- [ ] When toggled from ON → OFF: update `draft.persist_history = false` directly (no warning needed)

### Autostart Toggle
- [ ] Render a toggle bound to `draft.autostart`
- [ ] Label: "Start on login"

### Window Position Dropdown
- [ ] Render a dropdown (`egui::ComboBox`) with options "Near cursor" and "Fixed position"
- [ ] If "Fixed position": show two numeric inputs for x and y coordinates

### UI Theme Dropdown
- [ ] Render a dropdown with options "System", "Light", "Dark"
- [ ] Update `draft.theme` on selection

### Save / Apply
- [ ] Detect changes between `draft` and the last-saved config
- [ ] Emit `SettingsAction::SaveConfig(draft.clone())` when the user leaves a field (or a dedicated "Apply" button)
- [ ] The `app.rs` layer sends `UpdateConfig` to the daemon on `SettingsAction::SaveConfig`

### `SettingsAction`
- [ ] Define `SettingsAction` enum: `SaveConfig(Config)`, `Close`

### Tests
- [ ] Unit test: toggling persist ON shows warning, not saving until confirmed
- [ ] Unit test: cancelling warning reverts the toggle to OFF
- [ ] Unit test: max entries input clamps to minimum 10
