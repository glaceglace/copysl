# Module: `common` — Shared Types and Protocol

Crate path: `crates/common/src/lib.rs`
Compiled into both daemon and UI. No platform-specific code.

---

## Tasks

### Crate Setup
- [ ] Create `crates/common/` directory with `Cargo.toml`
- [ ] Add `bincode` and `serde` dependencies
- [ ] Add `common` to workspace `Cargo.toml` members

### `EntryId`
- [ ] Define `EntryId(pub u64)` newtype with `derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)`

### `ImageMime`
- [ ] Define `ImageMime` enum with variants `Png`, `Jpeg` and serde derives

### `ContentPayload`
- [ ] Define `ContentPayload` enum:
  - [ ] `PlainText(String)`
  - [ ] `RichText { html: String, plain_preview: String }`
  - [ ] `Image { data: Vec<u8>, mime: ImageMime }`
- [ ] Add serde derives

### `ClipboardEntry`
- [ ] Define `ClipboardEntry` struct with fields: `id`, `payload`, `captured_at` (`SystemTime`), `pinned` (`bool`)
- [ ] Add serde derives
- [ ] Implement content hash helper method (SHA-256 of raw payload bytes) using `sha2` crate

### `DaemonRequest`
- [ ] Define `DaemonRequest` enum with all variants:
  - [ ] `GetHistory { offset: usize, limit: usize }`
  - [ ] `PasteEntry { id: EntryId }`
  - [ ] `DeleteEntry { id: EntryId }`
  - [ ] `PinEntry { id: EntryId }`
  - [ ] `UnpinEntry { id: EntryId }`
  - [ ] `ClearHistory { include_pinned: bool }`
  - [ ] `GetConfig`
  - [ ] `UpdateConfig(Config)`
- [ ] Add serde derives

### `DaemonResponse`
- [ ] Define `DaemonResponse` enum with all variants:
  - [ ] `History(Vec<ClipboardEntry>)`
  - [ ] `Config(Config)`
  - [ ] `Ok`
  - [ ] `Err(String)`
  - [ ] `NewEntry(ClipboardEntry)`
- [ ] Add serde derives

### `Config` and supporting types
- [ ] Define `WindowPos` enum: `NearCursor`, `Fixed(i32, i32)`
- [ ] Define `Theme` enum: `System`, `Light`, `Dark`
- [ ] Define `KeyCombo` struct (wraps a string like `"Super+V"`) with parse/display
- [ ] Define `Config` struct with all fields and defaults
- [ ] Implement `Default` for `Config` matching spec defaults

### IPC framing helpers
- [ ] Implement `write_frame(stream, msg)` — serialize with bincode, prefix 4-byte LE length, write to stream
- [ ] Implement `read_frame(stream) -> Result<T>` — read 4-byte length, read that many bytes, deserialize with bincode
- [ ] Unit test: round-trip encode/decode for each message variant
