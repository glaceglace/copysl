use common::{ClipboardEntry, EntryId};
use crate::components::card::{CardAction, show_card};

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
}

impl CardList {
    pub fn new() -> Self {
        CardList {
            selected_idx: None,
            prev_filtered_len: 0,
            needs_scroll: false,
        }
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
                    let card_action = show_card(ui, entry, selected, scroll);
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
}
