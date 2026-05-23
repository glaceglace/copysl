use super::{PasteBackend, PasteError};
use crate::focus_tracker::FocusHandle;

/// Last-resort backend: all injection tools were unavailable or failed.
/// Content is already in the clipboard; injection is silently skipped.
/// No desktop notification is sent — the user can paste with Ctrl+V if needed.
pub struct NotificationFallbackBackend;

impl NotificationFallbackBackend {
    pub fn new() -> Self {
        NotificationFallbackBackend
    }
}

impl Default for NotificationFallbackBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PasteBackend for NotificationFallbackBackend {
    fn is_available(&self) -> bool {
        true
    }

    fn inject_paste(&self, _focus: Option<&FocusHandle>) -> Result<(), PasteError> {
        log::warn!(
            "Paste: no injection backend succeeded; content is in clipboard"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silent_fallback_is_always_available() {
        assert!(NotificationFallbackBackend::new().is_available());
    }

    #[test]
    fn silent_fallback_always_returns_ok() {
        let backend = NotificationFallbackBackend::new();
        assert!(backend.inject_paste(None).is_ok());
    }
}
