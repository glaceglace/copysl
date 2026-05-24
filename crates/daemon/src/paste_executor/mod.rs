use anyhow::Result;
use thiserror::Error;
use common::ClipboardEntry;
#[cfg(not(test))]
use common::ContentPayload;
use crate::focus_tracker::FocusHandle;

pub mod xdotool;
pub mod xsendevent;
pub mod ydotool;
pub mod wtype;
pub mod notification_fallback;

#[derive(Debug, Error)]
pub enum PasteError {
    #[error("Tool not found")]
    ToolNotFound,
    #[error("Injection failed: {0}")]
    InjectionFailed(String),
    #[error("No focused window")]
    NoFocusedWindow,
}

pub trait PasteBackend: Send + Sync {
    fn inject_paste(&self, focus: Option<&FocusHandle>) -> Result<(), PasteError>;
    fn is_available(&self) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayServer {
    X11,
    Wayland,
    Unknown,
}

pub fn detect_display_server() -> DisplayServer {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        DisplayServer::Wayland
    } else if std::env::var("DISPLAY").is_ok() {
        DisplayServer::X11
    } else {
        DisplayServer::Unknown
    }
}

pub fn select_backend(display: DisplayServer) -> Box<dyn PasteBackend> {
    #[cfg(not(test))]
    {
        use crate::paste_executor::{
            xdotool::XdotoolBackend,
            xsendevent::XSendEventBackend,
            ydotool::YdotoolBackend,
            notification_fallback::NotificationFallbackBackend,
        };
        match display {
            DisplayServer::X11 => {
                let xdotool = XdotoolBackend::new();
                if xdotool.is_available() {
                    log::info!("Paste backend: xdotool (X11)");
                    Box::new(xdotool)
                } else {
                    log::info!("Paste backend: xcb XTest (X11 fallback)");
                    Box::new(XSendEventBackend::new())
                }
            }
            _ => {
                // Wayland or unknown:
                // try ydotool → wtype → xdotool → xcb XTest (XWayland) → silent
                let ydotool = YdotoolBackend::new();
                if ydotool.is_available() {
                    log::info!("Paste backend: ydotool (Wayland)");
                    return Box::new(ydotool);
                }
                let wtype = wtype::WtypeBackend::new();
                if wtype.is_available() {
                    log::info!("Paste backend: wtype (Wayland fallback)");
                    return Box::new(wtype);
                }
                let xdotool = XdotoolBackend::new();
                if xdotool.is_available() {
                    log::info!("Paste backend: xdotool via XWayland (Wayland fallback)");
                    return Box::new(xdotool);
                }
                // When XWayland is running (DISPLAY is set), xcb XTest can inject
                // into X11/XWayland applications.  Native Wayland apps are not
                // reachable this way, but it is better than doing nothing.
                if std::env::var("DISPLAY").is_ok() {
                    log::info!("Paste backend: xcb XTest via XWayland (DISPLAY={:?})",
                        std::env::var("DISPLAY").unwrap_or_default());
                    return Box::new(XSendEventBackend::new());
                }
                log::warn!(
                    "Paste backend: no injection tool found \
                     (install xdotool, ydotool, or wtype); paste will be silent"
                );
                Box::new(NotificationFallbackBackend::new())
            }
        }
    }
    #[cfg(test)]
    {
        let _ = display;
        Box::new(MockPasteBackend::available())
    }
}

/// Try to set clipboard text via `wl-copy` (wl-clipboard package).
/// wl-copy forks a child daemon that serves the clipboard indefinitely.
#[cfg(not(test))]
fn try_wl_copy(text: &str) -> bool {
    use std::io::Write;
    let mut child = match std::process::Command::new("wl-copy")
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(text.as_bytes()).is_err() {
            return false;
        }
    }
    true
}

/// Try to set clipboard text via `xclip` (xclip package).
/// xclip stays running and serves the clipboard until another app takes ownership.
#[cfg(not(test))]
fn try_xclip(text: &str) -> bool {
    use std::io::Write;
    let mut child = match std::process::Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    if let Some(mut stdin) = child.stdin.take() {
        if stdin.write_all(text.as_bytes()).is_err() {
            return false;
        }
    }
    true
}

pub struct PasteExecutor {
    backend: Box<dyn PasteBackend>,
    focus_tracker: std::sync::Arc<dyn crate::focus_tracker::FocusTracker>,
    /// Shared config — read via `blocking_read()` inside `spawn_blocking`.
    /// Using the same Arc as the IpcServer means config changes (e.g. paste
    /// delay updated in settings) take effect on the next paste.
    config: std::sync::Arc<tokio::sync::RwLock<common::Config>>,
}

impl PasteExecutor {
    pub fn new(
        backend: Box<dyn PasteBackend>,
        focus_tracker: std::sync::Arc<dyn crate::focus_tracker::FocusTracker>,
        config: std::sync::Arc<tokio::sync::RwLock<common::Config>>,
    ) -> Self {
        PasteExecutor {
            backend,
            focus_tracker,
            config,
        }
    }

    pub fn paste(&self, entry: &ClipboardEntry) -> Result<()> {
        #[cfg(not(test))]
        {
            // ── Step 1: write content to the clipboard ───────────────────────
            // Prefer a persistent daemon (wl-copy / xclip) so the data outlives
            // this function.  arboard's backend is served by a thread that dies
            // on drop; we keep it alive until after Ctrl+V succeeds.
            let used_persistent = match &entry.payload {
                ContentPayload::PlainText(t) => try_wl_copy(t) || try_xclip(t),
                ContentPayload::RichText { plain_preview, .. } => {
                    try_wl_copy(plain_preview) || try_xclip(plain_preview)
                }
                ContentPayload::Image { .. } => false,
            };

            let mut clipboard_guard: Option<arboard::Clipboard> = None;
            if !used_persistent {
                let mut clipboard = arboard::Clipboard::new()?;
                match &entry.payload {
                    ContentPayload::PlainText(t) => {
                        clipboard.set_text(t.clone())?;
                    }
                    ContentPayload::RichText { plain_preview, .. } => {
                        clipboard.set_text(plain_preview.clone())?;
                    }
                    ContentPayload::Image { data, .. } => {
                        let img = image::load_from_memory(data)
                            .map_err(|e| anyhow::anyhow!("image decode: {e}"))?;
                        let rgba = img.to_rgba8();
                        let (width, height) = rgba.dimensions();
                        clipboard.set_image(arboard::ImageData {
                            width: width as usize,
                            height: height as usize,
                            bytes: rgba.into_raw().into(),
                        })?;
                    }
                }
                clipboard_guard = Some(clipboard);
                log::debug!("Paste: clipboard set via arboard");
            } else {
                log::debug!("Paste: clipboard set via persistent daemon");
            }

            // ── Step 2: decide whether to delay and which window to target ───
            // On X11/XWayland we know exactly which window was focused before
            // Copysl opened (stored in previous_focus).  We can set focus there
            // directly via XSetInputFocus, so no WM round-trip delay is needed.
            //
            // On Wayland + ydotool there is no per-window targeting API; ydotool
            // injects into whatever window currently has focus.  We must wait for
            // the WM to return focus to the previous app after our window closes.
            let prev_focus = self.focus_tracker.previous_focus();
            let has_x11_target = prev_focus.as_ref().map(|f| f.is_x11()).unwrap_or(false);

            if has_x11_target {
                log::info!("Paste: instant (X11 direct inject, no delay)");
            } else {
                let delay_ms = self.config.blocking_read().paste_delay_ms as u64;
                log::info!("Paste: delayed ({delay_ms}ms wait for WM focus transfer)");
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }

            // ── Step 3: inject Ctrl+V into the target window ─────────────────
            log::debug!("Paste: injecting Ctrl+V");
            let focus = if has_x11_target {
                prev_focus
            } else {
                self.focus_tracker.current_focus()
            };
            if let Err(e) = self.backend.inject_paste(focus.as_ref()) {
                log::warn!("Paste backend failed: {e}; content is in clipboard");
            } else {
                log::info!("Paste: key injection succeeded");
            }

            // ── Step 4: keep arboard alive while the target app reads ────────
            if clipboard_guard.is_some() {
                log::debug!("Paste: holding arboard alive for 300ms post-inject");
                std::thread::sleep(std::time::Duration::from_millis(300));
            }
            // clipboard_guard dropped here
        }
        #[cfg(test)]
        {
            let _ = &entry.payload;
            let focus = self.focus_tracker.current_focus();
            if let Err(e) = self.backend.inject_paste(focus.as_ref()) {
                let _ = e;
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MockPasteBackend — used by tests (always compiled in, used only in tests)
// ---------------------------------------------------------------------------

pub struct MockPasteBackend {
    available: bool,
    should_fail: bool,
}

impl MockPasteBackend {
    pub fn available() -> Self {
        MockPasteBackend {
            available: true,
            should_fail: false,
        }
    }
    pub fn unavailable() -> Self {
        MockPasteBackend {
            available: false,
            should_fail: true,
        }
    }
    pub fn failing() -> Self {
        MockPasteBackend {
            available: true,
            should_fail: true,
        }
    }
}

impl PasteBackend for MockPasteBackend {
    fn inject_paste(&self, _focus: Option<&FocusHandle>) -> Result<(), PasteError> {
        if self.should_fail {
            Err(PasteError::InjectionFailed("mock failure".to_string()))
        } else {
            Ok(())
        }
    }
    fn is_available(&self) -> bool {
        self.available
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::focus_tracker::NoopFocusTracker;
    use std::sync::Arc;

    fn default_test_config() -> Arc<tokio::sync::RwLock<common::Config>> {
        Arc::new(tokio::sync::RwLock::new(common::Config::default()))
    }

    #[test]
    fn select_backend_in_test_returns_mock() {
        let backend = select_backend(DisplayServer::X11);
        assert!(backend.is_available());
    }

    #[test]
    fn mock_available_backend_succeeds() {
        let backend = MockPasteBackend::available();
        assert!(backend.inject_paste(None).is_ok());
    }

    #[test]
    fn mock_failing_backend_fails() {
        let backend = MockPasteBackend::failing();
        assert!(backend.inject_paste(None).is_err());
    }

    #[test]
    fn paste_silently_succeeds_on_backend_failure() {
        let failing_backend = Box::new(MockPasteBackend::failing());
        let focus_tracker =
            Arc::new(NoopFocusTracker) as Arc<dyn crate::focus_tracker::FocusTracker>;
        let executor = PasteExecutor::new(failing_backend, focus_tracker, default_test_config());

        let entry = common::ClipboardEntry {
            id: common::EntryId(1),
            payload: common::ContentPayload::PlainText("test".to_string()),
            captured_at: std::time::SystemTime::now(),
            pinned: false,
        };

        assert!(executor.paste(&entry).is_ok());
    }

    #[test]
    fn select_backend_wayland_returns_available_backend() {
        let backend = select_backend(DisplayServer::Wayland);
        assert!(backend.is_available());
    }

    #[test]
    fn select_backend_unknown_returns_available_backend() {
        let backend = select_backend(DisplayServer::Unknown);
        assert!(backend.is_available());
    }

    #[test]
    fn detect_display_server_returns_something() {
        let _ = detect_display_server();
    }

    #[test]
    fn mock_unavailable_backend_is_not_available() {
        let backend = MockPasteBackend::unavailable();
        assert!(!backend.is_available());
    }

    #[test]
    fn paste_with_available_backend_succeeds() {
        let backend = Box::new(MockPasteBackend::available());
        let focus_tracker =
            Arc::new(NoopFocusTracker) as Arc<dyn crate::focus_tracker::FocusTracker>;
        let executor = PasteExecutor::new(backend, focus_tracker, default_test_config());

        let entry = common::ClipboardEntry {
            id: common::EntryId(2),
            payload: common::ContentPayload::PlainText("hello".to_string()),
            captured_at: std::time::SystemTime::now(),
            pinned: false,
        };

        assert!(executor.paste(&entry).is_ok());
    }

    #[test]
    fn paste_image_entry_succeeds_in_tests() {
        let backend = Box::new(MockPasteBackend::available());
        let focus_tracker =
            Arc::new(NoopFocusTracker) as Arc<dyn crate::focus_tracker::FocusTracker>;
        let executor = PasteExecutor::new(backend, focus_tracker, default_test_config());

        let entry = common::ClipboardEntry {
            id: common::EntryId(3),
            payload: common::ContentPayload::Image {
                data: vec![0u8; 4],
                mime: common::ImageMime::Png,
            },
            captured_at: std::time::SystemTime::now(),
            pinned: false,
        };

        assert!(executor.paste(&entry).is_ok());
    }

    #[test]
    fn paste_rich_text_entry_succeeds_in_tests() {
        let backend = Box::new(MockPasteBackend::available());
        let focus_tracker =
            Arc::new(NoopFocusTracker) as Arc<dyn crate::focus_tracker::FocusTracker>;
        let executor = PasteExecutor::new(backend, focus_tracker, default_test_config());

        let entry = common::ClipboardEntry {
            id: common::EntryId(4),
            payload: common::ContentPayload::RichText {
                html: "<b>hello</b>".to_string(),
                plain_preview: "hello".to_string(),
            },
            captured_at: std::time::SystemTime::now(),
            pinned: false,
        };

        assert!(executor.paste(&entry).is_ok());
    }
}
