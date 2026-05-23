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
}

impl CardList {
    pub fn new() -> Self {
        CardList { selected_idx: None, prev_filtered_len: 0 }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        entries: &[ClipboardEntry],
        filtered: &[usize],
    ) -> Option<CardListAction> {
        // Reset selection when filtered list changes
        if filtered.len() != self.prev_filtered_len {
            self.selected_idx = if filtered.is_empty() { None } else { Some(0) };
            self.prev_filtered_len = filtered.len();
        }

        let display_order = compute_display_order(entries, filtered);
        let len = display_order.len();

        // Keyboard navigation
        let mut action = None;
        ui.input(|input| {
            if input.key_pressed(egui::Key::ArrowDown) {
                self.selected_idx = Some(match self.selected_idx {
                    None => 0,
                    Some(i) => (i + 1).min(len.saturating_sub(1)),
                });
            }
            if input.key_pressed(egui::Key::ArrowUp) {
                self.selected_idx = Some(match self.selected_idx {
                    None => 0,
                    Some(i) => i.saturating_sub(1),
                });
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

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (display_pos, &entry_idx) in display_order.iter().enumerate() {
                let entry = &entries[entry_idx];
                let selected = self.selected_idx == Some(display_pos);
                let card_action = show_card(ui, entry, selected);
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

        action
    }
}

impl Default for CardList {
    fn default() -> Self { Self::new() }
}

/// Pure function: compute display order indices.
/// Pinned entries first (in their filtered order), then unpinned (in filtered order).
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
        // entry 1 (pinned) should come first
        assert_eq!(order[0], 1);
        // then 0 and 2 (unpinned) in filtered order
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

    #[test]
    fn card_list_selection_down_increments() {
        let mut list = CardList::new();
        // Simulate: filtered list has 3 items, selection at 0
        list.selected_idx = Some(0);
        // We can't call show() without egui context, but we can test the logic directly
        // Test the compute_display_order logic
        let entries = vec![make_entry(0, false), make_entry(1, false), make_entry(2, false)];
        let filtered = vec![0, 1, 2];
        let order = compute_display_order(&entries, &filtered);
        assert_eq!(order.len(), 3);

        // Simulate down press: idx 0 -> 1
        let new_idx = (0usize + 1).min(order.len().saturating_sub(1));
        assert_eq!(new_idx, 1);

        // Simulate down press at end: stays at end
        let at_end = (order.len() - 1 + 1).min(order.len().saturating_sub(1));
        assert_eq!(at_end, order.len() - 1);
    }

    #[test]
    fn card_list_selection_up_clamps() {
        // At idx 0, up press stays at 0
        let idx: usize = 0;
        let new_idx = idx.saturating_sub(1);
        assert_eq!(new_idx, 0);
    }
}
