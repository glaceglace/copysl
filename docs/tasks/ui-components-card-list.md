# Module: `ui::components::card_list`

File: `crates/ui/src/components/card_list.rs`
Renders the scrollable list of clipboard history cards.

---

## Tasks

### `CardList` State
- [ ] Define `CardList` struct: `selected_idx: Option<usize>`
- [ ] Implement `CardList::new() -> Self`

### Rendering (`show`)
- [ ] Implement `fn show(&mut self, ui: &mut egui::Ui, entries: &[ClipboardEntry], filtered: &[usize]) -> Option<CardListAction>`
- [ ] Use `egui::ScrollArea::vertical()` for the scrollable container
- [ ] Compute display order at render time (do not mutate input):
  - [ ] Pinned entries first (in their `filtered` appearance order)
  - [ ] Then unpinned entries (newest first, per `filtered` order)
- [ ] Render each visible entry by calling `card::show_card`
- [ ] Pass `selected = (Some(i) == self.selected_idx)` to highlight the active card

### Keyboard Navigation
- [ ] Consume `↑` key: decrement `selected_idx` (clamp to 0, do not wrap)
- [ ] Consume `↓` key: increment `selected_idx` (clamp to `len - 1`, do not wrap)
- [ ] Consume `Enter`: emit `CardListAction::Paste(id)` for the selected entry
- [ ] Consume `Delete`: emit `CardListAction::Delete(id)` for the selected entry
- [ ] Consume `Escape`: emit `CardListAction::Close`
- [ ] If `filtered` list changes (search updated), reset `selected_idx` to `Some(0)` if non-empty, else `None`

### Auto-Scroll
- [ ] After keyboard navigation changes `selected_idx`, scroll the `ScrollArea` to ensure the highlighted card is in view
- [ ] Use `ui.scroll_to_cursor` or track card rects and call `ScrollArea::scroll_to_rect`

### Mouse Interaction
- [ ] Forward `CardAction` from `card::show_card` to `CardListAction`
- [ ] On card click (primary): emit `CardListAction::Paste(id)` and close

### `CardListAction`
- [ ] Define `CardListAction` enum: `Paste(EntryId)`, `Delete(EntryId)`, `Pin(EntryId)`, `Unpin(EntryId)`, `Copy(EntryId)`, `Close`

### Tests
- [ ] Unit test: keyboard `↓` increments selection, clamps at last entry
- [ ] Unit test: keyboard `↑` decrements selection, clamps at first entry
- [ ] Unit test: display order puts pinned entries before unpinned
