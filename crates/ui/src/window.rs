use common::{Config, WindowPos};
use eframe::NativeOptions;
use egui::{Pos2, Vec2, ViewportBuilder};

/// Compute the logical window size from the screen resolution.
///
/// Width  ≈ 28 % of screen width,  clamped to [360, 600].
/// Height ≈ 65 % of screen height, clamped to [480, 860].
///
/// When `screen_size` is `None` the function falls back to 1920 × 1080.
pub fn compute_window_size(screen_size: Option<(u32, u32)>) -> (f32, f32) {
    let (sw, sh) = screen_size
        .map(|(w, h)| (w as f32, h as f32))
        .unwrap_or((1920.0, 1080.0));
    let w = (sw * 0.28).clamp(360.0, 600.0);
    let h = (sh * 0.65).clamp(480.0, 860.0);
    (w, h)
}

pub fn build_native_options(screen_size: Option<(u32, u32)>, config: &Config) -> NativeOptions {
    let (win_w, win_h) = compute_window_size(screen_size);
    let pos = compute_window_pos(screen_size, win_w, win_h, config);

    let mut viewport = ViewportBuilder::default()
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_inner_size(Vec2::new(win_w, win_h));

    if let Some(p) = pos {
        viewport = viewport.with_position(p);
    }

    NativeOptions {
        viewport,
        ..Default::default()
    }
}

/// Compute the window top-left corner.
///
/// `NearCursor` always centers the window — cursor tracking is unreliable on
/// Wayland and unnecessary now that the design calls for a centered launcher.
///
/// `screen_size` is `(width, height)` in logical pixels.  When `None` a
/// 1920×1080 fallback is used so the function always returns a safe position.
/// `win_w` / `win_h` are the computed window dimensions (from `compute_window_size`).
pub fn compute_window_pos(
    screen_size: Option<(u32, u32)>,
    win_w: f32,
    win_h: f32,
    config: &Config,
) -> Option<Pos2> {
    let (sw, sh) = screen_size
        .map(|(w, h)| (w as f32, h as f32))
        .unwrap_or((1920.0, 1080.0));

    match &config.window_position {
        WindowPos::NearCursor => {
            let x = ((sw - win_w) / 2.0).max(0.0);
            let y = ((sh - win_h) / 2.0).max(0.0);
            Some(Pos2::new(x, y))
        }
        WindowPos::Fixed(x, y) => Some(Pos2::new(*x as f32, *y as f32)),
    }
}

/// Query the current screen dimensions via XCB (X11 / XWayland).
///
/// Returns `(cursor_pos, screen_size)`.  Test builds always return `(None, None)`.
/// On pure Wayland without an X11 display, returns `(None, None)`.
pub fn get_display_info() -> (Option<(i32, i32)>, Option<(u32, u32)>) {
    #[cfg(not(test))]
    {
        get_display_info_impl()
    }
    #[cfg(test)]
    {
        (None, None)
    }
}

#[cfg(not(test))]
fn get_display_info_impl() -> (Option<(i32, i32)>, Option<(u32, u32)>) {
    if std::env::var("DISPLAY").is_err() {
        return (None, None);
    }
    let Ok((conn, screen_num)) = xcb::Connection::connect(None) else {
        return (None, None);
    };
    let setup = conn.get_setup();
    let Some(screen) = setup.roots().nth(screen_num as usize) else {
        return (None, None);
    };

    let screen_size = Some((
        screen.width_in_pixels() as u32,
        screen.height_in_pixels() as u32,
    ));

    let cookie = conn.send_request(&xcb::x::QueryPointer {
        window: screen.root(),
    });
    let cursor_pos = conn
        .wait_for_reply(cookie)
        .ok()
        .map(|r| (r.root_x() as i32, r.root_y() as i32));

    (cursor_pos, screen_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{Config, WindowPos};

    fn config_center() -> Config {
        Config {
            window_position: WindowPos::NearCursor,
            ..Config::default()
        }
    }

    fn config_fixed(x: i32, y: i32) -> Config {
        Config {
            window_position: WindowPos::Fixed(x, y),
            ..Config::default()
        }
    }

    const SCREEN_1080P: Option<(u32, u32)> = Some((1920, 1080));
    const SCREEN_1440P: Option<(u32, u32)> = Some((2560, 1440));

    // ── compute_window_size ───────────────────────────────────────────────────

    #[test]
    fn window_size_1080p_within_bounds() {
        let (w, h) = compute_window_size(SCREEN_1080P);
        assert!(w >= 360.0 && w <= 600.0, "width={w}");
        assert!(h >= 480.0 && h <= 860.0, "height={h}");
    }

    #[test]
    fn window_size_1440p_within_bounds() {
        let (w, h) = compute_window_size(SCREEN_1440P);
        assert!(w >= 360.0 && w <= 600.0, "width={w}");
        assert!(h >= 480.0 && h <= 860.0, "height={h}");
    }

    #[test]
    fn window_size_fallback_within_bounds() {
        let (w, h) = compute_window_size(None);
        assert!(w >= 360.0 && w <= 600.0, "width={w}");
        assert!(h >= 480.0 && h <= 860.0, "height={h}");
    }

    #[test]
    fn window_is_portrait_oriented() {
        let (w, h) = compute_window_size(SCREEN_1080P);
        assert!(h > w, "panel should be taller than wide: h={h}, w={w}");
    }

    #[test]
    fn window_size_small_screen_clamps_to_minimum() {
        let (w, h) = compute_window_size(Some((800, 600)));
        assert_eq!(w, 360.0);
        assert_eq!(h, 480.0);
    }

    #[test]
    fn window_size_4k_clamps_to_maximum() {
        let (w, h) = compute_window_size(Some((3840, 2160)));
        assert_eq!(w, 600.0);
        assert_eq!(h, 860.0);
    }

    #[test]
    fn window_size_scales_with_screen() {
        let (w1, h1) = compute_window_size(Some((1280, 720)));
        let (w2, h2) = compute_window_size(Some((1920, 1080)));
        assert!(w2 >= w1, "wider screen → wider window");
        assert!(h2 >= h1, "taller screen → taller window");
    }

    // ── compute_window_pos / NearCursor (always centers) ─────────────────────

    #[test]
    fn near_cursor_centers_on_1080p() {
        let (win_w, win_h) = compute_window_size(SCREEN_1080P);
        let pos = compute_window_pos(SCREEN_1080P, win_w, win_h, &config_center()).unwrap();
        assert_eq!(pos.x, (1920.0 - win_w) / 2.0);
        assert_eq!(pos.y, (1080.0 - win_h) / 2.0);
    }

    #[test]
    fn near_cursor_centers_on_1440p() {
        let (win_w, win_h) = compute_window_size(SCREEN_1440P);
        let pos = compute_window_pos(SCREEN_1440P, win_w, win_h, &config_center()).unwrap();
        assert_eq!(pos.x, (2560.0 - win_w) / 2.0);
        assert_eq!(pos.y, (1440.0 - win_h) / 2.0);
    }

    #[test]
    fn near_cursor_uses_fallback_when_screen_size_none() {
        let (win_w, win_h) = compute_window_size(None);
        let pos = compute_window_pos(None, win_w, win_h, &config_center()).unwrap();
        assert_eq!(pos.x, (1920.0 - win_w) / 2.0);
        let expected_y = ((1080.0 - win_h) / 2.0).max(0.0);
        assert_eq!(pos.y, expected_y);
    }

    #[test]
    fn near_cursor_clamps_y_on_small_screen() {
        let (win_w, win_h) = compute_window_size(SCREEN_1080P);
        let pos = compute_window_pos(SCREEN_1080P, win_w, win_h, &config_center()).unwrap();
        assert!(pos.y >= 0.0);
    }

    // ── compute_window_pos / Fixed ────────────────────────────────────────────

    #[test]
    fn fixed_returns_exact_position() {
        let (win_w, win_h) = compute_window_size(SCREEN_1080P);
        let pos = compute_window_pos(SCREEN_1080P, win_w, win_h, &config_fixed(300, 400)).unwrap();
        assert_eq!(pos.x, 300.0);
        assert_eq!(pos.y, 400.0);
    }

    #[test]
    fn fixed_returns_exact_position_on_1440p() {
        let (win_w, win_h) = compute_window_size(SCREEN_1440P);
        let pos = compute_window_pos(SCREEN_1440P, win_w, win_h, &config_fixed(500, 200)).unwrap();
        assert_eq!(pos.x, 500.0);
        assert_eq!(pos.y, 200.0);
    }

    #[test]
    fn fixed_negative_coordinates_passed_through() {
        let (win_w, win_h) = compute_window_size(SCREEN_1080P);
        let pos = compute_window_pos(SCREEN_1080P, win_w, win_h, &config_fixed(-50, -100)).unwrap();
        assert_eq!(pos.x, -50.0);
        assert_eq!(pos.y, -100.0);
    }

    // ── get_display_info ──────────────────────────────────────────────────────

    #[test]
    fn get_display_info_returns_none_in_tests() {
        let (cursor, screen) = get_display_info();
        assert!(cursor.is_none());
        assert!(screen.is_none());
    }

    // ── build_native_options ──────────────────────────────────────────────────

    #[test]
    fn build_native_options_center_sets_position_on_1440p() {
        let (win_w, win_h) = compute_window_size(SCREEN_1440P);
        let opts = build_native_options(SCREEN_1440P, &config_center());
        let p = opts.viewport.position.unwrap();
        assert_eq!(p.x, (2560.0 - win_w) / 2.0);
        assert_eq!(p.y, (1440.0 - win_h) / 2.0);
    }

    #[test]
    fn build_native_options_fixed_sets_position() {
        let opts = build_native_options(SCREEN_1080P, &config_fixed(200, 300));
        let p = opts.viewport.position.unwrap();
        assert_eq!(p.x, 200.0);
        assert_eq!(p.y, 300.0);
    }

    #[test]
    fn build_native_options_no_override_redirect() {
        let opts = build_native_options(SCREEN_1080P, &config_fixed(0, 0));
        assert_ne!(opts.viewport.override_redirect, Some(true));
    }

}
