use common::{ClipboardEntry, ContentPayload};

pub struct SearchBar {
    pub query: String,
    focus_requested: bool,
}

impl SearchBar {
    pub fn new() -> Self {
        SearchBar { query: String::new(), focus_requested: false }
    }

    pub fn clear(&mut self) {
        self.query.clear();
    }

    pub fn set_query(&mut self, s: &str) {
        self.query = s.to_string();
    }

    pub fn request_focus(&mut self) {
        self.focus_requested = true;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, focus_request: bool) -> bool {
        use crate::style::{RADIUS_SEARCH, SPACE_M, SPACE_XS};

        let old_query = self.query.clone();

        // Handle Ctrl+F
        if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            self.focus_requested = true;
        }

        let visuals = ui.visuals().clone();
        let frame = egui::Frame::new()
            .fill(visuals.extreme_bg_color)
            .stroke(visuals.widgets.noninteractive.bg_stroke)
            .corner_radius(egui::CornerRadius::same(RADIUS_SEARCH))
            .inner_margin(egui::Margin::symmetric(SPACE_M as i8, SPACE_XS as i8));

        let mut response_opt = None;
        frame.show(ui, |ui| {
            let resp = ui.add(
                egui::TextEdit::singleline(&mut self.query)
                    .hint_text("🔍  Search clipboard…")
                    .frame(egui::Frame::new())
                    .desired_width(f32::INFINITY),
            );
            response_opt = Some(resp);
        });

        if let Some(response) = response_opt {
            if focus_request || self.focus_requested {
                response.request_focus();
                self.focus_requested = false;
            }
        }

        self.query != old_query
    }
}

impl Default for SearchBar {
    fn default() -> Self { Self::new() }
}

pub fn filter(query: &str, entries: &[ClipboardEntry]) -> Vec<usize> {
    if query.is_empty() {
        return (0..entries.len()).collect();
    }

    let query_lower = query.to_lowercase();
    entries.iter().enumerate().filter_map(|(i, entry)| {
        match &entry.payload {
            ContentPayload::PlainText(text) => {
                if text.to_lowercase().contains(&query_lower) { Some(i) } else { None }
            }
            ContentPayload::RichText { plain_preview, .. } => {
                if plain_preview.to_lowercase().contains(&query_lower) { Some(i) } else { None }
            }
            ContentPayload::Image { .. } => None,  // images excluded when query is non-empty
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{EntryId, ImageMime};
    use std::time::SystemTime;

    fn make_text(s: &str) -> ClipboardEntry {
        ClipboardEntry {
            id: EntryId(0),
            payload: ContentPayload::PlainText(s.to_string()),
            captured_at: SystemTime::now(),
            pinned: false,
        }
    }

    fn make_rich(preview: &str) -> ClipboardEntry {
        ClipboardEntry {
            id: EntryId(0),
            payload: ContentPayload::RichText {
                html: "<b>html</b>".to_string(),
                plain_preview: preview.to_string(),
            },
            captured_at: SystemTime::now(),
            pinned: false,
        }
    }

    fn make_image() -> ClipboardEntry {
        ClipboardEntry {
            id: EntryId(0),
            payload: ContentPayload::Image { data: vec![1, 2, 3], mime: ImageMime::Png },
            captured_at: SystemTime::now(),
            pinned: false,
        }
    }

    #[test]
    fn filter_empty_query_returns_all() {
        let entries = vec![make_text("a"), make_text("b"), make_image()];
        let result = filter("", &entries);
        assert_eq!(result, vec![0, 1, 2]);
    }

    #[test]
    fn filter_matches_case_insensitively() {
        let entries = vec![make_text("Hello World"), make_text("other")];
        let result = filter("hello", &entries);
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn filter_excludes_images_when_query_nonempty() {
        let entries = vec![make_text("hello"), make_image()];
        let result = filter("hello", &entries);
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn filter_no_matches_returns_empty() {
        let entries = vec![make_text("hello"), make_text("world")];
        let result = filter("xyz", &entries);
        assert!(result.is_empty());
    }

    #[test]
    fn filter_rich_text_matches_plain_preview() {
        let entries = vec![make_rich("Bold text content"), make_text("plain")];
        let result = filter("bold", &entries);
        assert_eq!(result, vec![0]);
    }
}
