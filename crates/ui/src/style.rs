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
/// 44 px floor fits the longest compact timestamp ("25/12", "9.9h") in Small style.
pub fn meta_width(available: f32) -> f32 {
    (available * 0.12).max(44.0)
}

/// Width of the text-preview column.
pub fn text_width(available: f32, item_spacing: f32) -> f32 {
    let meta_w = meta_width(available);
    (available - meta_w - item_spacing).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window::compute_window_size;

    // ── card_height ───────────────────────────────────────────────────────────

    #[test]
    fn card_height_reasonable_at_target_dimensions() {
        // Use 1080p as a representative screen size.
        let (_, win_h) = compute_window_size(Some((1920, 1080)));
        let h = card_height(win_h);
        assert!(h > 60.0 && h < 130.0, "card_height = {h}");
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
        // 50 * 0.12 = 6 < 44  →  minimum kicks in
        assert_eq!(meta_width(50.0), 44.0);
        // 360 * 0.12 = 43.2 < 44  →  minimum kicks in
        assert_eq!(meta_width(360.0), 44.0);
    }

    #[test]
    fn meta_width_uses_percentage_above_minimum() {
        // 400 * 0.12 = 48.0 > 44  →  percentage wins
        let w = meta_width(400.0);
        assert!((w - 48.0).abs() < 0.01, "expected 48.0, got {w}");
    }

    #[test]
    fn meta_width_crossover_near_367() {
        // Crossover at 44 / 0.12 ≈ 366.7
        // 370 * 0.12 = 44.4 > 44  →  percentage wins
        assert!(meta_width(370.0) > 44.0);
        // 360 * 0.12 = 43.2 < 44  →  minimum
        assert_eq!(meta_width(360.0), 44.0);
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
