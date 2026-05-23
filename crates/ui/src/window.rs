use common::{Config, WindowPos};
use eframe::NativeOptions;
use egui::{Pos2, Vec2, ViewportBuilder};

const WINDOW_WIDTH: f32 = 380.0;
const WINDOW_HEIGHT: f32 = 520.0;
const SCREEN_WIDTH: f32 = 1920.0;
const SCREEN_HEIGHT: f32 = 1080.0;

pub fn build_native_options(cursor_pos: Option<(i32, i32)>, config: &Config) -> NativeOptions {
    let pos = compute_window_pos(cursor_pos, config);

    let mut viewport = ViewportBuilder::default()
        .with_decorations(false)
        .with_resizable(false)
        .with_always_on_top()
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

pub fn compute_window_pos(cursor_pos: Option<(i32, i32)>, config: &Config) -> Option<Pos2> {
    match &config.window_position {
        WindowPos::NearCursor => {
            let (x, y) = cursor_pos?;
            let x = (x as f32).clamp(0.0, SCREEN_WIDTH - WINDOW_WIDTH);
            let y = (y as f32).clamp(0.0, SCREEN_HEIGHT - WINDOW_HEIGHT);
            Some(Pos2::new(x, y))
        }
        WindowPos::Fixed(x, y) => Some(Pos2::new(*x as f32, *y as f32)),
    }
}

pub fn get_cursor_pos() -> Option<(i32, i32)> {
    #[cfg(not(test))]
    {
        get_cursor_pos_impl()
    }
    #[cfg(test)]
    {
        None
    }
}

#[cfg(not(test))]
fn get_cursor_pos_impl() -> Option<(i32, i32)> {
    // Try X11 via xcb
    if std::env::var("DISPLAY").is_ok() {
        if let Ok((conn, screen_num)) = xcb::Connection::connect(None) {
            let setup = conn.get_setup();
            let root = setup.roots().nth(screen_num as usize)?.root();
            let cookie = conn.send_request(&xcb::x::QueryPointer { window: root });
            if let Ok(reply) = conn.wait_for_reply(cookie) {
                return Some((reply.root_x() as i32, reply.root_y() as i32));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{Config, WindowPos};

    fn config_near_cursor() -> Config {
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

    #[test]
    fn near_cursor_no_cursor_returns_none() {
        let config = config_near_cursor();
        assert!(compute_window_pos(None, &config).is_none());
    }

    #[test]
    fn near_cursor_returns_cursor_position() {
        let config = config_near_cursor();
        let pos = compute_window_pos(Some((100, 200)), &config).unwrap();
        assert_eq!(pos.x, 100.0);
        assert_eq!(pos.y, 200.0);
    }

    #[test]
    fn near_cursor_clamps_to_screen_right_edge() {
        let config = config_near_cursor();
        // Place cursor at 1900, 100 — window would go off-screen to the right
        let pos = compute_window_pos(Some((1900, 100)), &config).unwrap();
        assert!(
            pos.x <= SCREEN_WIDTH - WINDOW_WIDTH,
            "x should be clamped: {}",
            pos.x
        );
        assert_eq!(pos.y, 100.0);
    }

    #[test]
    fn near_cursor_clamps_to_screen_bottom_edge() {
        let config = config_near_cursor();
        // Place cursor at 100, 1000 — window would go off-screen at bottom
        let pos = compute_window_pos(Some((100, 1000)), &config).unwrap();
        assert_eq!(pos.x, 100.0);
        assert!(
            pos.y <= SCREEN_HEIGHT - WINDOW_HEIGHT,
            "y should be clamped: {}",
            pos.y
        );
    }

    #[test]
    fn fixed_returns_exact_position() {
        let config = config_fixed(300, 400);
        let pos = compute_window_pos(None, &config).unwrap();
        assert_eq!(pos.x, 300.0);
        assert_eq!(pos.y, 400.0);
    }

    #[test]
    fn fixed_returns_exact_position_with_cursor() {
        let config = config_fixed(300, 400);
        // Cursor pos is irrelevant for Fixed mode
        let pos = compute_window_pos(Some((100, 100)), &config).unwrap();
        assert_eq!(pos.x, 300.0);
        assert_eq!(pos.y, 400.0);
    }

    #[test]
    fn get_cursor_pos_returns_none_in_tests() {
        assert!(get_cursor_pos().is_none());
    }
}
