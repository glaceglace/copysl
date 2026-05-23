#[cfg(not(test))]
use std::sync::{Arc, Mutex};
#[cfg(not(test))]
use anyhow::Result;
#[cfg(not(test))]
use super::{FocusHandle, FocusTracker, PlatformFocusHandle};

#[cfg(not(test))]
pub struct WaylandFocusTracker {
    #[allow(dead_code)]
    _focused: Arc<Mutex<Option<()>>>,
}

#[cfg(not(test))]
impl WaylandFocusTracker {
    pub fn new() -> Self {
        WaylandFocusTracker {
            _focused: Arc::new(Mutex::new(None)),
        }
    }
}

#[cfg(not(test))]
impl Default for WaylandFocusTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(test))]
impl FocusTracker for WaylandFocusTracker {
    fn start(&self) -> Result<()> {
        Ok(())
    }

    fn current_focus(&self) -> Option<FocusHandle> {
        Some(FocusHandle {
            inner: PlatformFocusHandle::Wayland,
        })
    }
}
