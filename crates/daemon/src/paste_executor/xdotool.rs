use super::{PasteBackend, PasteError};
use crate::focus_tracker::FocusHandle;

pub struct XdotoolBackend;

impl XdotoolBackend {
    pub fn new() -> Self {
        XdotoolBackend
    }
}

impl Default for XdotoolBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PasteBackend for XdotoolBackend {
    fn is_available(&self) -> bool {
        #[cfg(not(test))]
        {
            std::process::Command::new("which")
                .arg("xdotool")
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
            let status = std::process::Command::new("xdotool")
                .args(["key", "--clearmodifiers", "ctrl+v"])
                .status()
                .map_err(|e| PasteError::InjectionFailed(e.to_string()))?;
            if status.success() {
                Ok(())
            } else {
                Err(PasteError::InjectionFailed("xdotool failed".into()))
            }
        }
        #[cfg(test)]
        {
            Err(PasteError::ToolNotFound)
        }
    }
}
