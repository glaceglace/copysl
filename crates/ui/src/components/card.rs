use std::time::Duration;
use common::{ClipboardEntry, ContentPayload};
use crate::style::{RADIUS_CARD, SPACE_M};

#[derive(Debug, Clone, PartialEq)]
pub enum CardAction {
    Paste,
    Delete,
    Pin,
    Unpin,
    Copy,
}

/// Pure function: compute the display lines for a plain-text preview.
///
/// Returns at most `max_lines` strings.  If the source text has more lines
/// than `max_lines`, the last returned element is `"…"`.
pub fn preview_lines(text: &str, max_lines: usize) -> Vec<String> {
    if max_lines == 0 {
        return vec![];
    }
    let mut result = Vec::with_capacity(max_lines);
    let mut iter = text.lines();
    loop {
        match iter.next() {
            None => break,
            Some(line) => {
                if result.len() + 1 < max_lines {
                    result.push(line.to_string());
                } else {
                    let has_more = iter.next().is_some();
                    result.push(if has_more { "\u{2026}".to_string() } else { line.to_string() });
                    break;
                }
            }
        }
    }
    result
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

/// Render one clipboard entry card.
///
/// `selected`     — keyboard-selected (highlighted background + 2 px accent border).
/// `scroll_to_me` — when true the containing ScrollArea scrolls to show this card.
/// `texture`      — pre-decoded GPU texture for `Image` entries; `None` for text.
pub fn show_card(
    ui: &mut egui::Ui,
    entry: &ClipboardEntry,
    selected: bool,
    scroll_to_me: bool,
    texture: Option<&egui::TextureHandle>,
) -> Option<CardAction> {
    let mut action = None;

    let ch = crate::style::card_height(ui.ctx().screen_rect().height());
    let content_height = (ch - SPACE_M * 2.0).max(0.0);

    // Pre-estimate the card rect for hover detection (one frame lag is acceptable).
    let card_top = ui.next_widget_position();
    let card_rect_est = egui::Rect::from_min_size(
        card_top,
        egui::vec2(ui.available_width(), ch),
    );
    let is_card_hovered = ui.rect_contains_pointer(card_rect_est);

    let visuals = ui.visuals().clone();

    let fill = if selected {
        visuals.selection.bg_fill
    } else if is_card_hovered {
        visuals.widgets.hovered.bg_fill
    } else {
        visuals.widgets.inactive.bg_fill
    };

    let frame = egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::same(SPACE_M as i8))
        .corner_radius(egui::CornerRadius::same(RADIUS_CARD));

    let mut delete_btn_rect: Option<egui::Rect> = None;
    let mut pin_btn_rect: Option<egui::Rect> = None;
    let frame_resp = frame.show(ui, |ui| {
        let avail = ui.available_width();
        let meta_w = crate::style::meta_width(avail);
        let text_w = crate::style::text_width(avail, ui.spacing().item_spacing.x);

        ui.horizontal(|ui| {
            // ── Text preview column ───────────────────────────────────────
            ui.vertical(|ui| {
                ui.set_width(text_w);
                ui.set_min_height(content_height);
                match &entry.payload {
                    ContentPayload::PlainText(text) => {
                        for line in preview_lines(text, 4) {
                            ui.add(egui::Label::new(line).truncate())
                                .on_hover_text(text.as_str());
                        }
                    }
                    ContentPayload::RichText { plain_preview, .. } => {
                        let lbl = ui.add(
                            egui::Label::new(plain_preview.as_str()).truncate(),
                        );
                        lbl.on_hover_text(plain_preview.as_str());
                        ui.label(
                            egui::RichText::new("HTML")
                                .text_style(egui::TextStyle::Small),
                        );
                    }
                    ContentPayload::Image { .. } => {
                        if let Some(tex) = texture {
                            let [tw, th] = tex.size();
                            let (tw, th) = (tw as f32, (th as f32).max(1.0));
                            let scale = (text_w / tw).min(content_height / th);
                            let sized = egui::load::SizedTexture::new(
                                tex.id(),
                                egui::vec2(tw * scale, th * scale),
                            );
                            ui.add(egui::Image::new(sized));
                        } else {
                            ui.label("[Image]");
                        }
                    }
                }
            });

            // ── Metadata column (right) — timestamp only ──────────────────
            // Buttons have been moved to a floating overlay so they never
            // shift the timestamp or text-preview columns.
            ui.vertical(|ui| {
                ui.set_width(meta_w);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    let age = entry.captured_at.elapsed().unwrap_or(Duration::ZERO);
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format_relative_time(age))
                                .text_style(egui::TextStyle::Small)
                                .color(visuals.weak_text_color()),
                        )
                        .truncate(),
                    );
                });
            });
        });
    });

    // Interaction sense so we can detect hover and click on the full card area.
    let interact = frame_resp.response.interact(egui::Sense::click());

    // ── Borders ───────────────────────────────────────────────────────────────
    // Selected: 2 px accent border via painter (on top of the frame border).
    // Hovered:  1 px hovered stroke.
    // Resting:  0.5 px noninteractive stroke — always visible, very subtle.
    if selected {
        ui.painter().rect_stroke(
            interact.rect.shrink(1.0),
            egui::CornerRadius::same(RADIUS_CARD),
            egui::Stroke::new(2.0, visuals.selection.stroke.color),
            egui::StrokeKind::Middle,
        );
    } else if is_card_hovered {
        ui.painter().rect_stroke(
            interact.rect.shrink(0.5),
            egui::CornerRadius::same(RADIUS_CARD),
            egui::Stroke::new(1.0, visuals.widgets.hovered.bg_stroke.color),
            egui::StrokeKind::Middle,
        );
    } else {
        ui.painter().rect_stroke(
            interact.rect,
            egui::CornerRadius::same(RADIUS_CARD),
            egui::Stroke::new(0.5, visuals.widgets.noninteractive.bg_stroke.color),
            egui::StrokeKind::Middle,
        );
    }

    // ── Pinned accent bar (3 px left-edge) ───────────────────────────────────
    if entry.pinned {
        let accent_rect = egui::Rect::from_min_size(
            interact.rect.min + egui::vec2(0.0, 4.0),
            egui::vec2(3.0, interact.rect.height() - 8.0),
        );
        ui.painter().rect_filled(
            accent_rect,
            egui::CornerRadius::same(2),
            visuals.selection.stroke.color,
        );
    }

    // ── Floating button overlay (bottom-left corner, only when hovered) ──────
    // Buttons are painted over the card using ui.interact() so they never
    // allocate layout space and cannot shift the timestamp or text columns.
    if is_card_hovered {
        let btn_size = egui::vec2(16.0, 16.0);
        let margin = SPACE_M;
        let del_origin = egui::pos2(
            interact.rect.left() + margin,
            interact.rect.bottom() - btn_size.y - margin,
        );
        let del_rect = egui::Rect::from_min_size(del_origin, btn_size);

        let del_resp = ui.interact(
            del_rect,
            egui::Id::new(entry.id).with("del"),
            egui::Sense::click(),
        );
        delete_btn_rect = Some(del_rect);

        // Paint delete button: transparent fill, red border, red X
        let del_red = egui::Color32::from_rgb(200, 60, 60);
        let del_fill = egui::Color32::from_rgba_unmultiplied(200, 60, 60, 20);
        ui.painter().rect(
            del_rect,
            egui::CornerRadius::same(3),
            del_fill,
            egui::Stroke::new(1.0, del_red),
            egui::StrokeKind::Middle,
        );
        let pad = 4.0;
        let p = ui.painter();
        p.line_segment(
            [del_rect.min + egui::vec2(pad, pad), del_rect.max - egui::vec2(pad, pad)],
            egui::Stroke::new(1.5, del_red),
        );
        p.line_segment(
            [egui::pos2(del_rect.max.x - pad, del_rect.min.y + pad),
             egui::pos2(del_rect.min.x + pad, del_rect.max.y - pad)],
            egui::Stroke::new(1.5, del_red),
        );

        if del_resp.clicked() {
            action = Some(CardAction::Delete);
        }

        // Pin button: immediately to the right of the delete button
        let pin_origin = egui::pos2(del_rect.right() + 4.0, del_rect.top());
        let pin_rect = egui::Rect::from_min_size(pin_origin, btn_size);

        let pin_resp = ui.interact(
            pin_rect,
            egui::Id::new(entry.id).with("pin"),
            egui::Sense::click(),
        );
        pin_btn_rect = Some(pin_rect);

        // Paint pin button: accent color when pinned, muted otherwise
        let pin_color = if entry.pinned {
            visuals.selection.stroke.color
        } else {
            visuals.widgets.inactive.fg_stroke.color
        };
        let pin_fill = egui::Color32::from_rgba_unmultiplied(
            pin_color.r(), pin_color.g(), pin_color.b(), 20,
        );
        ui.painter().rect(
            pin_rect,
            egui::CornerRadius::same(3),
            pin_fill,
            egui::Stroke::new(1.0, pin_color),
            egui::StrokeKind::Middle,
        );
        // Draw thumbtack: circle head at top-center, vertical shaft below
        let cx = pin_rect.center().x;
        let head_y = pin_rect.min.y + 5.0;
        ui.painter().circle_filled(egui::pos2(cx, head_y), 3.0, pin_color);
        ui.painter().line_segment(
            [egui::pos2(cx, head_y + 3.0), egui::pos2(cx, pin_rect.max.y - 2.0)],
            egui::Stroke::new(1.5, pin_color),
        );

        if pin_resp.clicked() {
            action = Some(if entry.pinned { CardAction::Unpin } else { CardAction::Pin });
        }
    }

    // Scroll the containing ScrollArea to show this card when keyboard
    // navigation just moved here.
    if scroll_to_me {
        interact.scroll_to_me(Some(egui::Align::Center));
    }

    // Right-click context menu
    frame_resp.response.context_menu(|ui| {
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

    // Primary click = paste.  Use click position to distinguish from the delete
    // and pin buttons: egui's interact() fires for the whole card rect, so we
    // check whether the pointer landed on either button rect and route accordingly.
    if interact.clicked() {
        let pos = ui.input(|i| i.pointer.interact_pos());
        action = route_card_click(pos, delete_btn_rect, pin_btn_rect, entry.pinned, action);
    }

    action
}

/// Route a card-level click to the correct [`CardAction`].
///
/// `pos` is the pointer position at click time (`None` → treated as no hit).
/// `existing` is an action already set by a child widget's own click handler;
/// button hits preserve it, a plain card hit always produces [`CardAction::Paste`].
pub(crate) fn route_card_click(
    pos: Option<egui::Pos2>,
    delete_rect: Option<egui::Rect>,
    pin_rect: Option<egui::Rect>,
    pinned: bool,
    existing: Option<CardAction>,
) -> Option<CardAction> {
    let hit_delete = delete_rect.zip(pos).map(|(r, p)| r.contains(p)).unwrap_or(false);
    let hit_pin    = pin_rect   .zip(pos).map(|(r, p)| r.contains(p)).unwrap_or(false);
    if hit_delete {
        existing.or(Some(CardAction::Delete))
    } else if hit_pin {
        existing.or(Some(if pinned { CardAction::Unpin } else { CardAction::Pin }))
    } else {
        Some(CardAction::Paste)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── preview_lines ─────────────────────────────────────────────────────────

    #[test]
    fn preview_lines_empty_text() {
        assert!(preview_lines("", 4).is_empty());
    }

    #[test]
    fn preview_lines_single_line() {
        assert_eq!(preview_lines("hello", 4), vec!["hello"]);
    }

    #[test]
    fn preview_lines_exactly_four_lines_no_ellipsis() {
        let text = "a\nb\nc\nd";
        let got = preview_lines(text, 4);
        assert_eq!(got, vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn preview_lines_five_lines_shows_ellipsis_on_fourth() {
        let text = "a\nb\nc\nd\ne";
        let got = preview_lines(text, 4);
        assert_eq!(got, vec!["a", "b", "c", "\u{2026}"]);
    }

    #[test]
    fn preview_lines_many_lines_caps_at_max() {
        let text = (0..20).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        let got = preview_lines(&text, 4);
        assert_eq!(got.len(), 4);
        assert_eq!(got[3], "\u{2026}");
    }

    #[test]
    fn preview_lines_three_lines_no_ellipsis() {
        let text = "x\ny\nz";
        let got = preview_lines(text, 4);
        assert_eq!(got, vec!["x", "y", "z"]);
    }

    #[test]
    fn preview_lines_max_zero_returns_empty() {
        assert!(preview_lines("a\nb\nc", 0).is_empty());
    }

    #[test]
    fn preview_lines_max_one_single_line_no_ellipsis() {
        assert_eq!(preview_lines("only", 1), vec!["only"]);
    }

    #[test]
    fn preview_lines_max_one_multiline_shows_ellipsis() {
        assert_eq!(preview_lines("a\nb", 1), vec!["\u{2026}"]);
    }

    // ── Happy-path coverage ───────────────────────────────────────────────────

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

    // ── Boundary transitions ──────────────────────────────────────────────────

    #[test]
    fn relative_time_zero_is_just_now() {
        assert_eq!(format_relative_time(Duration::ZERO), "just now");
    }

    #[test]
    fn relative_time_59s_is_just_now() {
        assert_eq!(format_relative_time(Duration::from_secs(59)), "just now");
    }

    #[test]
    fn relative_time_60s_crosses_to_minutes() {
        assert_eq!(format_relative_time(Duration::from_secs(60)), "1 min ago");
    }

    #[test]
    fn relative_time_3599s_is_minutes() {
        assert_eq!(format_relative_time(Duration::from_secs(3599)), "59 min ago");
    }

    #[test]
    fn relative_time_3600s_crosses_to_hours() {
        assert_eq!(format_relative_time(Duration::from_secs(3600)), "1 h ago");
    }

    #[test]
    fn relative_time_86399s_is_hours() {
        assert_eq!(format_relative_time(Duration::from_secs(86399)), "23 h ago");
    }

    #[test]
    fn relative_time_86400s_crosses_to_yesterday() {
        assert_eq!(format_relative_time(Duration::from_secs(86400)), "yesterday");
    }

    #[test]
    fn relative_time_172799s_is_still_yesterday() {
        assert_eq!(format_relative_time(Duration::from_secs(172799)), "yesterday");
    }

    #[test]
    fn relative_time_172800s_crosses_to_days() {
        assert_eq!(format_relative_time(Duration::from_secs(172800)), "2 days ago");
    }

    // ── route_card_click ──────────────────────────────────────────────────────

    fn r(x: f32, y: f32) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(16.0, 16.0))
    }
    fn p(x: f32, y: f32) -> Option<egui::Pos2> {
        Some(egui::pos2(x, y))
    }

    #[test]
    fn route_click_outside_buttons_is_paste() {
        assert_eq!(
            route_card_click(p(50.0, 50.0), Some(r(200.0, 0.0)), Some(r(200.0, 20.0)), false, None),
            Some(CardAction::Paste),
        );
    }

    #[test]
    fn route_click_on_delete_rect_is_delete() {
        assert_eq!(
            route_card_click(p(208.0, 8.0), Some(r(200.0, 0.0)), None, false, None),
            Some(CardAction::Delete),
        );
    }

    #[test]
    fn route_click_on_delete_preserves_existing_action() {
        // btn_resp.clicked() already fired — existing = Some(Delete); routing must not change it
        assert_eq!(
            route_card_click(p(208.0, 8.0), Some(r(200.0, 0.0)), None, false, Some(CardAction::Delete)),
            Some(CardAction::Delete),
        );
    }

    #[test]
    fn route_click_on_pin_unpinned_is_pin() {
        assert_eq!(
            route_card_click(p(208.0, 28.0), None, Some(r(200.0, 20.0)), false, None),
            Some(CardAction::Pin),
        );
    }

    #[test]
    fn route_click_on_pin_pinned_is_unpin() {
        assert_eq!(
            route_card_click(p(208.0, 28.0), None, Some(r(200.0, 20.0)), true, None),
            Some(CardAction::Unpin),
        );
    }

    #[test]
    fn route_click_on_pin_preserves_existing_action() {
        assert_eq!(
            route_card_click(p(208.0, 28.0), None, Some(r(200.0, 20.0)), false, Some(CardAction::Pin)),
            Some(CardAction::Pin),
        );
    }

    #[test]
    fn route_click_no_pos_is_paste() {
        // pos = None → no hit on any button → paste
        assert_eq!(
            route_card_click(None, Some(r(200.0, 0.0)), Some(r(200.0, 20.0)), false, None),
            Some(CardAction::Paste),
        );
    }

    #[test]
    fn route_click_no_button_rects_is_paste() {
        assert_eq!(
            route_card_click(p(50.0, 50.0), None, None, false, None),
            Some(CardAction::Paste),
        );
    }

    #[test]
    fn route_click_delete_takes_priority_over_pin() {
        // Degenerate case: both rects at the same position — delete wins
        let rect = r(200.0, 0.0);
        assert_eq!(
            route_card_click(p(208.0, 8.0), Some(rect), Some(rect), false, None),
            Some(CardAction::Delete),
        );
    }

    #[test]
    fn route_click_on_delete_rect_edge_inside() {
        // Click exactly on the rect boundary (min corner) — still a hit
        assert_eq!(
            route_card_click(p(200.0, 0.0), Some(r(200.0, 0.0)), None, false, None),
            Some(CardAction::Delete),
        );
    }

    #[test]
    fn route_click_one_pixel_outside_delete_is_paste() {
        // Click just outside the rect — not a hit
        assert_eq!(
            route_card_click(p(216.1, 8.0), Some(r(200.0, 0.0)), None, false, None),
            Some(CardAction::Paste),
        );
    }
}
