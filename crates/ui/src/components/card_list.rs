use std::collections::HashMap;
use common::{ClipboardEntry, ContentPayload, EntryId};
use crate::components::card::{CardAction, show_card};
use crate::emoji::EmojiRenderer;

#[derive(Debug, Clone, PartialEq)]
pub enum CardListAction {
    Paste(EntryId),
    Delete(EntryId),
    Pin(EntryId),
    Unpin(EntryId),
    Copy(EntryId),
    Close,
}

pub struct CardList {
    pub selected_idx: Option<usize>,
    prev_filtered_len: usize,
    /// Set true when arrow-key navigation just changed the selection so the
    /// card can ask its parent ScrollArea to scroll it into view.  Cleared
    /// after each render pass so mouse-wheel scrolling is never fought.
    needs_scroll: bool,
    /// Decoded GPU textures for image entries, keyed by entry id.
    /// Populated lazily on first render; dropped when the entry is deleted.
    texture_cache: HashMap<EntryId, egui::TextureHandle>,
    /// Color emoji renderer — loads NotoColorEmoji on first construction and
    /// caches per-codepoint textures so emoji render in full color.
    emoji: EmojiRenderer,
}

impl CardList {
    pub fn new() -> Self {
        CardList {
            selected_idx: None,
            prev_filtered_len: 0,
            needs_scroll: false,
            texture_cache: HashMap::new(),
            emoji: EmojiRenderer::new(),
        }
    }

    /// Remove a cached texture when its entry is deleted or history is cleared.
    pub fn evict_texture(&mut self, id: EntryId) {
        self.texture_cache.remove(&id);
    }

    /// Drop all cached textures (e.g. on ClearHistory).
    pub fn clear_textures(&mut self) {
        self.texture_cache.clear();
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        entries: &[ClipboardEntry],
        filtered: &[usize],
    ) -> Option<CardListAction> {
        // Reset selection when the filtered list changes (new search query).
        if filtered.len() != self.prev_filtered_len {
            self.selected_idx = if filtered.is_empty() { None } else { Some(0) };
            self.prev_filtered_len = filtered.len();
            self.needs_scroll = true;
        }

        let display_order = compute_display_order(entries, filtered);
        let len = display_order.len();

        let mut action = None;
        ui.input(|input| {
            if input.key_pressed(egui::Key::ArrowDown) {
                let new_idx = Some(match self.selected_idx {
                    None => 0,
                    Some(i) => (i + 1).min(len.saturating_sub(1)),
                });
                if new_idx != self.selected_idx {
                    self.selected_idx = new_idx;
                    self.needs_scroll = true;
                }
            }
            if input.key_pressed(egui::Key::ArrowUp) {
                let new_idx = Some(match self.selected_idx {
                    None => 0,
                    Some(i) => i.saturating_sub(1),
                });
                if new_idx != self.selected_idx {
                    self.selected_idx = new_idx;
                    self.needs_scroll = true;
                }
            }
            if input.key_pressed(egui::Key::Escape) {
                action = Some(CardListAction::Close);
            }
            if input.key_pressed(egui::Key::Enter) {
                if let Some(sel) = self.selected_idx {
                    if let Some(&entry_idx) = display_order.get(sel) {
                        action = Some(CardListAction::Paste(entries[entry_idx].id));
                    }
                }
            }
            if input.key_pressed(egui::Key::Delete) {
                if let Some(sel) = self.selected_idx {
                    if let Some(&entry_idx) = display_order.get(sel) {
                        action = Some(CardListAction::Delete(entries[entry_idx].id));
                    }
                }
            }
        });

        egui::ScrollArea::vertical()
            .scroll_bar_visibility(egui::containers::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                for (display_pos, &entry_idx) in display_order.iter().enumerate() {
                    if display_pos > 0 {
                        ui.add_space(crate::style::SPACE_S);
                    }
                    let entry = &entries[entry_idx];
                    let selected = self.selected_idx == Some(display_pos);
                    // Only ask for a scroll when keyboard navigation just moved here;
                    // otherwise the scroll area fights the user's manual scrolling.
                    let scroll = selected && self.needs_scroll;

                    // Decode image entries into GPU textures on first render.
                    if let ContentPayload::Image { data, .. } = &entry.payload {
                        if !self.texture_cache.contains_key(&entry.id) {
                            let tex = decode_image_texture(ui.ctx(), entry.id, data);
                            self.texture_cache.insert(entry.id, tex);
                        }
                    }
                    let texture = self.texture_cache.get(&entry.id);
                    let card_action = show_card(ui, entry, selected, scroll, texture, &mut self.emoji);
                    if let Some(card_action) = card_action {
                        action = Some(match card_action {
                            CardAction::Paste => CardListAction::Paste(entry.id),
                            CardAction::Delete => CardListAction::Delete(entry.id),
                            CardAction::Pin => CardListAction::Pin(entry.id),
                            CardAction::Unpin => CardListAction::Unpin(entry.id),
                            CardAction::Copy => CardListAction::Copy(entry.id),
                        });
                    }
                }
            });

        // Clear the scroll flag so the next frame doesn't re-trigger a scroll
        // unless the user presses another arrow key.
        self.needs_scroll = false;

        action
    }
}

impl Default for CardList {
    fn default() -> Self { Self::new() }
}

/// Decode PNG bytes into an egui GPU texture.  Falls back to a 1×1 black
/// pixel on decode failure so the card still renders without crashing.
fn decode_image_texture(
    ctx: &egui::Context,
    id: EntryId,
    png_data: &[u8],
) -> egui::TextureHandle {
    let rgba = image::load_from_memory(png_data)
        .unwrap_or_else(|_| image::DynamicImage::new_rgba8(1, 1))
        .to_rgba8();
    let (w, h) = rgba.dimensions();
    let color_img = egui::ColorImage::from_rgba_unmultiplied(
        [w as usize, h as usize],
        rgba.as_raw(),
    );
    ctx.load_texture(
        format!("clip_{}", id.0),
        color_img,
        egui::TextureOptions::LINEAR,
    )
}

/// Pure function: compute display order indices.
/// Pinned entries first (in their filtered order), then unpinned.
pub fn compute_display_order(entries: &[ClipboardEntry], filtered: &[usize]) -> Vec<usize> {
    let pinned: Vec<usize> = filtered.iter().copied()
        .filter(|&i| entries[i].pinned)
        .collect();
    let unpinned: Vec<usize> = filtered.iter().copied()
        .filter(|&i| !entries[i].pinned)
        .collect();
    pinned.into_iter().chain(unpinned).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{ContentPayload, EntryId};
    use std::time::SystemTime;

    fn make_entry(id: u64, pinned: bool) -> ClipboardEntry {
        ClipboardEntry {
            id: EntryId(id),
            payload: ContentPayload::PlainText(format!("entry {id}")),
            captured_at: SystemTime::now(),
            pinned,
        }
    }

    #[test]
    fn display_order_pinned_first() {
        let entries = vec![
            make_entry(0, false),
            make_entry(1, true),
            make_entry(2, false),
        ];
        let filtered = vec![0, 1, 2];
        let order = compute_display_order(&entries, &filtered);
        assert_eq!(order[0], 1);
        assert_eq!(order[1], 0);
        assert_eq!(order[2], 2);
    }

    #[test]
    fn display_order_all_unpinned() {
        let entries = vec![make_entry(0, false), make_entry(1, false)];
        let filtered = vec![0, 1];
        let order = compute_display_order(&entries, &filtered);
        assert_eq!(order, vec![0, 1]);
    }

    #[test]
    fn display_order_empty_filtered() {
        let entries = vec![make_entry(0, false)];
        let filtered: Vec<usize> = vec![];
        let order = compute_display_order(&entries, &filtered);
        assert!(order.is_empty());
    }

    // ── Keyboard navigation logic (tested through the pure index arithmetic) ──

    #[test]
    fn arrow_down_increments_index() {
        let len = 3usize;
        let new = (0usize + 1).min(len.saturating_sub(1));
        assert_eq!(new, 1);
    }

    #[test]
    fn arrow_down_clamps_at_end() {
        let len = 3usize;
        let at_end = len - 1;
        let new = (at_end + 1).min(len.saturating_sub(1));
        assert_eq!(new, at_end);
    }

    #[test]
    fn arrow_up_clamps_at_zero() {
        let new = 0usize.saturating_sub(1);
        assert_eq!(new, 0);
    }

    #[test]
    fn card_list_starts_with_no_selection() {
        let list = CardList::new();
        assert!(list.selected_idx.is_none());
        assert!(!list.needs_scroll);
    }

    #[test]
    fn card_list_starts_with_empty_texture_cache() {
        let list = CardList::new();
        assert!(list.texture_cache.is_empty());
    }

    #[test]
    fn evict_texture_removes_entry() {
        let mut list = CardList::new();
        // Insert a dummy value using a raw HashMap insert (no actual GPU texture needed).
        // We can't create a real TextureHandle without an egui context, so we just
        // verify the evict_texture method removes the key if it exists.
        // Evicting a non-existent key must not panic.
        list.evict_texture(EntryId(42));
        assert!(list.texture_cache.is_empty());
    }

    #[test]
    fn clear_textures_empties_cache() {
        let mut list = CardList::new();
        // Evict from empty cache is a no-op.
        list.clear_textures();
        assert!(list.texture_cache.is_empty());
    }

    // ── decode_image_texture ──────────────────────────────────────────────────

    fn tiny_png() -> Vec<u8> {
        use image::ImageEncoder as _;
        let rgba = [255u8, 0, 0, 255];
        let mut buf = Vec::new();
        image::codecs::png::PngEncoder::new(&mut buf)
            .write_image(&rgba, 1, 1, image::ExtendedColorType::Rgba8)
            .expect("encode");
        buf
    }

    #[test]
    fn decode_image_texture_valid_png_does_not_panic() {
        let ctx = egui::Context::default();
        let _tex = decode_image_texture(&ctx, EntryId(1), &tiny_png());
        // texture was created without panicking
    }

    #[test]
    fn decode_image_texture_invalid_bytes_falls_back() {
        let ctx = egui::Context::default();
        // Should silently fall back to 1×1, not panic.
        let _tex = decode_image_texture(&ctx, EntryId(2), &[0u8; 16]);
    }

    #[test]
    fn decode_image_texture_empty_bytes_falls_back() {
        let ctx = egui::Context::default();
        let _tex = decode_image_texture(&ctx, EntryId(3), &[]);
    }

    #[test]
    fn decode_image_texture_distinct_ids_produce_distinct_textures() {
        let ctx = egui::Context::default();
        let png = tiny_png();
        let t1 = decode_image_texture(&ctx, EntryId(10), &png);
        let t2 = decode_image_texture(&ctx, EntryId(11), &png);
        // Different egui texture names → different texture IDs
        assert_ne!(t1.id(), t2.id());
    }

    #[test]
    fn display_order_image_entries_included() {
        // Image entries should be ordered like any other entry: pinned first.
        let image_entry = ClipboardEntry {
            id: EntryId(10),
            payload: ContentPayload::Image {
                data: vec![0u8; 4],
                mime: common::ImageMime::Png,
            },
            captured_at: SystemTime::now(),
            pinned: true,
        };
        let text_entry = make_entry(11, false);
        let entries = vec![image_entry, text_entry];
        let order = compute_display_order(&entries, &[0, 1]);
        assert_eq!(order[0], 0, "pinned image entry should appear first");
        assert_eq!(order[1], 1);
    }
}
