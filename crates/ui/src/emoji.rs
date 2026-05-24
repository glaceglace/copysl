use std::collections::HashMap;

/// Renders text that may contain emoji as color images inline with plain text.
pub struct EmojiRenderer {
    font_data: Option<Vec<u8>>,
    /// Cached textures keyed by Unicode codepoint. `None` = no glyph found.
    cache: HashMap<u32, Option<egui::TextureHandle>>,
}

impl EmojiRenderer {
    pub fn new() -> Self {
        EmojiRenderer {
            font_data: load_color_emoji_font(),
            cache: HashMap::new(),
        }
    }

    /// Render `text` inline, replacing emoji codepoints with color PNG images.
    /// Falls back to plain label rendering for characters with no raster glyph.
    /// Returns the outer `Response` so callers can chain `.on_hover_text()`.
    pub fn render_line(&mut self, ui: &mut egui::Ui, text: &str) -> egui::Response {
        if !has_emoji(text) {
            return ui.add(egui::Label::new(text).truncate());
        }

        let font_size = ui.text_style_height(&egui::TextStyle::Body);
        let resp = ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for run in split_runs(text) {
                match run {
                    Run::Text(s) => {
                        ui.label(s);
                    }
                    Run::Emoji(cp) => {
                        let c = char::from_u32(cp).unwrap_or('?');
                        if let Some(tex) = self.get_texture(ui.ctx(), cp) {
                            let sz = egui::vec2(font_size, font_size);
                            ui.add(egui::Image::new(
                                egui::load::SizedTexture::new(tex.id(), sz),
                            ));
                        } else {
                            ui.label(c.to_string());
                        }
                    }
                }
            }
        });
        resp.response
    }

    fn get_texture(&mut self, ctx: &egui::Context, cp: u32) -> Option<&egui::TextureHandle> {
        if !self.cache.contains_key(&cp) {
            let tex = self.extract_texture(ctx, cp);
            self.cache.insert(cp, tex);
        }
        self.cache.get(&cp)?.as_ref()
    }

    fn extract_texture(&self, ctx: &egui::Context, cp: u32) -> Option<egui::TextureHandle> {
        let data = self.font_data.as_ref()?;
        let c = char::from_u32(cp)?;
        let face = ttf_parser::Face::parse(data, 0).ok()?;
        let glyph_id = face.glyph_index(c)?;
        let raster = face.glyph_raster_image(glyph_id, 72)?;
        let img = image::load_from_memory(raster.data).ok()?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let color_img = egui::ColorImage::from_rgba_unmultiplied(
            [w as usize, h as usize],
            rgba.as_raw(),
        );
        Some(ctx.load_texture(
            format!("emoji_{cp:x}"),
            color_img,
            egui::TextureOptions::LINEAR,
        ))
    }
}

impl Default for EmojiRenderer {
    fn default() -> Self {
        Self::new()
    }
}

enum Run {
    Text(String),
    Emoji(u32),
}

/// Split `text` into alternating plain-text and emoji runs.
/// Variation selectors (U+FE0E/FE0F) and ZWJ (U+200D) are silently dropped
/// since we don't attempt to compose ZWJ sequences.
fn split_runs(text: &str) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    let mut buf = String::new();

    for c in text.chars() {
        match c as u32 {
            0xFE0E | 0xFE0F | 0x200D | 0x20E3 => {}
            cp if is_emoji(cp) => {
                if !buf.is_empty() {
                    runs.push(Run::Text(std::mem::take(&mut buf)));
                }
                runs.push(Run::Emoji(cp));
            }
            _ => buf.push(c),
        }
    }
    if !buf.is_empty() {
        runs.push(Run::Text(buf));
    }
    runs
}

pub fn has_emoji(text: &str) -> bool {
    text.chars()
        .any(|c| matches!(c as u32, cp if is_emoji(cp)))
}

/// Returns true for codepoints in the standard Unicode emoji blocks.
pub fn is_emoji(cp: u32) -> bool {
    matches!(
        cp,
        0x1F300..=0x1F9FF   // Misc Symbols, Emoticons, Transport, Symbols & Pictographs
        | 0x1FA00..=0x1FAFF // Chess symbols, Extended Pictographs
        | 0x2300..=0x27BF   // Misc Technical, Symbols, Dingbats (covers ⏰🕐☀❤✅ etc.)
        | 0x2934 | 0x2935
        | 0x2B05..=0x2B07
        | 0x2B1B | 0x2B1C | 0x2B50 | 0x2B55
        | 0x3030 | 0x303D | 0x3297 | 0x3299
        | 0x1F004 | 0x1F0CF
    )
}

/// Probe known filesystem paths for a CBDT/CBLC color emoji TTF.
/// The Google Noto Color Emoji font (NotoColorEmoji-Regular.ttf) is the target;
/// the COLRv1 variant (Noto-COLRv1.ttf) is intentionally skipped since
/// ttf-parser's raster-image extraction only supports CBDT/CBLC.
fn load_color_emoji_font() -> Option<Vec<u8>> {
    let system_candidates: &[&str] = &[
        "/usr/share/fonts/google-noto-color-emoji-fonts/NotoColorEmoji-Regular.ttf",
        "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
        "/usr/share/fonts/noto/NotoColorEmoji.ttf",
        "/usr/share/fonts/TTF/NotoColorEmoji.ttf",
        "/usr/share/fonts/noto-color-emoji/NotoColorEmoji.ttf",
    ];
    for path in system_candidates {
        if let Ok(data) = std::fs::read(path) {
            return Some(data);
        }
    }
    // Fall back to a user-local copy (e.g. ~/Downloads/…)
    if let Ok(home) = std::env::var("HOME") {
        let path = format!("{home}/Downloads/Noto_Color_Emoji/NotoColorEmoji-Regular.ttf");
        if let Ok(data) = std::fs::read(&path) {
            return Some(data);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_emoji_known_emoticons() {
        assert!(is_emoji(0x1F600)); // 😀
        assert!(is_emoji(0x1F64F)); // 🙏
        assert!(is_emoji(0x1F525)); // 🔥
    }

    #[test]
    fn is_emoji_rejects_ascii() {
        for cp in 0x0020u32..=0x007E {
            assert!(!is_emoji(cp), "ASCII char U+{cp:04X} should not be emoji");
        }
    }

    #[test]
    fn is_emoji_misc_symbols() {
        assert!(is_emoji(0x2600)); // ☀
        assert!(is_emoji(0x2764)); // ❤
    }

    #[test]
    fn has_emoji_detects() {
        assert!(has_emoji("hello 😀"));
        assert!(!has_emoji("hello world"));
        assert!(!has_emoji(""));
    }

    #[test]
    fn split_runs_plain_text() {
        let runs = split_runs("hello");
        assert_eq!(runs.len(), 1);
        assert!(matches!(&runs[0], Run::Text(s) if s == "hello"));
    }

    #[test]
    fn split_runs_emoji_only() {
        let runs = split_runs("😀");
        assert_eq!(runs.len(), 1);
        assert!(matches!(runs[0], Run::Emoji(0x1F600)));
    }

    #[test]
    fn split_runs_mixed() {
        let runs = split_runs("hi 😀 bye");
        assert_eq!(runs.len(), 3);
        assert!(matches!(&runs[0], Run::Text(s) if s == "hi "));
        assert!(matches!(runs[1], Run::Emoji(0x1F600)));
        assert!(matches!(&runs[2], Run::Text(s) if s == " bye"));
    }

    #[test]
    fn split_runs_drops_variation_selector() {
        // ☀️ = U+2600 U+FE0F — FE0F should be silently dropped
        let runs = split_runs("\u{2600}\u{FE0F}");
        assert_eq!(runs.len(), 1);
        assert!(matches!(runs[0], Run::Emoji(0x2600)));
    }
}
