use std::path::{Path, PathBuf};

use anyhow::Result;
use common::{Config, Theme, WindowPos};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// TOML-friendly intermediate representation
// ---------------------------------------------------------------------------

/// A flat, string-based struct that maps cleanly to TOML.
///
/// `WindowPos::Fixed(x, y)` is split into two optional integer fields so that
/// the generated TOML is human-readable rather than a nested table.
#[derive(Serialize, Deserialize)]
struct ConfigToml {
    max_entries: usize,
    persist_history: bool,
    autostart: bool,
    /// `"NearCursor"` or `"Fixed"`
    window_position: String,
    /// Present only when `window_position == "Fixed"`
    #[serde(skip_serializing_if = "Option::is_none")]
    window_position_x: Option<i32>,
    /// Present only when `window_position == "Fixed"`
    #[serde(skip_serializing_if = "Option::is_none")]
    window_position_y: Option<i32>,
    /// `"System"`, `"Light"`, or `"Dark"`
    theme: String,
    #[serde(default = "default_paste_delay_ms")]
    paste_delay_ms: u32,
}

fn default_paste_delay_ms() -> u32 { 150 }

impl From<&Config> for ConfigToml {
    fn from(c: &Config) -> Self {
        let (window_position, window_position_x, window_position_y) = match c.window_position {
            WindowPos::NearCursor => ("NearCursor".to_string(), None, None),
            WindowPos::Fixed(x, y) => ("Fixed".to_string(), Some(x), Some(y)),
        };

        let theme = match c.theme {
            Theme::System => "System",
            Theme::Light => "Light",
            Theme::Dark => "Dark",
        }
        .to_string();

        ConfigToml {
            max_entries: c.max_entries,
            persist_history: c.persist_history,
            autostart: c.autostart,
            window_position,
            window_position_x,
            window_position_y,
            theme,
            paste_delay_ms: c.paste_delay_ms,
        }
    }
}

impl TryFrom<ConfigToml> for Config {
    type Error = anyhow::Error;

    fn try_from(t: ConfigToml) -> Result<Self> {
        let window_position = match t.window_position.as_str() {
            "NearCursor" => WindowPos::NearCursor,
            "Fixed" => {
                let x = t
                    .window_position_x
                    .ok_or_else(|| anyhow::anyhow!("missing window_position_x for Fixed variant"))?;
                let y = t
                    .window_position_y
                    .ok_or_else(|| anyhow::anyhow!("missing window_position_y for Fixed variant"))?;
                WindowPos::Fixed(x, y)
            }
            other => {
                return Err(anyhow::anyhow!(
                    "unknown window_position value: {:?}",
                    other
                ))
            }
        };

        let theme = match t.theme.as_str() {
            "System" => Theme::System,
            "Light" => Theme::Light,
            "Dark" => Theme::Dark,
            other => return Err(anyhow::anyhow!("unknown theme value: {:?}", other)),
        };

        Ok(Config {
            max_entries: t.max_entries,
            persist_history: t.persist_history,
            autostart: t.autostart,
            window_position,
            theme,
            paste_delay_ms: t.paste_delay_ms,
        })
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns the canonical path to the user's config file: `~/.copysl/config.toml`.
pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .expect("Could not determine home directory")
        .join(".copysl")
        .join("config.toml")
}

/// Load config from the default location (`config_path()`).
///
/// If the file does not exist, defaults are written to disk and returned.
/// If the file is malformed, an error is logged and defaults are returned
/// **without** overwriting the corrupt file.
pub fn load() -> Result<Config> {
    load_from(&config_path())
}

/// Load config from an explicit `path`.  Useful for tests.
///
/// See [`load`] for the full behaviour description.
pub fn load_from(path: &Path) -> Result<Config> {
    if !path.exists() {
        let defaults = Config::default();
        save_to(path, &defaults)?;
        return Ok(defaults);
    }

    let raw = std::fs::read_to_string(path)?;
    match toml::from_str::<ConfigToml>(&raw) {
        Ok(toml_cfg) => Config::try_from(toml_cfg),
        Err(e) => {
            log::error!("Failed to parse config at {}: {e}", path.display());
            Ok(Config::default())
        }
    }
}

/// Save `config` to the default location (`config_path()`).
///
/// Uses an atomic write: serializes to a `.tmp` file then renames it.
pub fn save(config: &Config) -> Result<()> {
    save_to(&config_path(), config)
}

/// Save `config` to an explicit `path`.  Useful for tests.
///
/// See [`save`] for the full behaviour description.
pub fn save_to(path: &Path, config: &Config) -> Result<()> {
    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let toml_cfg = ConfigToml::from(config);
    let content = toml::to_string_pretty(&toml_cfg)?;

    // Write to a sibling .tmp file then atomically rename.
    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &content)?;
    std::fs::rename(&tmp_path, path)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use common::{Theme, WindowPos};
    use tempfile::tempdir;

    // -----------------------------------------------------------------------
    // 1. Load from valid TOML
    // -----------------------------------------------------------------------

    #[test]
    fn load_valid_toml_returns_correct_config() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let toml_content = r#"
max_entries = 50
persist_history = true
autostart = false
window_position = "NearCursor"
theme = "Dark"
"#;
        std::fs::write(&path, toml_content).unwrap();

        let config = load_from(&path).unwrap();

        assert_eq!(config.max_entries, 50);
        assert!(config.persist_history);
        assert!(!config.autostart);
        assert_eq!(config.window_position, WindowPos::NearCursor);
        assert_eq!(config.theme, Theme::Dark);
    }

    // -----------------------------------------------------------------------
    // 2. Load from missing file returns defaults and creates the file
    // -----------------------------------------------------------------------

    #[test]
    fn load_missing_file_returns_defaults_and_creates_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        assert!(!path.exists(), "file should not exist before load");

        let config = load_from(&path).unwrap();

        assert_eq!(config, Config::default());
        assert!(path.exists(), "file should have been created by load_from");
    }

    // -----------------------------------------------------------------------
    // 3. Load from malformed TOML returns defaults WITHOUT overwriting
    // -----------------------------------------------------------------------

    #[test]
    fn load_malformed_toml_returns_defaults_without_overwriting() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let garbage = "this is not valid toml @@@ !!!";
        std::fs::write(&path, garbage).unwrap();

        let config = load_from(&path).unwrap();

        assert_eq!(config, Config::default());

        // The corrupt file must NOT have been overwritten.
        let contents_after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(contents_after, garbage);
    }

    // -----------------------------------------------------------------------
    // 4. Save then load round-trips all fields
    // -----------------------------------------------------------------------

    #[test]
    fn save_then_load_round_trips_all_fields() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let original = Config {
            max_entries: 42,
            persist_history: true,
            autostart: false,
            window_position: WindowPos::Fixed(100, 200),
            theme: Theme::Light,
            paste_delay_ms: 200,
        };

        save_to(&path, &original).unwrap();
        let loaded = load_from(&path).unwrap();

        assert_eq!(loaded, original);
    }

    // -----------------------------------------------------------------------
    // 5. Default config serializes to the expected TOML values
    // -----------------------------------------------------------------------

    #[test]
    fn default_config_serializes_to_expected_toml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        save_to(&path, &Config::default()).unwrap();
        let content = std::fs::read_to_string(&path).unwrap();

        assert!(content.contains("max_entries = 200"));
        assert!(content.contains("persist_history = false"));
        assert!(content.contains("autostart = true"));
        assert!(content.contains("window_position = \"NearCursor\""));
        assert!(content.contains("theme = \"System\""));
    }

    // -----------------------------------------------------------------------
    // 6. NearCursor and all theme variants round-trip correctly
    // -----------------------------------------------------------------------

    #[test]
    fn near_cursor_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let cfg = Config {
            window_position: WindowPos::NearCursor,
            ..Config::default()
        };
        save_to(&path, &cfg).unwrap();
        assert_eq!(load_from(&path).unwrap(), cfg);
    }

    #[test]
    fn theme_light_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let cfg = Config {
            theme: Theme::Light,
            ..Config::default()
        };
        save_to(&path, &cfg).unwrap();
        assert_eq!(load_from(&path).unwrap(), cfg);
    }

    #[test]
    fn theme_dark_round_trips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let cfg = Config {
            theme: Theme::Dark,
            ..Config::default()
        };
        save_to(&path, &cfg).unwrap();
        assert_eq!(load_from(&path).unwrap(), cfg);
    }
}
