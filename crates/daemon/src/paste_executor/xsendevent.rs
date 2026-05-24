use super::{PasteBackend, PasteError};
use crate::focus_tracker::FocusHandle;

/// X11 key injection via XTest FakeInput (xcb).
/// XTest-generated events are accepted by virtually all X11 applications;
/// they bypass the synthetic-event filter that XSendEvent events trigger.
pub struct XSendEventBackend;

impl XSendEventBackend {
    pub fn new() -> Self {
        XSendEventBackend
    }
}

impl Default for XSendEventBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PasteBackend for XSendEventBackend {
    fn is_available(&self) -> bool {
        true
    }

    fn inject_paste(&self, focus: Option<&FocusHandle>) -> Result<(), PasteError> {
        #[cfg(not(test))]
        {
            use xcb::{x, xtest, XidNew};
            use crate::focus_tracker::PlatformFocusHandle;

            let (conn, screen_num) = xcb::Connection::connect(None)
                .map_err(|e| PasteError::InjectionFailed(e.to_string()))?;
            let setup = conn.get_setup();
            let screen = setup
                .roots()
                .nth(screen_num as usize)
                .ok_or_else(|| PasteError::InjectionFailed("no screen".into()))?;
            let root = screen.root();

            // If the caller supplies a specific X11 window (the app that was focused
            // before Copysl opened), move input focus there explicitly.  This lets
            // us skip the 300 ms WM-delay entirely: we target the window directly
            // rather than waiting for the WM to return focus on its own.
            //
            // After SetInputFocus we do a round-trip GetInputFocus to ensure the X
            // server has processed the focus change before the XTest events arrive.
            if let Some(FocusHandle { inner: PlatformFocusHandle::X11(win_id) }) = focus {
                let target: x::Window = XidNew::new(*win_id);
                conn.send_request(&x::SetInputFocus {
                    revert_to: x::InputFocus::PointerRoot,
                    focus: target,
                    time: x::CURRENT_TIME,
                });
                conn.flush()
                    .map_err(|e| PasteError::InjectionFailed(e.to_string()))?;
                // Round-trip sync: guarantees focus is set before XTest events fire.
                let cookie = conn.send_request(&x::GetInputFocus {});
                conn.wait_for_reply(cookie)
                    .map_err(|e| PasteError::InjectionFailed(e.to_string()))?;
            }

            // Inject Ctrl+V via XTest FakeInput.
            // Keycodes are stable on PC keyboards under the standard X11 keymap:
            //   Control_L = 37, v = 55
            // Event type constants: KeyPress = 2, KeyRelease = 3
            for (event_type, keycode) in [(2u8, 37u8), (2, 55), (3, 55), (3, 37)] {
                conn.send_request(&xtest::FakeInput {
                    r#type: event_type,
                    detail: keycode,
                    time: x::CURRENT_TIME,
                    root,
                    root_x: 0,
                    root_y: 0,
                    deviceid: 0,
                });
            }
            conn.flush()
                .map_err(|e| PasteError::InjectionFailed(e.to_string()))?;
            Ok(())
        }
        #[cfg(test)]
        {
            let _ = focus;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xsendevent_is_available() {
        assert!(XSendEventBackend::new().is_available());
    }

    #[test]
    fn xsendevent_inject_succeeds_in_tests() {
        assert!(XSendEventBackend::new().inject_paste(None).is_ok());
    }
}
