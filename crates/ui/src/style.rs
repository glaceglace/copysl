pub const SPACE_XS: f32 = 4.0;
pub const SPACE_S:  f32 = 6.0;
pub const SPACE_M:  f32 = 10.0;
pub const SPACE_L:  f32 = 16.0;

pub const RADIUS_CARD:   u8 = 6;
pub const RADIUS_SEARCH: u8 = 6;
pub const RADIUS_BUTTON: u8 = 4;
pub const RADIUS_WINDOW: u8 = 8;

/// Height of one clipboard card at the given window height.
pub fn card_height(window_height: f32) -> f32 {
    window_height / 7.0
}

/// Width of the metadata (timestamp) column.
/// 70 px floor comfortably fits "23 h ago" in Small style.
pub fn meta_width(available: f32) -> f32 {
    (available * 0.20).max(70.0)
}

/// Width of the text-preview column.
pub fn text_width(available: f32, item_spacing: f32) -> f32 {
    let meta_w = meta_width(available);
    (available - meta_w - item_spacing).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::WINDOW_HEIGHT;

    // ── card_height ───────────────────────────────────────────────────────────

    #[test]
    fn card_height_reasonable_at_target_dimensions() {
        let h = card_height(WINDOW_HEIGHT);
        assert!(h > 80.0 && h < 120.0, "card_height = {h}");
    }

    #[test]
    fn card_height_never_below_reasonable_minimum() {
        for height in [480.0_f32, 600.0, 680.0, 1080.0] {
            let h = card_height(height);
            assert!(h > 60.0, "card_height({height}) = {h} too small");
        }
    }

    #[test]
    fn card_height_is_exactly_window_height_over_seven() {
        assert_eq!(card_height(680.0), 680.0 / 7.0);
        assert_eq!(card_height(480.0), 480.0 / 7.0);
    }

    #[test]
    fn card_height_scales_linearly() {
        // Doubling the window height doubles the card height
        assert_eq!(card_height(700.0), card_height(350.0) * 2.0);
    }

    // ── meta_width ────────────────────────────────────────────────────────────

    #[test]
    fn meta_width_clamps_to_minimum() {
        // 50 * 0.20 = 10 < 70  →  minimum kicks in
        assert_eq!(meta_width(50.0), 70.0);
        // 100 * 0.20 = 20 < 70  →  minimum kicks in
        assert_eq!(meta_width(100.0), 70.0);
    }

    #[test]
    fn meta_width_uses_percentage_above_minimum() {
        // 500 * 0.20 = 100 > 70  →  percentage wins
        let w = meta_width(500.0);
        assert!((w - 100.0).abs() < 0.01, "expected 100.0, got {w}");
    }

    #[test]
    fn meta_width_crossover_at_350() {
        // 350 * 0.20 = 70.0 exactly — both rules agree at the breakpoint
        let w = meta_width(350.0);
        assert!((w - 70.0).abs() < 0.01, "expected 70.0, got {w}");
    }

    // ── text_width ────────────────────────────────────────────────────────────

    #[test]
    fn column_widths_sum_to_available() {
        let avail = 400.0_f32;
        let spacing = 4.0_f32;
        let meta_w = meta_width(avail);
        let text_w = text_width(avail, spacing);
        assert!(
            (text_w + meta_w + spacing - avail).abs() < 0.01,
            "text_w={text_w} + meta_w={meta_w} + spacing={spacing} != avail={avail}"
        );
    }

    #[test]
    fn column_widths_sum_at_narrow_window() {
        // At the target window width minus outer padding: 480 - 20 = 460
        let avail = 460.0_f32;
        let spacing = 8.0_f32;
        let meta_w = meta_width(avail);
        let text_w = text_width(avail, spacing);
        assert!(
            (text_w + meta_w + spacing - avail).abs() < 0.01,
            "text_w={text_w} + meta_w={meta_w} + spacing={spacing} != avail={avail}"
        );
    }

    #[test]
    fn text_width_non_negative() {
        // When available is tiny the text column clamps to zero
        assert_eq!(text_width(10.0, 4.0), 0.0);
        assert_eq!(text_width(0.0, 0.0), 0.0);
    }

    #[test]
    fn text_width_grows_with_available() {
        // Wider window → wider text column
        assert!(text_width(600.0, 8.0) > text_width(400.0, 8.0));
    }
}
