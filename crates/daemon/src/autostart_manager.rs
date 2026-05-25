use std::path::PathBuf;
use anyhow::Result;

#[allow(dead_code)]
pub fn desktop_file_path() -> PathBuf {
    dirs::config_dir()
        .expect("Could not determine config directory")
        .join("autostart")
        .join("copysl.desktop")
}

pub struct AutostartManager {
    base_dir: PathBuf,
}

impl AutostartManager {
    /// Production constructor — uses real ~/.config/autostart/
    pub fn new() -> Self {
        AutostartManager {
            base_dir: dirs::config_dir()
                .expect("Could not determine config directory")
                .join("autostart"),
        }
    }

    /// Test constructor — uses a provided base directory
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        AutostartManager { base_dir }
    }

    fn file_path(&self) -> PathBuf {
        self.base_dir.join("copysl.desktop")
    }

    pub fn enable(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base_dir)?;
        let exe = std::env::current_exe()
            .map_err(|e| anyhow::anyhow!("Could not determine executable path: {e}"))?;
        let exe_str = exe.to_str()
            .ok_or_else(|| anyhow::anyhow!("Executable path contains non-UTF-8 characters"))?;
        // Escape backslash and double-quote per the Desktop Entry spec so that
        // paths containing those characters remain valid inside a quoted Exec argument.
        let exec_path = exe_str.replace('\\', "\\\\").replace('"', "\\\"");
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=Copysl\nExec=\"{exec_path}\" --daemon\nTerminal=false\nHidden=false\nNoDisplay=true\nX-GNOME-Autostart-enabled=true\n"
        );
        // Guard: skip the write if the file already has the correct content.
        // This avoids overwriting user customisations on every daemon restart
        // while still self-healing when the executable path changes after a reinstall.
        if std::fs::read_to_string(self.file_path()).ok().as_deref() == Some(&content) {
            return Ok(());
        }
        std::fs::write(self.file_path(), content)?;
        Ok(())
    }

    pub fn disable(&self) -> Result<()> {
        match std::fs::remove_file(self.file_path()) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

impl Default for AutostartManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn enable_writes_file_with_exec_path() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        manager.enable().unwrap();
        let content = std::fs::read_to_string(manager.file_path()).unwrap();
        assert!(content.contains("[Desktop Entry]"));
        assert!(content.contains("--daemon"));
        assert!(content.contains("X-GNOME-Autostart-enabled=true"));
    }

    #[test]
    fn enable_quotes_exec_path() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        manager.enable().unwrap();
        let content = std::fs::read_to_string(manager.file_path()).unwrap();
        assert!(content.contains("Exec=\""), "Exec path must be double-quoted");
    }

    #[test]
    fn enable_includes_terminal_false() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        manager.enable().unwrap();
        let content = std::fs::read_to_string(manager.file_path()).unwrap();
        assert!(content.contains("Terminal=false"));
    }

    #[test]
    fn enable_includes_no_display_true() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        manager.enable().unwrap();
        let content = std::fs::read_to_string(manager.file_path()).unwrap();
        assert!(content.contains("NoDisplay=true"));
    }

    #[test]
    fn enable_is_no_op_when_content_unchanged() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        manager.enable().unwrap();
        let content_first = std::fs::read_to_string(manager.file_path()).unwrap();
        manager.enable().unwrap(); // second call — same exe, should be a no-op
        let content_second = std::fs::read_to_string(manager.file_path()).unwrap();
        assert_eq!(content_first, content_second);
    }

    #[test]
    fn enable_overwrites_when_existing_content_differs() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        // Simulate a stale desktop file left by an older install.
        std::fs::write(manager.file_path(), "stale content").unwrap();
        manager.enable().unwrap();
        let content = std::fs::read_to_string(manager.file_path()).unwrap();
        assert!(content.contains("[Desktop Entry]"), "stale file must be overwritten");
        assert!(!content.contains("stale content"));
    }

    #[test]
    fn disable_removes_file() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        manager.enable().unwrap();
        assert!(manager.file_path().exists());
        manager.disable().unwrap();
        assert!(!manager.file_path().exists());
    }

    #[test]
    fn disable_twice_does_not_error() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        manager.enable().unwrap();
        manager.disable().unwrap();
        manager.disable().unwrap(); // second call should not fail
    }

    #[test]
    fn enable_then_disable_leaves_no_file() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        manager.enable().unwrap();
        manager.disable().unwrap();
        assert!(!manager.file_path().exists());
    }

    #[test]
    fn disable_when_never_enabled_ok() {
        let dir = tempdir().unwrap();
        let manager = AutostartManager::with_base_dir(dir.path().to_path_buf());
        // Never called enable() -- disable should be fine
        manager.disable().unwrap();
    }
}
