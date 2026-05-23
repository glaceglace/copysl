use std::time::Duration;
use common::{ClipboardEntry, ContentPayload};

#[derive(Debug, Clone, PartialEq)]
pub enum CardAction {
    Paste,
    Delete,
    Pin,
    Unpin,
    Copy,
}

/// Pure function: format relative time. Testable without egui.
pub fn format_relative_time(age: Duration) -> String {
    let secs = age.as_secs();
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{} min ago", secs / 60)
    } else if secs < 86400 {
        format!("{} h ago", secs / 3600)
    } else if secs < 172800 {
        "yesterday".to_string()
    } else {
        let days = secs / 86400;
        format!("{days} days ago")
    }
}

pub fn show_card(ui: &mut egui::Ui, entry: &ClipboardEntry, selected: bool) -> Option<CardAction> {
    let mut action = None;

    let frame = egui::Frame::new()
        .fill(if selected {
            ui.visuals().selection.bg_fill
        } else {
            ui.visuals().widgets.inactive.bg_fill
        })
        .inner_margin(egui::Margin::same(8))
        .corner_radius(egui::CornerRadius::same(4));

    let resp = frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                ui.set_min_width(280.0);
                match &entry.payload {
                    ContentPayload::PlainText(text) => {
                        let preview: String = text.lines().take(2).collect::<Vec<_>>().join("\n");
                        let label = ui.label(egui::RichText::new(&preview));
                        label.on_hover_text(text.as_str());
                    }
                    ContentPayload::RichText { plain_preview, .. } => {
                        let label = ui.label(plain_preview.as_str());
                        label.on_hover_text(plain_preview.as_str());
                        ui.small("HTML");
                    }
                    ContentPayload::Image { data, .. } => {
                        let _ = data;
                        ui.label("[Image]");
                    }
                }
            });

            ui.vertical(|ui| {
                ui.set_min_width(80.0);
                let age = entry.captured_at.elapsed().unwrap_or(Duration::ZERO);
                ui.small(format_relative_time(age));

                if entry.pinned {
                    ui.small("📌");
                }

                let hover = ui.rect_contains_pointer(ui.min_rect());
                if hover && ui.small_button("✕").clicked() {
                    action = Some(CardAction::Delete);
                }
            });
        });
    });

    // Right-click context menu
    resp.response.context_menu(|ui| {
        if entry.pinned {
            if ui.button("Unpin").clicked() {
                action = Some(CardAction::Unpin);
                ui.close();
            }
        } else if ui.button("Pin").clicked() {
            action = Some(CardAction::Pin);
            ui.close();
        }
        if ui.button("Delete").clicked() {
            action = Some(CardAction::Delete);
            ui.close();
        }
        if ui.button("Copy").clicked() {
            action = Some(CardAction::Copy);
            ui.close();
        }
    });

    // Primary click = paste
    if resp.response.interact(egui::Sense::click()).clicked() {
        action = Some(CardAction::Paste);
    }

    action
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_time_just_now() {
        assert_eq!(format_relative_time(Duration::from_secs(30)), "just now");
    }

    #[test]
    fn relative_time_minutes() {
        assert_eq!(format_relative_time(Duration::from_secs(120)), "2 min ago");
    }

    #[test]
    fn relative_time_hours() {
        assert_eq!(format_relative_time(Duration::from_secs(7200)), "2 h ago");
    }

    #[test]
    fn relative_time_yesterday() {
        assert_eq!(format_relative_time(Duration::from_secs(90000)), "yesterday");
    }

    #[test]
    fn relative_time_days() {
        assert_eq!(format_relative_time(Duration::from_secs(3 * 86400)), "3 days ago");
    }
}
