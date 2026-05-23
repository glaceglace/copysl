# Module: `config` — Configuration I/O

Crate path: `crates/config/src/lib.rs`
Handles reading and writing `~/.config/copieur/config.toml`.

---

## Tasks

### Crate Setup
- [ ] Create `crates/config/` with `Cargo.toml`
- [ ] Add `toml`, `serde`, `dirs` (for home dir resolution) dependencies
- [ ] Add `config` to workspace `Cargo.toml` members

### Path Resolution
- [ ] Implement `config_path() -> PathBuf` — returns `~/.config/copieur/config.toml` using `dirs::config_dir()`
- [ ] Ensure parent directory is created if it does not exist

### Read Path
- [ ] Implement `load() -> Result<Config>`:
  - [ ] If file does not exist, return `Config::default()` and write defaults to disk
  - [ ] If file exists, read and parse with `toml::from_str`
  - [ ] On parse error, log the error and return `Config::default()` (do not overwrite corrupt file)
- [ ] Unit test: load from a temp file with valid TOML returns correct `Config`
- [ ] Unit test: load from missing file returns defaults and creates the file
- [ ] Unit test: load from malformed TOML returns defaults without overwriting

### Write Path
- [ ] Implement `save(config: &Config) -> Result<()>`:
  - [ ] Serialize with `toml::to_string_pretty`
  - [ ] Write to `.config.toml.tmp` in same directory
  - [ ] Atomically rename `.tmp` → `config.toml`
- [ ] Unit test: save then load round-trips all fields correctly
- [ ] Unit test: partial write failure (simulate) does not corrupt existing file

### Default TOML content
- [ ] Verify that `Config::default()` serializes to the spec-defined defaults:
  - `shortcut = "Super+V"`, `max_entries = 200`, `persist_history = false`,
    `autostart = true`, `window_position = "NearCursor"`, `theme = "System"`
