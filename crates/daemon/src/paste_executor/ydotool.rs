use super::{PasteBackend, PasteError};
use crate::focus_tracker::FocusHandle;

pub struct YdotoolBackend;

impl YdotoolBackend {
    pub fn new() -> Self {
        YdotoolBackend
    }
}

impl Default for YdotoolBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PasteBackend for YdotoolBackend {
    fn is_available(&self) -> bool {
        #[cfg(not(test))]
        {
            std::process::Command::new("which")
                .arg("ydotool")
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
            // Key codes: 29 = Left Ctrl, 47 = V
            // Format: keycode:value where 1 = press, 0 = release
            let status = std::process::Command::new("ydotool")
                .args(["key", "29:1", "47:1", "47:0", "29:0"])
                .status()
                .map_err(|e| PasteError::InjectionFailed(e.to_string()))?;
            if status.success() {
                Ok(())
            } else {
                Err(PasteError::InjectionFailed("ydotool failed".into()))
            }
        }
        #[cfg(test)]
        {
            Err(PasteError::ToolNotFound)
        }
    }
}
