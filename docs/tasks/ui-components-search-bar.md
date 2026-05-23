# Module: `ui::components::search_bar`

File: `crates/ui/src/components/search_bar.rs`
Search bar rendered at the top of every frame. Filters the card list in real time.

---

## Tasks

### `SearchBar` State
- [ ] Define `SearchBar` struct: `query: String`
- [ ] Implement `SearchBar::new() -> Self` with empty query
- [ ] Implement `SearchBar::clear(&mut self)` — resets query to `""`

### Rendering (`show`)
- [ ] Implement `fn show(&mut self, ui: &mut egui::Ui, focus_request: bool) -> bool` (returns `true` if query changed)
- [ ] Render a single-line `egui::TextEdit` with placeholder text "Search..."
- [ ] Apply `focus_request = true` to auto-focus the widget on window open
- [ ] Handle `Ctrl+F`: if pressed, request focus on the text edit
- [ ] Return whether the query string changed this frame

### Keystroke Redirect
- [ ] In `ui::app` (not here), detect when a printable key is pressed while a card is selected and no text edit is focused
- [ ] In that case, set `search_bar.query` to the typed character and request focus on search bar
- [ ] This logic belongs in `app.rs` but the search bar must expose `set_query(&mut self, s: &str)` and `request_focus(&mut self)`

### Filter Logic
- [ ] Implement `fn filter(query: &str, entries: &[ClipboardEntry]) -> Vec<usize>`:
  - [ ] If `query` is empty: return all indices in display order
  - [ ] Otherwise: case-insensitive substring match on text content
    - [ ] `PlainText`: match against the string
    - [ ] `RichText`: match against `plain_preview`
    - [ ] `Image`: excluded from results when query is non-empty
  - [ ] Return indices of matching entries

### Tests
- [ ] Unit test: `filter` with empty query returns all indices
- [ ] Unit test: `filter` with query matches case-insensitively
- [ ] Unit test: `filter` excludes image entries when query is non-empty
- [ ] Unit test: `filter` with no matches returns empty vec
