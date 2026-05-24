# Copieur UI Beautification — Implementation Prompt

## Context

Copieur is a clipboard history panel written in Rust with **egui/eframe 0.34**.
The window is **borderless** (`with_decorations(false)`) and **not resizable**.
All sizing must use **relative values** derived from window constants or
`ui.available_width()` / `ui.ctx().screen_rect()` — never hardcoded pixels that
would break when dimensions change.

Target aesthetic: **Windows Win+V style** — compact card-list panel, visible but
soft card borders, clear keyboard-selection highlight, comfortable balance between
mouse and keyboard use.

---

## 1. Window

### 1.1 Target dimensions

```rust
// crates/ui/src/window.rs
pub const WINDOW_WIDTH:  f32 = 480.0;
pub const WINDOW_HEIGHT: f32 = 680.0;
```

These replace the current 760 × 1040.  All internal proportions below are
expressed as fractions of these constants so they scale together if the values
are ever changed.

### 1.2 Window dragging (new feature)

The window is borderless so the user has no native drag handle.  The **header
strip** (see §4.1) must be the drag region.

Implementation in `app.rs`:

```rust
// Inside render(), after drawing the header row:
let header_resp = /* the Response of the header horizontal layout */;
if header_resp.is_pointer_button_down_on() {
    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
}
```

The header cursor must change to a grab cursor when hovered:

```rust
header_resp.on_hover_cursor(egui::CursorIcon::Grab);
```

### 1.3 Optional: rounded window corners

If the compositor supports transparency, pass `with_transparent(true)` in
`ViewportBuilder` and override `clear_color` to return fully transparent
`[0.0, 0.0, 0.0, 0.0]`.  Then paint the entire UI inside a rounded
`egui::Frame` with `corner_radius(8)` at the outermost level.  This gives the
window itself rounded corners.  This is optional and compositor-dependent — the
rest of the design must work without it.

---

## 2. Design System

Define these constants once in a shared module (e.g. `crates/ui/src/style.rs`)
and import everywhere.  No component may hard-code a spacing or radius value that
is not derived from these.

### 2.1 Spacing scale

```rust
pub const SPACE_XS: f32 = 4.0;   // separator gaps, badge margins
pub const SPACE_S:  f32 = 6.0;   // between cards in the list
pub const SPACE_M:  f32 = 10.0;  // card inner margin, section padding
pub const SPACE_L:  f32 = 16.0;  // header/search bar vertical padding
```

All `Margin::same(x)` calls must use one of these constants.

### 2.2 Typography scale

```rust
// Use egui's built-in TextStyle; do not hardcode font sizes.
// Heading  → egui::TextStyle::Heading  (card list title, header)
// Body     → egui::TextStyle::Body     (card preview text)
// Small    → egui::TextStyle::Small    (timestamp, badges)
// Monospace→ egui::TextStyle::Monospace (install-hint code block)
```

The card preview text uses `TextStyle::Body`.  The timestamp and "HTML" badge
use `TextStyle::Small`.  Do not mix explicit font sizes with `RichText::size()`.

### 2.3 Corner radii

```rust
pub const RADIUS_CARD:   u8 = 6;   // clipboard entry cards
pub const RADIUS_SEARCH: u8 = 6;   // search bar frame
pub const RADIUS_BUTTON: u8 = 4;   // small action buttons
pub const RADIUS_WINDOW: u8 = 8;   // outermost frame (only if transparent mode)
```

### 2.4 Semantic colors

Do not hard-code `Color32` values inside components.  Read from
`ui.visuals()` instead:

| Purpose | egui source |
|---|---|
| Panel / window background | `visuals.panel_fill` |
| Card resting background | `visuals.widgets.inactive.bg_fill` |
| Card hovered background | `visuals.widgets.hovered.bg_fill` |
| Card selected background | `visuals.selection.bg_fill` |
| Card selected border | `visuals.selection.stroke.color` (width 2 px) |
| Card hovered border | `visuals.widgets.hovered.bg_stroke.color` (width 1 px) |
| Card resting border | `visuals.widgets.noninteractive.bg_stroke.color` (width 0.5 px) — visible but very subtle |
| Timestamp / secondary text | `visuals.weak_text_color()` |
| Danger / delete | `egui::Color32::from_rgb(180, 60, 60)` in dark; read from visuals for light |
| Pin accent | `visuals.selection.stroke.color` or a warm amber — derive from selection |

For the **light theme**, the existing `light_visuals()` in `app.rs` already
provides warm cream tones.  No additional color overrides are needed.

---

## 3. Layout Structure

```
┌─────────────────────────────────────────┐  ← WINDOW_WIDTH × WINDOW_HEIGHT
│ [≡] Clipboard History            [⚙]   │  ← header strip,  H = W_H * 0.057 ≈ 39 px
├─────────────────────────────────────────┤
│ [🔍 Search…                           ] │  ← search bar,    H = W_H * 0.057 ≈ 39 px
├─────────────────────────────────────────┤  ← hairline separator
│  ┌─────────────────────────────────┐    │
│  │ 📌 Pinned text content here     │    │  ← card,          H = W_H / 7 ≈ 97 px
│  │                      just now  ✕│    │
│  └─────────────────────────────────┘    │
│  ┌─────────────────────────────────┐    │  ← SPACE_S gap between cards
│  │ Lorem ipsum dolor sit amet…     │    │
│  │                     2 min ago  ✕│    │
│  └─────────────────────────────────┘    │
│                   …                     │
└─────────────────────────────────────────┘
```

Outer horizontal padding on both sides: `SPACE_M` (10 px).
This is applied once at the `CentralPanel` level, not per-component.

---

## 4. Components

### 4.1 Header strip

**File**: `app.rs` → `render()`

```
[≡ drag-icon]  Clipboard History  ·····················  [⚙]
```

- **Height**: `WINDOW_HEIGHT * 0.057` (≈ 39 px).  Do not hardcode 39.
- **Drag icon**: render a small `"⠿"` or `"⋮⋮"` glyph in `weak_text_color()` on
  the far left.  It signals that the area is draggable without being loud.
- **Title**: `egui::RichText::new("Clipboard History").text_style(TextStyle::Body).strong()`.
  Not a `ui.heading()` — headings are too large for a compact panel.
- **Gear icon**: right-aligned via `ui.with_layout(right_to_left, ...)`.
  Button has no frame (use `egui::Button::new("⚙").frame(false)`); it should
  look like a symbol, not a boxy button.
- **Drag region**: the entire `ui.horizontal(...)` response triggers
  `ViewportCommand::StartDrag` on pointer-button-down.  Set cursor to
  `CursorIcon::Grab` on hover.
- **Separator**: `ui.separator()` with default stroke — a hairline between header
  and search bar.

### 4.2 Search bar

**File**: `components/search_bar.rs`

- Width: fills `ui.available_width()` (already implemented via
  `desired_width(f32::INFINITY)`).
- Add a surrounding `egui::Frame` with:
  - `corner_radius(RADIUS_SEARCH)`
  - `fill(visuals.extreme_bg_color)` (the light input background)
  - `stroke(visuals.widgets.noninteractive.bg_stroke)` (subtle border)
  - `inner_margin(Margin::symmetric(SPACE_M, SPACE_XS))`
- The `TextEdit` inside should have `frame(false)` so the custom frame is the
  only visible border.
- Hint text: `"🔍  Search clipboard…"` (magnifier + two spaces for padding).
- Height of the surrounding frame: `WINDOW_HEIGHT * 0.057` — same as the header,
  giving both strips equal visual weight.

### 4.3 Card

**File**: `components/card.rs`

#### Dimensions

```rust
let card_height  = WINDOW_HEIGHT / 7.0;          // ≈ 97 px at 680
let inner_margin = SPACE_M;                       // 10 px all sides
let content_h    = card_height - inner_margin * 2.0;
```

Column split inside the card:

```rust
let avail   = ui.available_width();
let meta_w  = (avail * 0.22).max(88.0);          // timestamp + actions
let text_w  = avail - meta_w - ui.spacing().item_spacing.x;
```

#### Resting state (no hover, no selection)

- `frame.fill = visuals.widgets.inactive.bg_fill`
- `frame.stroke = Stroke::new(0.5, visuals.widgets.noninteractive.bg_stroke.color)`
  — Always draw this thin border so cards are visually separated from the
  background even without hover or selection.

#### Hover state

- `frame.fill = visuals.widgets.hovered.bg_fill`
- `frame.stroke = Stroke::new(1.0, visuals.widgets.hovered.bg_stroke.color)`
- Show the `✕` delete button (currently only shown on hover — keep this behaviour).

#### Selected state (keyboard or mouse)

- `frame.fill = visuals.selection.bg_fill`
- Paint a `2 px` border using `visuals.selection.stroke.color` via
  `ui.painter().rect_stroke(...)`.

#### Pinned card accent

If `entry.pinned`:
- Paint a `3 px` vertical bar on the **left inner edge** of the card using the
  selection accent color:
  ```rust
  let accent_rect = egui::Rect::from_min_size(
      card_rect.min + egui::vec2(0.0, 4.0),
      egui::vec2(3.0, card_rect.height() - 8.0),
  );
  ui.painter().rect_filled(accent_rect, 2.0, visuals.selection.stroke.color);
  ```
- The `📌` emoji badge stays in the metadata column but the accent bar makes the
  pin status immediately visible without reading the icon.

#### Text preview column

- `ui.set_min_width(text_w)` + `ui.set_min_height(content_h)`.
- Clip long text to **3 lines** (`text.lines().take(3)`).  Hover tooltip shows
  the full text.
- Use `egui::Label::new(...).truncate()` to truncate lines that are still wider
  than the column rather than wrapping beyond 3 lines.

#### Metadata column (right)

Layout (top to bottom, right-aligned):

```
[timestamp]      ← TextStyle::Small, weak_text_color()
[📌]             ← only if pinned, same Small style
[✕]              ← only on hover, small_button, shown via rect_contains_pointer
```

- Timestamp: `ui.with_layout(egui::Layout::right_to_left(Align::TOP), ...)` so
  it's always right-aligned regardless of text length.
- The `✕` button: use `egui::Button::new("✕").small().frame(false)`.  Give it a
  danger-color tint on hover only:
  ```rust
  let btn = ui.add(egui::Button::new(
      egui::RichText::new("✕").color(if btn_resp.hovered() {
          egui::Color32::from_rgb(200, 70, 70)
      } else {
          visuals.weak_text_color()
      })
  ).frame(false));
  ```

### 4.4 Card list

**File**: `components/card_list.rs`

- `egui::ScrollArea::vertical()` already in place.
- Add `ui.add_space(SPACE_S)` **between** cards (not before the first or after
  the last).  Implement by tracking iteration position:
  ```rust
  for (i, &entry_idx) in display_order.iter().enumerate() {
      if i > 0 { ui.add_space(SPACE_S); }
      // … show_card(…)
  }
  ```
- The scroll area should have `show_scrollbar(egui::ScrollBarVisibility::VisibleWhenNeeded)`
  so the scrollbar only appears when the list overflows, keeping the card area
  maximally wide when few items exist.

### 4.5 Settings panel

**File**: `components/settings_panel.rs`

The settings panel overlays the card list in the same `CentralPanel`.  Style it
as a **distinct visual layer**:

- Wrap the entire settings content in an `egui::Frame` with:
  - `fill(visuals.window_fill)`
  - `stroke(visuals.window_stroke())`
  - `corner_radius(RADIUS_CARD)`
  - `inner_margin(Margin::same(SPACE_L))`
  This creates a "sheet" sitting on top of the main background.
- **Close button**: place a `"←"` or `"✕"` button (no frame) at the top-right,
  not a plain text "×".  Use `egui::Button::new("←").frame(false)` to indicate
  "back to list".
- **Section separators**: `ui.separator()` before each logical group (history
  limits, persistence, display settings, danger zone).
- **Danger zone** (Kill daemon): wrap in a frame with a faint red tint on the
  fill (`Color32::from_rgba_unmultiplied(180, 40, 40, 15)`) and a red stroke
  (`1 px`).  The "Kill daemon" button inside uses full red text.
- Use `ui.add_space(SPACE_S)` between controls inside each section for breathing
  room.

---

## 5. Implementation Rules

### 5.1 Absolute value ban

The following are **forbidden** in any UI component:

- `ui.set_min_width(N)` where `N` is a literal number.
- `ui.set_min_height(N)` where `N` is a literal number.
- `egui::Margin::same(N)` where `N` is a literal number not equal to a named
  constant from §2.1.
- `Vec2::new(W, H)` with literal W or H for anything other than icons/glyphs.

Every dimension must trace back to `WINDOW_WIDTH`, `WINDOW_HEIGHT`, a spacing
constant, `ui.available_width()`, or `ui.ctx().screen_rect()`.

### 5.2 egui API guidance

| Task | Correct API |
|---|---|
| Full-width widget | `desired_width(f32::INFINITY)` or `ui.set_min_width(ui.available_width())` |
| Right-align content | `ui.with_layout(Layout::right_to_left(Align::Center), \|ui\| …)` |
| Inter-card spacing | `ui.add_space(SPACE_S)` between iterations |
| Drag to move window | `ctx.send_viewport_cmd(ViewportCommand::StartDrag)` on pointer-down |
| Cursor change | `response.on_hover_cursor(CursorIcon::Grab)` |
| Frameless button | `egui::Button::new(…).frame(false)` |
| Clip text to width | `egui::Label::new(…).truncate()` |
| Scrollbar on demand | `ScrollArea::vertical().show_scrollbar(ScrollBarVisibility::VisibleWhenNeeded)` |
| Card painter border | `ui.painter().rect_stroke(rect, radius, Stroke::new(w, color), StrokeKind::Middle)` |

### 5.3 Do not touch

- `apply_theme()` and `light_visuals()` in `app.rs` — theme logic is already correct.
- `clear_color()` override in `app.rs` — already returns `visuals.panel_fill`.
- `focus_decision()` and `had_focus` logic — unchanged.
- IPC, daemon, config crates — UI-only task.

### 5.4 Tests

For every **pure function** added or changed during beautification, add a unit
test.  In particular:

- If you extract a `card_height(window_height: f32) -> f32` helper, test that it
  returns sensible values at common window heights and never goes below a minimum.
- If you extract column-width helpers, test the split adds up to `available_width`.
- Do not add tests that require an egui `Context` unless they follow the pattern
  already established in `app.rs` tests (`egui::Context::default()`).

---

## 6. Acceptance Criteria

After the changes, the app must satisfy **all** of the following:

1. Window is 480 × 680 px.
2. The header strip is draggable — clicking and dragging anywhere on it moves the
   window.  The cursor changes to a grab icon on hover.
3. Cards have a visible (but subtle) 0.5 px border at rest, a 1 px border on
   hover, and a 2 px accent border when keyboard-selected.
4. Pinned cards show a 3 px left-edge accent bar in the selection color.
5. The delete button `✕` is invisible at rest, visible on card hover, and turns
   red on button hover.
6. No literal pixel values appear in component code (except values that equal a
   named spacing/radius constant).
7. All existing tests pass (`cargo test -p ui`).
8. The settings panel is visually distinct from the card list (framed sheet).
9. The gear button has no box frame — it renders as a plain icon.
10. Card spacing uses `SPACE_S` between cards, not the default egui item spacing.
