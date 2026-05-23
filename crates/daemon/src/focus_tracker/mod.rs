use anyhow::Result;

#[cfg(not(test))]
pub mod x11;
#[cfg(not(test))]
pub mod wayland;

#[derive(Debug, Clone)]
pub enum PlatformFocusHandle {
    X11(u32),
    Wayland,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct FocusHandle {
    pub inner: PlatformFocusHandle,
}

impl FocusHandle {
    pub fn is_x11(&self) -> bool {
        matches!(self.inner, PlatformFocusHandle::X11(_))
    }
}

pub trait FocusTracker: Send + Sync {
    fn start(&self) -> Result<()>;
    fn current_focus(&self) -> Option<FocusHandle>;
    /// The window that had focus just before the most recent focus change.
    /// On X11/XWayland this is the app that was active before Copieur opened,
    /// allowing paste to be injected directly without waiting for the WM.
    /// Returns `None` on Wayland (security model blocks focus queries).
    fn previous_focus(&self) -> Option<FocusHandle> {
        None
    }
}

pub struct NoopFocusTracker;

impl FocusTracker for NoopFocusTracker {
    fn start(&self) -> Result<()> {
        Ok(())
    }
    fn current_focus(&self) -> Option<FocusHandle> {
        None
    }
}

pub fn create_focus_tracker() -> Box<dyn FocusTracker> {
    #[cfg(not(test))]
    {
        if std::env::var("WAYLAND_DISPLAY").is_ok() {
            Box::new(wayland::WaylandFocusTracker::new())
        } else if std::env::var("DISPLAY").is_ok() {
            Box::new(x11::X11FocusTracker::new())
        } else {
            Box::new(NoopFocusTracker)
        }
    }
    #[cfg(test)]
    {
        Box::new(NoopFocusTracker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_focus_tracker_returns_none() {
        let tracker = NoopFocusTracker;
        assert!(tracker.current_focus().is_none());
    }

    #[test]
    fn noop_focus_tracker_start_ok() {
        let tracker = NoopFocusTracker;
        assert!(tracker.start().is_ok());
    }

    #[test]
    fn create_focus_tracker_in_test_is_noop() {
        let tracker = create_focus_tracker();
        assert!(tracker.current_focus().is_none());
    }
}
