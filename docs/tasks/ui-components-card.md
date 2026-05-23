# Module: `ui::components::card`

File: `crates/ui/src/components/card.rs`
Renders a single clipboard history entry as a card widget.

---

## Tasks

### Card Layout
- [ ] Implement `fn show_card(ui: &mut egui::Ui, entry: &ClipboardEntry, selected: bool) -> CardResponse`
- [ ] Use `egui::Frame` for card border/background; highlight if `selected = true`
- [ ] Layout: content area on the left, metadata column on the right (timestamp, pin icon, delete button)

### Text Card
- [ ] For `ContentPayload::PlainText(text)`:
  - [ ] Display first ~2 lines, truncated with `…` using `egui::Label` with `truncate(true)`
  - [ ] Show full text as a `egui::Tooltip` on hover

### Rich Text Card
- [ ] For `ContentPayload::RichText { plain_preview, .. }`:
  - [ ] Display `plain_preview`, truncated with `…`
  - [ ] Show a small grey `"HTML"` badge (small label with colored background) in the top-right metadata area
  - [ ] Tooltip shows full `plain_preview`

### Image Card
- [ ] For `ContentPayload::Image { data, mime }`:
  - [ ] Decode image from `data` bytes using `image` crate, scale to a fixed 80 px tall thumbnail
  - [ ] Cache the decoded `egui::TextureHandle` by `EntryId` to avoid re-decoding every frame
  - [ ] Render with `egui::Image`
  - [ ] Tooltip shows image at capped 400 px wide resolution

### Common Metadata Elements
- [ ] Timestamp: render in top-right using relative format:
  - [ ] < 60 s → "just now"
  - [ ] < 60 min → "X min ago"
  - [ ] < 24 h → "X h ago"
  - [ ] ≥ 24 h → "yesterday" or full date
  - [ ] Tooltip shows absolute datetime
- [ ] Pin icon: render `📌` label only if `entry.pinned = true`
- [ ] Delete button (✕): render as a small button, visible only on hover (`ui.input().pointer.any_hover()` within card rect); on click emit `CardAction::Delete`

### Right-Click Context Menu
- [ ] Use `egui::popup_above_or_below_widget` on right-click (`response.secondary_clicked()`)
- [ ] Menu items:
  - [ ] "Pin" / "Unpin" (toggled based on `entry.pinned`)
  - [ ] "Delete"
  - [ ] "Copy" (re-write to clipboard without pasting)

### `CardResponse` / `CardAction`
- [ ] Define `CardAction` enum: `Paste`, `Delete`, `Pin`, `Unpin`, `Copy`
- [ ] Return `Option<CardAction>` from `show_card`

### Tests
- [ ] Unit test: relative timestamp formatting for each time range
- [ ] Unit test: `filter` integration — text card matches search, image card does not
