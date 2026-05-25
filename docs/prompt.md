# Copieur — Vibe Coding Start Prompt

> This document is the authoritative start prompt for an autonomous multi-agent implementation session.
> The agent reading this is the **main (orchestrating) agent**. It tracks global progress and spawns
> one child agent per module. No human interaction is required during implementation.

---

## Project Context

**What**: Copieur is a Linux clipboard history manager written in pure Rust. A long-running daemon
monitors the clipboard, stores history in memory, and exposes a card-based UI (egui/eframe) triggered
by a global keyboard shortcut (Super+V). Inspired by Windows clipboard history (Win+V).

**Key source documents** (read these before doing anything):
- `docs/demands.md` — user-facing requirements
- `docs/detailed-design.md` — architecture, data structures, module breakdown, data flows
- `docs/tasks/progress.md` — global progress checklist (update this as modules complete)
- `docs/tasks/<module>.md` — per-module task checklist (one file per module)

**Current repo state**: The root `Cargo.toml` is a bare single-package manifest and `src/main.rs`
contains a stub. Both will be completely replaced as part of the workspace-setup module.

---

## Orchestration Model

You are the **main agent**. Your responsibilities:

1. Read `docs/tasks/progress.md` at the start to determine which modules are already done.
2. For each incomplete module (in the order listed in §Implementation Order below), spawn a
   **child agent** and give it the module brief from §Module Briefs.
3. After each child agent returns, verify its quality gate passed (see §Quality Gates).
4. Mark the module complete in `docs/tasks/progress.md`.
5. Proceed to the next module. Do not spawn the next child until the current one passes its gate.
6. After all 20 modules are complete, run the final integration check (§Final Integration).

**Resumability**: If this session is interrupted, re-run this prompt. The main agent reads
`docs/tasks/progress.md` and skips already-completed modules.

**Child agent isolation**: Each child agent receives only its module brief (from §Module Briefs).
The child agent must also read the relevant `docs/tasks/<module>.md` file for full task details.
Child agents must not communicate with each other; all coordination flows through you (the main agent).

---

## Implementation Order

Modules must be implemented in this exact order (earlier modules are dependencies of later ones):

| # | Module | Task file |
|---|--------|-----------|
| 1 | workspace-setup | `docs/tasks/workspace-setup.md` |
| 2 | common | `docs/tasks/common.md` |
| 3 | config | `docs/tasks/config.md` |
| 4 | daemon-clipboard-monitor | `docs/tasks/daemon-clipboard-monitor.md` |
| 5 | daemon-persistence | `docs/tasks/daemon-persistence.md` |
| 6 | daemon-history-store | `docs/tasks/daemon-history-store.md` |
| 7 | daemon-focus-tracker | `docs/tasks/daemon-focus-tracker.md` |
| 8 | daemon-paste-executor | `docs/tasks/daemon-paste-executor.md` |
| 9 | daemon-shortcut-listener | `docs/tasks/daemon-shortcut-listener.md` |
| 10 | daemon-autostart-manager | `docs/tasks/daemon-autostart-manager.md` |
| 11 | daemon-ipc-server | `docs/tasks/daemon-ipc-server.md` |
| 12 | daemon-main | `docs/tasks/daemon-main.md` |
| 13 | ui-ipc-client | `docs/tasks/ui-ipc-client.md` |
| 14 | ui-window | `docs/tasks/ui-window.md` |
| 15 | ui-components-search-bar | `docs/tasks/ui-components-search-bar.md` |
| 16 | ui-components-card | `docs/tasks/ui-components-card.md` |
| 17 | ui-components-card-list | `docs/tasks/ui-components-card-list.md` |
| 18 | ui-components-settings-panel | `docs/tasks/ui-components-settings-panel.md` |
| 19 | ui-app | `docs/tasks/ui-app.md` |
| 20 | ui-main | `docs/tasks/ui-main.md` |

---

## Quality Gates

Every child agent must pass all of the following before it is considered complete.
Run these commands from the workspace root after the agent finishes its code changes:

```bash
# 1. Type-check (no errors)
cargo check --workspace

# 2. Tests (all pass)
cargo test --workspace

# 3. Lint (zero warnings treated as errors)
cargo clippy --workspace -- -D warnings

# 4. Coverage (100% line coverage required)
cargo llvm-cov --workspace --lcov --output-path target/llvm-cov/lcov.info
cargo llvm-cov report --workspace --summary-only
```

If any gate fails, send the error output back to the same child agent with instructions to fix it.
Repeat until all gates pass. Only then mark the module complete in `docs/tasks/progress.md`.

**Installing cargo-llvm-cov** (run once if not already installed):
```bash
cargo install cargo-llvm-cov
rustup component add llvm-tools-preview
```

---

## Coverage Strategy for Platform-Specific Code

100% line coverage applies to **all Rust source files in the workspace**, including platform-specific
modules. To achieve this without a real X11/Wayland display server, every platform-specific module
must use the following pattern:

1. **Define a trait** for the platform operation (e.g. `FocusTracker`, `ShortcutListener`,
   `PasteBackend`). The trait is already specified in `docs/detailed-design.md §8`.
2. **Provide a `MockXxx` implementation** in a `#[cfg(test)]` block (or in a `tests/` submodule)
   that covers every branch of the trait.
3. **Gate real platform I/O** behind `#[cfg(not(test))]` at the struct/impl level so the test build
   never tries to open an X11 connection or `/dev/input` device.
4. **Unit-test the mock** so that every line in the mock implementation is exercised.

The X11 and Wayland concrete structs (e.g. `X11FocusTracker`, `WaylandShortcutListener`) live behind
`#[cfg(not(test))]`. Their logic is covered by a thin integration-test layer gated with:
```rust
#[cfg(all(test, feature = "integration"))]
```
These integration tests are NOT required to pass in CI (they require a display server). The `100%`
coverage target excludes lines gated behind `#[cfg(all(test, feature = "integration"))]`.

Use `#[cfg_attr(test, allow(dead_code))]` sparingly to silence unavoidable dead-code warnings on
platform stubs in test builds.

---

## Workspace & Crate Layout

After module 1 (workspace-setup) completes, the repo must match this layout exactly:

```
copieur/                          ← workspace root
├── Cargo.toml                    ← workspace manifest (replaces bare package)
├── Cargo.lock                    ← committed (binary application)
├── src/
│   └── main.rs                   ← binary entry: dispatches --daemon vs UI
├── crates/
│   ├── common/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── config/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   ├── daemon/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── clipboard_monitor.rs
│   │       ├── history_store.rs
│   │       ├── focus_tracker/
│   │       │   ├── mod.rs
│   │       │   ├── x11.rs
│   │       │   └── wayland.rs
│   │       ├── shortcut_listener/
│   │       │   ├── mod.rs
│   │       │   ├── x11.rs
│   │       │   └── wayland.rs
│   │       ├── ipc_server.rs
│   │       ├── paste_executor/
│   │       │   ├── mod.rs
│   │       │   ├── xdotool.rs
│   │       │   ├── xsendevent.rs
│   │       │   ├── ydotool.rs
│   │       │   └── notification_fallback.rs
│   │       ├── persistence.rs
│   │       └── autostart_manager.rs
│   └── ui/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── app.rs
│           ├── window.rs
│           ├── ipc_client.rs
│           └── components/
│               ├── search_bar.rs
│               ├── card_list.rs
│               ├── card.rs
│               └── settings_panel.rs
└── doc/
    └── prompt.md                 ← this file
```

The root `Cargo.toml` workspace manifest uses `resolver = "2"` and declares shared dependency
versions in `[workspace.dependencies]`. All crate `Cargo.toml` files inherit versions from
workspace dependencies using `{ workspace = true }`.

---

## Module Briefs

Each section below is the brief to hand to the corresponding child agent. The child agent must
also read the linked `docs/tasks/<module>.md` file for the full task checklist.

---

### Module 1 — workspace-setup

**Task file**: `docs/tasks/workspace-setup.md`

You are implementing the **workspace-setup** module for the Copieur project.

Copieur is a Rust clipboard history manager. The root `Cargo.toml` currently contains a bare
single-package manifest. Your job is to completely replace it with a Cargo workspace manifest and
scaffold all four crates (`common`, `config`, `daemon`, `ui`).

Steps:
1. Read `docs/demands.md`, `docs/detailed-design.md`, and `docs/tasks/workspace-setup.md` in full.
2. Replace the root `Cargo.toml` with a workspace manifest containing:
   - `members = ["crates/common", "crates/config", "crates/daemon", "crates/ui"]`
   - `resolver = "2"`
   - `[workspace.dependencies]` with pinned versions for: `serde`, `bincode`, `tokio`, `eframe`,
     `egui`, `arboard`, `rusqlite`, `toml`, `xcb`, `notify-rust`, `sha2`, `dirs`, `image`,
     `anyhow`, `thiserror`, `log`, `env_logger`
3. Create stub `Cargo.toml` and `src/lib.rs` (or `src/main.rs`) for each crate.
   - `common` and `config` are library crates.
   - `daemon` and `ui` are library crates (with a `lib.rs` exporting a public entry function).
   - The top-level binary is `src/main.rs` (already exists, update it to the dispatch stub).
4. Replace `src/main.rs` with the argument-dispatch stub:
   ```rust
   fn main() {
       let args: Vec<String> = std::env::args().collect();
       if args.iter().any(|a| a == "--daemon") {
           daemon::daemon_main();
       } else {
           ui::ui_main();
       }
   }
   ```
5. Add a `.gitignore` with `target/` (keep `Cargo.lock`).
6. Verify `cargo check --workspace` passes with zero errors.
7. All quality gates must pass before you report completion.

---

### Module 2 — common

**Task file**: `docs/tasks/common.md`

You are implementing the **common** crate for the Copieur project.

Copieur is a Rust clipboard history manager. The `common` crate is a pure library crate (no
platform-specific code) that defines all shared types and the IPC wire protocol used by both the
daemon and the UI.

Steps:
1. Read `docs/demands.md`, `docs/detailed-design.md §3.1`, and `docs/tasks/common.md` in full.
2. Implement in `crates/common/src/lib.rs`:
   - `EntryId(pub u64)` newtype
   - `ImageMime` enum (`Png`, `Jpeg`)
   - `ContentPayload` enum (`PlainText`, `RichText`, `Image`)
   - `ClipboardEntry` struct with `content_hash()` method using `sha2`
   - `DaemonRequest` enum (all 8 variants)
   - `DaemonResponse` enum (all 5 variants)
   - `WindowPos`, `Theme`, `KeyCombo`, `Config` with `Default`
   - `write_frame` and `read_frame` framing helpers (4-byte LE length prefix + bincode)
3. All types must derive `serde::{Serialize, Deserialize}` and appropriate std traits.
4. Write unit tests that round-trip every message variant through `write_frame`/`read_frame`.
5. All quality gates must pass before you report completion.

---

### Module 3 — config

**Task file**: `docs/tasks/config.md`

You are implementing the **config** crate for the Copieur project.

Copieur is a Rust clipboard history manager. The `config` crate handles reading and writing
`~/.config/copieur/config.toml`. It is used by both the daemon (on startup and on `UpdateConfig`)
and exposed to the UI via IPC.

Steps:
1. Read `docs/detailed-design.md §3.4` and `docs/tasks/config.md` in full.
2. Implement in `crates/config/src/lib.rs`:
   - `config_path() -> PathBuf` using `dirs::config_dir()`
   - `load() -> anyhow::Result<Config>`: read TOML or write defaults; on parse error log + return
     defaults without overwriting
   - `save(config: &Config) -> anyhow::Result<()>`: atomic rename via `.tmp` file
3. Write unit tests using `tempfile` (add as dev-dependency):
   - load from valid TOML returns correct Config
   - load from missing file returns defaults and creates file
   - load from malformed TOML returns defaults without overwriting
   - save then load round-trips all fields
4. All quality gates must pass before you report completion.

---

### Module 4 — daemon-clipboard-monitor

**Task file**: `docs/tasks/daemon-clipboard-monitor.md`

You are implementing the **clipboard_monitor** module inside the `daemon` crate for Copieur.

Copieur is a Rust clipboard history manager. The clipboard monitor runs on a dedicated OS thread,
polls the clipboard every 200 ms via `arboard`, detects changes via SHA-256 hashing, and sends new
`ClipboardEntry` values to the history store via an `mpsc` channel.

Steps:
1. Read `docs/detailed-design.md §3.2.1` and `docs/tasks/daemon-clipboard-monitor.md` in full.
2. Create `crates/daemon/src/clipboard_monitor.rs`.
3. Implement `ClipboardMonitor::spawn(tx: mpsc::Sender<ClipboardEntry>) -> std::thread::JoinHandle<()>`.
4. Poll order: Image → RichText/HTML → PlainText. Skip if clipboard is empty.
5. Assign `EntryId(0)` (HistoryStore reassigns the real ID).
6. Change detection via SHA-256 of raw payload bytes. Skip duplicates.
7. For testing without a real clipboard: accept an injectable `ClipboardReader` trait (or a
   constructor parameter) that can be replaced by a mock in tests. Gate the real `arboard`
   implementation behind `#[cfg(not(test))]`.
8. Write unit tests:
   - Two identical reads do not produce two sends.
   - Changing content between reads produces a send.
9. All quality gates must pass before you report completion.

---

### Module 5 — daemon-persistence

**Task file**: `docs/tasks/daemon-persistence.md`

You are implementing the **persistence** module inside the `daemon` crate for Copieur.

Copieur is a Rust clipboard history manager. The persistence module provides optional SQLite-backed
history storage. It runs on a dedicated thread (because `rusqlite::Connection` is not `Send`) and
receives commands via an `mpsc` channel.

Steps:
1. Read `docs/detailed-design.md §3.2.7` and `docs/tasks/daemon-persistence.md` in full.
2. Create `crates/daemon/src/persistence.rs`.
3. Implement:
   - `db_path() -> PathBuf` (`~/.local/share/copieur/history.db`)
   - Schema: CREATE TABLE IF NOT EXISTS with WAL mode
   - `PersistenceCommand` enum and `PersistenceHandle` (mpsc sender)
   - Dedicated thread processing: Upsert, Delete, UpdatePin, Clear, LoadAll
4. Use `rusqlite` with feature `"bundled"`.
5. For testing: use an in-memory SQLite database (`Connection::open_in_memory()`). Do NOT use real
   file paths in tests. All unit tests must use in-memory DBs.
6. Write unit tests:
   - Upsert then LoadAll returns same entry
   - Delete removes entry
   - Clear(false) retains pinned entries
   - Clear(true) removes all
   - UpdatePin changes flag
7. All quality gates must pass before you report completion.

---

### Module 6 — daemon-history-store

**Task file**: `docs/tasks/daemon-history-store.md`

You are implementing the **history_store** module inside the `daemon` crate for Copieur.

Copieur is a Rust clipboard history manager. The history store is the authoritative in-memory store
of clipboard entries. It owns a `VecDeque<ClipboardEntry>`, handles deduplication, eviction,
pinning, and paging. It is owned by a single async task and communicated with via message passing.

Steps:
1. Read `docs/detailed-design.md §3.2.2` and `docs/tasks/daemon-history-store.md` in full.
2. Create `crates/daemon/src/history_store.rs`.
3. Implement `HistoryStore` with operations: `push`, `delete`, `pin`, `unpin`, `clear`,
   `get_page`. The `PersistenceHandle` parameter is `Option<PersistenceHandle>`.
4. `push` must: deduplicate by content hash (move to front if match), prepend new entry, evict
   oldest unpinned if over capacity.
5. `get_page` must: sort pinned first (insertion order), then unpinned (newest first). Apply
   offset + limit. Never mutate the VecDeque order.
6. Write unit tests (pure, no I/O — pass `persistence: None`):
   - push deduplicates (same content → moves to front, count unchanged)
   - push evicts oldest unpinned at capacity; pinned never evicted
   - get_page ordering (pinned before unpinned)
   - clear(false) retains pinned
   - clear(true) removes all
   - delete removes correct entry by id
7. All quality gates must pass before you report completion.

---

### Module 7 — daemon-focus-tracker

**Task file**: `docs/tasks/daemon-focus-tracker.md`

You are implementing the **focus_tracker** module inside the `daemon` crate for Copieur.

Copieur is a Rust clipboard history manager. The focus tracker monitors which window has focus so
the paste executor knows where to inject Ctrl+V. It must support X11 and Wayland via separate
implementations behind a trait.

Steps:
1. Read `docs/detailed-design.md §3.2.3` and `docs/tasks/daemon-focus-tracker.md` in full.
2. Create `crates/daemon/src/focus_tracker/mod.rs`, `x11.rs`, `wayland.rs`.
3. Define `FocusHandle`, `PlatformFocusHandle` (X11(u32), Wayland variant, Unknown).
4. Define `FocusTracker` trait: `start()`, `current_focus() -> Option<FocusHandle>`.
5. Implement `NoopFocusTracker` (always returns None) — used in tests.
6. X11 and Wayland concrete structs are gated behind `#[cfg(not(test))]`.
7. Platform selection function: checks `WAYLAND_DISPLAY` / `DISPLAY` env vars and returns
   `Box<dyn FocusTracker>`. In test builds, always returns `NoopFocusTracker`.
8. Write unit tests:
   - `NoopFocusTracker::current_focus()` returns None
   - Platform selection logic (by injecting env vars in tests using `std::env::set_var` in a
     controlled scope or by passing display-server discriminant directly)
9. All quality gates must pass before you report completion.

---

### Module 8 — daemon-paste-executor

**Task file**: `docs/tasks/daemon-paste-executor.md`

You are implementing the **paste_executor** module inside the `daemon` crate for Copieur.

Copieur is a Rust clipboard history manager. The paste executor writes a clipboard entry back to
the system clipboard via `arboard` and injects a synthetic Ctrl+V into the previously focused
window. It tries backends in priority order: xdotool (X11), XSendEvent (X11 fallback), ydotool
(Wayland), notification fallback.

Steps:
1. Read `docs/detailed-design.md §3.2.6` and `docs/tasks/daemon-paste-executor.md` in full.
2. Create `crates/daemon/src/paste_executor/mod.rs`, `xdotool.rs`, `xsendevent.rs`,
   `ydotool.rs`, `notification_fallback.rs`.
3. Define `PasteError` and `PasteBackend` trait.
4. Implement each backend. Gate subprocess calls and xcb calls behind `#[cfg(not(test))]`.
   In test builds, inject the tool availability via a constructor parameter or strategy trait.
5. `select_backend`: X11 → try xdotool → XSendEvent; Wayland → try ydotool → notification.
6. `PasteExecutor::paste(id)`: look up entry, write to arboard, sleep 50ms, inject paste.
   For tests: inject a mock `arboard` writer and a mock `PasteBackend`.
7. Write unit tests:
   - select_backend returns correct type based on injected display server + tool availability
   - fallback chain triggers notification when primary + secondary backends fail
8. All quality gates must pass before you report completion.

---

### Module 9 — daemon-shortcut-listener

**Task file**: `docs/tasks/daemon-shortcut-listener.md`

You are implementing the **shortcut_listener** module inside the `daemon` crate for Copieur.

Copieur is a Rust clipboard history manager. The shortcut listener captures the global keyboard
shortcut (default Super+V) and spawns or closes the UI process (toggle behavior). It supports X11
via XGrabKey and Wayland via ext-global-shortcuts / evdev / notification fallback.

Steps:
1. Read `docs/detailed-design.md §3.2.4` and `docs/tasks/daemon-shortcut-listener.md` in full.
2. Create `crates/daemon/src/shortcut_listener/mod.rs`, `x11.rs`, `wayland.rs`.
3. Define `ShortcutListener` trait: `start(key, on_trigger)`, `stop()`.
4. Implement UI spawn/toggle logic in `mod.rs`:
   - Track `Option<std::process::Child>` for the running UI process.
   - On trigger: if no UI → spawn `copieur` with `COPIEUR_SOCKET` env var; if UI running → SIGTERM.
   - Reap zombie child with `Child::try_wait`.
5. Gate X11 / Wayland / evdev concrete impls behind `#[cfg(not(test))]`.
6. Provide a `MockShortcutListener` that accepts a callback and can be triggered from tests.
7. Write unit tests:
   - Toggle logic: first trigger spawns child (use `MockProcess`/callback capture), second trigger
     kills it. Mock the process spawn to avoid real execution.
   - `KeyCombo` parse produces correct modifier/keycode values.
8. All quality gates must pass before you report completion.

---

### Module 10 — daemon-autostart-manager

**Task file**: `docs/tasks/daemon-autostart-manager.md`

You are implementing the **autostart_manager** module inside the `daemon` crate for Copieur.

Copieur is a Rust clipboard history manager. The autostart manager writes or deletes
`~/.config/autostart/copieur.desktop` to register the daemon as an XDG autostart application.

Steps:
1. Read `docs/detailed-design.md §3.2.8` and `docs/tasks/daemon-autostart-manager.md` in full.
2. Create `crates/daemon/src/autostart_manager.rs`.
3. Implement:
   - `desktop_file_path() -> PathBuf` using `dirs::config_dir()`
   - `AutostartManager::enable(&self) -> anyhow::Result<()>` — writes `.desktop` file
   - `AutostartManager::disable(&self) -> anyhow::Result<()>` — removes `.desktop` file, ignores
     NotFound
4. For tests: inject a `base_dir: PathBuf` parameter so tests use `tempfile::tempdir()` instead
   of the real `~/.config/autostart/`.
5. Write unit tests:
   - enable() writes file with correct Exec path
   - disable() removes file; calling disable() twice does not error
   - enable() then disable() leaves no file
6. All quality gates must pass before you report completion.

---

### Module 11 — daemon-ipc-server

**Task file**: `docs/tasks/daemon-ipc-server.md`

You are implementing the **ipc_server** module inside the `daemon` crate for Copieur.

Copieur is a Rust clipboard history manager. The IPC server accepts Unix socket connections from
UI processes, routes `DaemonRequest` messages to the appropriate subsystems, and pushes `NewEntry`
notifications to all open connections.

Steps:
1. Read `docs/detailed-design.md §3.2.5` and `docs/tasks/daemon-ipc-server.md` in full.
2. Create `crates/daemon/src/ipc_server.rs`.
3. Use `tokio::net::UnixListener`. Each connection is handled in a `tokio::spawn` task.
4. Implement socket path resolution (`XDG_RUNTIME_DIR/copieur.sock` or `/tmp/copieur-{uid}.sock`).
   Remove stale socket on startup.
5. Route all 8 `DaemonRequest` variants to their handlers (HistoryStore, PasteExecutor, Config,
   AutostartManager). Use `tokio::sync::mpsc` channels for cross-task communication.
6. Push notifications: after responding to `GetHistory`, subscribe to a `tokio::sync::broadcast`
   channel on HistoryStore. On new entry, write `NewEntry` to all open connections.
7. For testing: bind to a temporary socket path (e.g. `tempfile::NamedTempFile` socket). Inject
   mock handles for HistoryStore and PasteExecutor.
8. Write integration tests (using `tokio::test`):
   - Connect mock client, send GetHistory, receive History response.
   - UpdateConfig round-trip updates in-memory config.
   - Push notification received after new entry pushed to store.
9. All quality gates must pass before you report completion.

---

### Module 12 — daemon-main

**Task file**: `docs/tasks/daemon-main.md`

You are implementing the **daemon entry point** (`crates/daemon/src/main.rs`) for Copieur.

Copieur is a Rust clipboard history manager. The daemon entry point wires all subsystems together:
config loading, history store, clipboard monitor, focus tracker, shortcut listener, IPC server,
paste executor, persistence, and autostart manager. It blocks on the tokio runtime until SIGTERM/SIGINT.

Steps:
1. Read `docs/detailed-design.md §3.2` and `docs/tasks/daemon-main.md` in full.
2. Implement `pub fn daemon_main()` in `crates/daemon/src/main.rs` (it is called from the root
   `src/main.rs` dispatch).
3. Wire subsystems in this order:
   a. Load `Config` via `config::load()`
   b. Optionally open `PersistenceHandle` and load history into `HistoryStore`
   c. Start `HistoryStore` task
   d. Detect display server, start `FocusTracker`
   e. Bind `IpcServer`, store socket path
   f. Spawn `ClipboardMonitor` thread
   g. Construct `PasteExecutor`
   h. Start `ShortcutListener` (passes socket path for UI spawn)
   i. Conditionally call `AutostartManager::enable()` on first run
   j. Log "Copieur daemon started at <socket_path>"
   k. Await `tokio::signal::ctrl_c()` or SIGTERM
   l. Graceful shutdown: close socket, join threads
4. Write integration smoke test: daemon starts, socket is reachable, `GetHistory` returns empty list.
   Gate behind `#[cfg(test)]` and use a dedicated test socket path to avoid conflicts.
5. All quality gates must pass before you report completion.

---

### Module 13 — ui-ipc-client

**Task file**: `docs/tasks/ui-ipc-client.md`

You are implementing the **ipc_client** module inside the `ui` crate for Copieur.

Copieur is a Rust clipboard history manager. The IPC client connects to the daemon's Unix socket,
sends `DaemonRequest` frames (blocking), and reads pushed `DaemonResponse::NewEntry` frames
(non-blocking, called each UI frame).

Steps:
1. Read `docs/detailed-design.md §3.3.7` and `docs/tasks/ui-ipc-client.md` in full.
2. Create `crates/ui/src/ipc_client.rs`.
3. Implement `IpcClient { stream: std::os::unix::net::UnixStream }`:
   - `connect() -> anyhow::Result<IpcClient>`: reads `COPIEUR_SOCKET` env var, connects, keeps
     stream in a mode where blocking can be toggled per operation.
   - `send(&mut self, req: DaemonRequest) -> anyhow::Result<DaemonResponse>`: blocking write+read.
   - `try_recv_push(&mut self) -> Option<DaemonResponse>`: non-blocking read, returns None on
     WouldBlock. Buffers partial reads across calls.
4. For testing: use a `std::os::unix::net::UnixListener` in a tempdir to create a mock server.
5. Write unit tests:
   - connect fails with clear error when COPIEUR_SOCKET points to nonexistent socket
   - send and try_recv_push correctly frame and decode messages against a mock server
6. All quality gates must pass before you report completion.

---

### Module 14 — ui-window

**Task file**: `docs/tasks/ui-window.md`

You are implementing the **window** module inside the `ui` crate for Copieur.

Copieur is a Rust clipboard history manager. The window module builds the `eframe::NativeOptions`
for the borderless, always-on-top, near-cursor clipboard history window.

Steps:
1. Read `docs/detailed-design.md §3.3.2` and `docs/tasks/ui-window.md` in full.
2. Create `crates/ui/src/window.rs`.
3. Implement:
   - `build_native_options(cursor_pos: Option<(i32, i32)>, config: &Config) -> eframe::NativeOptions`
     with `decorated: false`, `resizable: false`, `always_on_top: true`, `skip_taskbar: true`,
     `initial_window_size: Some(Vec2::new(380.0, 520.0))`
   - `compute_window_pos(cursor_pos, config) -> Option<egui::Pos2>`: NearCursor clamps to screen;
     Fixed returns exact coords; None → eframe centers
   - `get_cursor_pos() -> Option<(i32, i32)>`: X11 via xcb pointer query (gated behind
     `#[cfg(not(test))]`); always returns None in Wayland/test builds
4. Write unit tests:
   - compute_window_pos with NearCursor near screen edge clamps position so window stays on screen
   - compute_window_pos with Fixed returns exact coordinates
5. All quality gates must pass before you report completion.

---

### Module 15 — ui-components-search-bar

**Task file**: `docs/tasks/ui-components-search-bar.md`

You are implementing the **search_bar** component inside the `ui` crate for Copieur.

Copieur is a Rust clipboard history manager. The search bar is rendered at the top of every frame,
auto-focused on window open, and filters the card list in real time via case-insensitive substring
matching.

Steps:
1. Read `docs/detailed-design.md §3.3.3` and `docs/tasks/ui-components-search-bar.md` in full.
2. Create `crates/ui/src/components/search_bar.rs`.
3. Implement `SearchBar { query: String }`:
   - `new()`, `clear()`, `set_query(&mut self, s: &str)`, `request_focus(&mut self)`
   - `show(&mut self, ui: &mut egui::Ui, focus_request: bool) -> bool` (returns true if changed)
   - Handle Ctrl+F to request focus
4. Implement `fn filter(query: &str, entries: &[ClipboardEntry]) -> Vec<usize>`:
   - Empty query → all indices
   - Non-empty → case-insensitive substring match on PlainText and RichText plain_preview
   - Image entries excluded from results when query is non-empty
5. Write unit tests (pure logic, no egui rendering):
   - filter empty query returns all indices
   - filter matches case-insensitively
   - filter excludes image entries when query is non-empty
   - filter no matches returns empty vec
6. All quality gates must pass before you report completion.

---

### Module 16 — ui-components-card

**Task file**: `docs/tasks/ui-components-card.md`

You are implementing the **card** component inside the `ui` crate for Copieur.

Copieur is a Rust clipboard history manager. Each card renders one `ClipboardEntry` as an
`egui::Frame` with content preview, relative timestamp, pin icon, delete button, and right-click
context menu.

Steps:
1. Read `docs/detailed-design.md §3.3.5` and `docs/tasks/ui-components-card.md` in full.
2. Create `crates/ui/src/components/card.rs`.
3. Define `CardAction` enum: `Paste`, `Delete`, `Pin`, `Unpin`, `Copy`.
4. Implement `fn show_card(ui: &mut egui::Ui, entry: &ClipboardEntry, selected: bool) -> Option<CardAction>`.
5. Text card: first ~2 lines truncated with `…`, hover tooltip shows full text.
6. RichText card: plain_preview + small grey "HTML" badge.
7. Image card: 80px-tall thumbnail (use `image` crate for decoding); cache `egui::TextureHandle`
   by EntryId. Tooltip shows 400px-wide preview.
8. Timestamp formatting:
   - < 60s → "just now"; < 60min → "X min ago"; < 24h → "X h ago"; else → "yesterday" or date
   - Absolute datetime on hover tooltip
9. Delete button visible only on hover; right-click menu with Pin/Unpin/Delete/Copy.
10. Extract timestamp formatting into a pure function `format_relative_time(age: Duration) -> &str`
    testable without egui.
11. Write unit tests:
    - relative timestamp for each time range
12. All quality gates must pass before you report completion.

---

### Module 17 — ui-components-card-list

**Task file**: `docs/tasks/ui-components-card-list.md`

You are implementing the **card_list** component inside the `ui` crate for Copieur.

Copieur is a Rust clipboard history manager. The card list renders a scrollable list of clipboard
entry cards, handles keyboard navigation (↑↓ Enter Delete Escape), and auto-scrolls the highlighted
card into view.

Steps:
1. Read `docs/detailed-design.md §3.3.4` and `docs/tasks/ui-components-card-list.md` in full.
2. Create `crates/ui/src/components/card_list.rs`.
3. Define `CardListAction` enum: `Paste(EntryId)`, `Delete(EntryId)`, `Pin(EntryId)`,
   `Unpin(EntryId)`, `Copy(EntryId)`, `Close`.
4. Implement `CardList { selected_idx: Option<usize> }`:
   - `new()`
   - `show(&mut self, ui, entries, filtered) -> Option<CardListAction>`
5. Display order at render time: pinned entries first, then unpinned newest-first (from `filtered`).
6. Keyboard: ↑ (decrement, clamp 0), ↓ (increment, clamp len-1), Enter (Paste), Delete (Delete),
   Escape (Close).
7. When `filtered` changes: reset selected_idx to Some(0) if non-empty, else None.
8. Auto-scroll: after keyboard navigation, scroll selected card into view.
9. Extract ordering logic into a pure function `compute_display_order(entries, filtered) -> Vec<usize>`
    for testing without egui.
10. Write unit tests (pure logic):
    - ↓ increments selection, clamps at last entry
    - ↑ decrements selection, clamps at first
    - display order puts pinned before unpinned
11. All quality gates must pass before you report completion.

---

### Module 18 — ui-components-settings-panel

**Task file**: `docs/tasks/ui-components-settings-panel.md`

You are implementing the **settings_panel** component inside the `ui` crate for Copieur.

Copieur is a Rust clipboard history manager. The settings panel is an overlay rendered over the
card list (not a separate window). It exposes all config fields. Enabling disk persistence requires
the user to confirm a privacy warning before the config is saved.

Steps:
1. Read `docs/detailed-design.md §3.3.6` and `docs/tasks/ui-components-settings-panel.md` in full.
2. Create `crates/ui/src/components/settings_panel.rs`.
3. Define `SettingsAction` enum: `SaveConfig(Config)`, `Close`.
4. Implement `SettingsPanel { draft: Config, show_persist_warning: bool, rebinding: bool }`:
   - `new(config: Config)`
   - `show(&mut self, ui: &mut egui::Ui) -> Option<SettingsAction>`
5. Privacy warning flow: toggle OFF→ON shows inline warning; user must click "I understand, enable"
   before `draft.persist_history` is set to true. Cancel reverts the toggle.
6. Shortcut rebinding: while `rebinding = true`, capture next key press from `ui.input().events`.
7. Max entries: clamp to ≥ 10 via DragValue or TextEdit.
8. Window position dropdown: show x/y inputs when Fixed is selected.
9. Extract persistence-warning state machine into a pure testable type `PersistenceToggle`.
10. Write unit tests (pure logic on `PersistenceToggle`):
    - Toggling ON shows warning, not saving until confirmed
    - Cancelling warning reverts toggle to OFF
    - max entries clamps to minimum 10
11. All quality gates must pass before you report completion.

---

### Module 19 — ui-app

**Task file**: `docs/tasks/ui-app.md`

You are implementing the **app** module inside the `ui` crate for Copieur.

Copieur is a Rust clipboard history manager. `CopieurApp` is the top-level `eframe::App` struct.
It holds all UI state, orchestrates components, processes `CardListAction` and `SettingsAction`,
and handles all close triggers.

Steps:
1. Read `docs/detailed-design.md §3.3.1` and `docs/tasks/ui-app.md` in full.
2. Create `crates/ui/src/app.rs`.
3. Implement `CopieurApp`:
   - `new(cc)`: connect IpcClient; on failure, exit with error message; fetch history + config.
   - `update(ctx, frame)`: check pushed NewEntry each frame; render gear icon; show settings
     or (search bar + card list); handle all actions; handle close triggers.
4. Action handlers: Paste → send IPC + close; Delete → send IPC + update local list; Pin/Unpin
   → send IPC + update local + re-sort; Copy → write to arboard locally; SaveConfig → send IPC.
5. Close triggers: Escape key, click outside window rect, action Paste.
6. On close: clear search_bar.query.
7. Error banner: set on IPC error, auto-dismiss after 3s or on next successful IPC call.
8. Daemon-not-running startup: show modal "Copieur daemon is not running. Start with: `copieur --daemon`".
9. For testing: inject a `MockIpcClient` that implements the same interface as `IpcClient`.
    Write integration tests:
   - new() with mock daemon socket populates entries correctly
   - NewEntry push prepends to entries list
10. All quality gates must pass before you report completion.

---

### Module 20 — ui-main

**Task file**: `docs/tasks/ui-main.md`

You are implementing the **UI entry point** (`crates/ui/src/main.rs`) for Copieur.

Copieur is a Rust clipboard history manager. The UI entry point calls `window::get_cursor_pos()`,
builds `NativeOptions`, and starts the eframe event loop with `CopieurApp`.

Steps:
1. Read `docs/detailed-design.md §3.3` and `docs/tasks/ui-main.md` in full.
2. Implement `pub fn ui_main()` in `crates/ui/src/main.rs` (called from root `src/main.rs`).
3. Steps inside `ui_main()`:
   a. Attempt `IpcClient::connect()` to verify daemon is running (exit with message if not).
   b. Call `window::get_cursor_pos()`.
   c. Call `ipc.send(GetConfig)` or fall back to `config::load()`.
   d. Call `window::build_native_options(cursor_pos, &config)`.
   e. Call `eframe::run_native("Copieur", options, Box::new(|cc| Box::new(CopieurApp::new(cc))))`.
   f. Handle `run_native` errors.
4. The root `src/main.rs` dispatches to `daemon::daemon_main()` or `ui::ui_main()` based on
   `--daemon` flag. Ensure this dispatch works correctly.
5. Write unit test:
   - Argument dispatch selects daemon vs UI path correctly (mock both functions).
6. All quality gates must pass before you report completion.

---

## Final Integration Check

After all 20 modules are marked complete in `docs/tasks/progress.md`, run:

```bash
# Full workspace build
cargo build --workspace --release

# Full test suite
cargo test --workspace

# Full lint
cargo clippy --workspace -- -D warnings

# Full coverage report
cargo llvm-cov --workspace --summary-only

# Verify binary dispatches correctly
./target/release/copieur --help 2>&1 || true
```

If all pass, the implementation is complete. Report a summary of what was built.

---

## Important Constraints

1. **No Electron / WebView** — all UI is native egui/eframe.
2. **No mandatory GPU** — eframe must work with the `softbuffer` software-rasterization backend.
3. **X11 + Wayland** — both display protocols must be supported.
4. **Platform abstractions** — X11 and Wayland code always lives behind traits so future macOS
   support can be added by implementing new trait impls.
5. **Single binary** — the root `src/main.rs` dispatches to daemon or UI. There is no separate
   daemon binary.
6. **Atomic config writes** — `config::save()` must write to `.tmp` then rename, never corrupt
   the live config file.
7. **Privacy warning** — disk persistence can only be activated after the user explicitly confirms
   the inline privacy warning in the settings panel.
8. **Search state cleared on close** — `SearchBar::query` is always reset to `""` when the UI
   window closes.
9. **No comments unless non-obvious** — do not add explanatory comments. Only add a comment when
   the WHY is non-obvious: a hidden constraint, a workaround for a specific bug, or a subtle
   invariant.
10. **100% test coverage** — all lines in all Rust source files must be covered by tests. Use
    mock implementations for platform-specific I/O. Use `cargo-llvm-cov` for measurement.
