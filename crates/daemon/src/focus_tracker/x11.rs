#[cfg(not(test))]
use std::sync::{Arc, Mutex};
#[cfg(not(test))]
use anyhow::Result;
#[cfg(not(test))]
use super::{FocusHandle, FocusTracker, PlatformFocusHandle};

#[cfg(not(test))]
pub struct X11FocusTracker {
    /// The X11 window that currently has focus.
    current: Arc<Mutex<Option<u32>>>,
    /// The X11 window that had focus just before `current` — i.e. the app
    /// that was active before Copysl opened.  Used by the paste executor to
    /// inject Ctrl+V directly into that window without any WM delay.
    previous: Arc<Mutex<Option<u32>>>,
}

#[cfg(not(test))]
impl X11FocusTracker {
    pub fn new() -> Self {
        X11FocusTracker {
            current: Arc::new(Mutex::new(None)),
            previous: Arc::new(Mutex::new(None)),
        }
    }
}

#[cfg(not(test))]
impl Default for X11FocusTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(test))]
impl FocusTracker for X11FocusTracker {
    fn start(&self) -> Result<()> {
        let current = Arc::clone(&self.current);
        let previous = Arc::clone(&self.previous);
        std::thread::spawn(move || {
            if let Ok((conn, screen_num)) = xcb::Connection::connect(None) {
                let setup = conn.get_setup();
                let screen = setup.roots().nth(screen_num as usize).unwrap();
                let root = screen.root();

                let active_window_atom = {
                    let cookie = conn.send_request(&xcb::x::InternAtom {
                        only_if_exists: false,
                        name: b"_NET_ACTIVE_WINDOW",
                    });
                    conn.wait_for_reply(cookie)
                        .map(|r| r.atom())
                        .unwrap_or(xcb::x::ATOM_NONE)
                };

                conn.send_request(&xcb::x::ChangeWindowAttributes {
                    window: root,
                    value_list: &[xcb::x::Cw::EventMask(xcb::x::EventMask::PROPERTY_CHANGE)],
                });
                conn.flush().ok();

                loop {
                    match conn.wait_for_event() {
                        Ok(xcb::Event::X(xcb::x::Event::PropertyNotify(event)))
                            if event.atom() == active_window_atom =>
                        {
                            let cookie = conn.send_request(&xcb::x::GetProperty {
                                delete: false,
                                window: root,
                                property: active_window_atom,
                                r#type: xcb::x::ATOM_WINDOW,
                                long_offset: 0,
                                long_length: 1,
                            });
                            if let Ok(reply) = conn.wait_for_reply(cookie) {
                                let value: &[u32] = reply.value();
                                if let Some(&win_id) = value.first() {
                                    let mut cur = current.lock().unwrap();
                                    let mut prev = previous.lock().unwrap();
                                    *prev = *cur;
                                    *cur = Some(win_id);
                                }
                            }
                        }
                        Err(_) => break,
                        _ => {}
                    }
                }
            }
        });
        Ok(())
    }

    fn current_focus(&self) -> Option<FocusHandle> {
        self.current.lock().unwrap().map(|id| FocusHandle {
            inner: PlatformFocusHandle::X11(id),
        })
    }

    fn previous_focus(&self) -> Option<FocusHandle> {
        self.previous.lock().unwrap().map(|id| FocusHandle {
            inner: PlatformFocusHandle::X11(id),
        })
    }
}
