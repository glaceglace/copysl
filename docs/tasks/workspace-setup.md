# Module: Workspace & Project Setup

Top-level Cargo workspace configuration and project scaffolding.

---

## Tasks

### Workspace `Cargo.toml`
- [ ] Create root `Cargo.toml` as a workspace manifest:
  ```toml
  [workspace]
  members = ["crates/common", "crates/config", "crates/daemon", "crates/ui"]
  resolver = "2"
  ```
- [ ] Define shared dependency versions in `[workspace.dependencies]` for: `serde`, `bincode`, `tokio`, `eframe`, `egui`, `arboard`, `rusqlite`, `toml`, `xcb`, `notify-rust`, `sha2`, `dirs`, `image`

### Root Binary (`src/main.rs`)
- [ ] Create `src/` directory and `src/main.rs`
- [ ] Add to root `Cargo.toml`:
  ```toml
  [[bin]]
  name = "copieur"
  path = "src/main.rs"
  ```
- [ ] Implement argument dispatch (see `ui-main.md`)

### `.gitignore`
- [ ] Add standard Rust `.gitignore`: `target/`, `Cargo.lock` (keep for binary crate)
- [ ] Keep `Cargo.lock` committed (it is a binary application)

### `README.md` (minimal)
- [ ] Document: how to build (`cargo build --release`), how to run daemon, how to open UI, external tool requirements (`xdotool` / `ydotool`)

### CI Skeleton (optional, v1)
- [ ] Add `.github/workflows/ci.yml` with `cargo check`, `cargo test`, `cargo clippy`

### First Build Verification
- [ ] Run `cargo check` with no errors
- [ ] Run `cargo test` — all unit tests pass
- [ ] Run `cargo clippy -- -D warnings` — no warnings
