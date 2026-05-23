use super::{PasteBackend, PasteError};
use crate::focus_tracker::FocusHandle;

/// Wayland key injection via `wtype` (wtype package).
/// wtype works on most Wayland compositors and needs no daemon.
pub struct WtypeBackend;

impl WtypeBackend {
    pub fn new() -> Self {
        WtypeBackend
    }
}

impl Default for WtypeBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PasteBackend for WtypeBackend {
    fn is_available(&self) -> bool {
        #[cfg(not(test))]
        {
            std::process::Command::new("which")
                .arg("wtype")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        }
        #[cfg(test)]
        {
            false
        }
    }

    fn inject_paste(&self, _focus: Option<&FocusHandle>) -> Result<(), PasteError> {
        #[cfg(not(test))]
        {
            let status = std::process::Command::new("wtype")
                .args(["-k", "ctrl+v"])
                .status()
                .map_err(|e| PasteError::InjectionFailed(e.to_string()))?;
            if status.success() {
                Ok(())
            } else {
                Err(PasteError::InjectionFailed("wtype exited with error".into()))
            }
        }
        #[cfg(test)]
        {
            Err(PasteError::ToolNotFound)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wtype_is_not_available_in_tests() {
        let backend = WtypeBackend::new();
        assert!(!backend.is_available());
    }

    #[test]
    fn wtype_inject_returns_tool_not_found_in_tests() {
        let backend = WtypeBackend::new();
        assert!(matches!(backend.inject_paste(None), Err(PasteError::ToolNotFound)));
    }
}
