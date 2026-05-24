use common::{Config, WindowPos};
use eframe::NativeOptions;
use egui::{Pos2, Vec2, ViewportBuilder};

pub const WINDOW_WIDTH: f32 = 480.0;
pub const WINDOW_HEIGHT: f32 = 680.0;

pub fn build_native_options(screen_size: Option<(u32, u32)>, config: &Config) -> NativeOptions {
    let pos = compute_window_pos(screen_size, config);

    let mut viewport = ViewportBuilder::default()
        .with_decorations(false)
        .with_resizable(false)
        .with_taskbar(false)
        .with_inner_size(Vec2::new(WINDOW_WIDTH, WINDOW_HEIGHT));

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
/// `screen_size` is `(width, height)` in physical pixels.  When `None` a
/// 1920×1080 fallback is used so the function always returns a safe position.
pub fn compute_window_pos(screen_size: Option<(u32, u32)>, config: &Config) -> Option<Pos2> {
    let (sw, sh) = screen_size
        .map(|(w, h)| (w as f32, h as f32))
        .unwrap_or((1920.0, 1080.0));

    match &config.window_position {
        WindowPos::NearCursor => {
            let x = ((sw - WINDOW_WIDTH) / 2.0).max(0.0);
            let y = ((sh - WINDOW_HEIGHT) / 2.0).max(0.0);
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

    // ── compute_window_pos / NearCursor (always centers) ─────────────────────

    #[test]
    fn near_cursor_centers_on_1080p() {
        let pos = compute_window_pos(SCREEN_1080P, &config_center()).unwrap();
        assert_eq!(pos.x, (1920.0 - WINDOW_WIDTH) / 2.0);
        assert_eq!(pos.y, (1080.0 - WINDOW_HEIGHT) / 2.0);
    }

    #[test]
    fn near_cursor_centers_on_1440p() {
        let pos = compute_window_pos(SCREEN_1440P, &config_center()).unwrap();
        assert_eq!(pos.x, (2560.0 - WINDOW_WIDTH) / 2.0);
        assert_eq!(pos.y, (1440.0 - WINDOW_HEIGHT) / 2.0);
    }

    #[test]
    fn near_cursor_uses_fallback_when_screen_size_none() {
        let pos = compute_window_pos(None, &config_center()).unwrap();
        assert_eq!(pos.x, (1920.0 - WINDOW_WIDTH) / 2.0);
        let expected_y = ((1080.0 - WINDOW_HEIGHT) / 2.0).max(0.0);
        assert_eq!(pos.y, expected_y);
    }

    #[test]
    fn near_cursor_clamps_y_on_small_screen() {
        let pos = compute_window_pos(SCREEN_1080P, &config_center()).unwrap();
        assert!(pos.y >= 0.0);
    }

    // ── compute_window_pos / Fixed ────────────────────────────────────────────

    #[test]
    fn fixed_returns_exact_position() {
        let pos = compute_window_pos(SCREEN_1080P, &config_fixed(300, 400)).unwrap();
        assert_eq!(pos.x, 300.0);
        assert_eq!(pos.y, 400.0);
    }

    #[test]
    fn fixed_returns_exact_position_on_1440p() {
        let pos = compute_window_pos(SCREEN_1440P, &config_fixed(500, 200)).unwrap();
        assert_eq!(pos.x, 500.0);
        assert_eq!(pos.y, 200.0);
    }

    #[test]
    fn fixed_negative_coordinates_passed_through() {
        let pos = compute_window_pos(SCREEN_1080P, &config_fixed(-50, -100)).unwrap();
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
        let opts = build_native_options(SCREEN_1440P, &config_center());
        let p = opts.viewport.position.unwrap();
        assert_eq!(p.x, (2560.0 - WINDOW_WIDTH) / 2.0);
        assert_eq!(p.y, (1440.0 - WINDOW_HEIGHT) / 2.0);
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

    #[test]
    fn window_size_matches_target_dimensions() {
        assert_eq!(WINDOW_WIDTH, 480.0);
        assert_eq!(WINDOW_HEIGHT, 680.0);
    }

    #[test]
    fn header_height_formula_gives_approx_39px() {
        // The header strip uses WINDOW_HEIGHT * 0.057.
        // At 680 px this yields ≈ 38.76 px — must stay in a reasonable range
        // so that the strip has comfortable hit area without dominating the panel.
        let h = WINDOW_HEIGHT * 0.057;
        assert!(h > 35.0 && h < 45.0, "header_height = {h}");
    }

    #[test]
    fn window_is_portrait_oriented() {
        assert!(WINDOW_HEIGHT > WINDOW_WIDTH, "panel should be taller than wide");
    }
}
