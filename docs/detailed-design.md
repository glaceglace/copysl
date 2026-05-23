# Copieur — Detailed Design Document

> Based on `demands.md`. Decisions made during design clarification are marked **[Decision]**.

---

## 1. Decisions Log

| Topic | Decision | Rationale |
|---|---|---|
| IPC mechanism | Unix domain socket | Lightweight, no shared-memory synchronization complexity, works on macOS for future portability |
| Binary layout | Single binary, `--daemon` flag | Simpler distribution; UI and daemon are cleanly separated in code but ship as one executable |
| Focus tracking | Daemon tracks continuously | Daemon has the lifecycle to observe window focus at all times, not just at UI startup |
| Settings storage | TOML file at `~/.config/copieur/config.toml` | Human-editable, XDG-standard, decoupled from optional SQLite history |
| Image labels | Dropped from v1 | Reduced scope; image entries are hidden when search query is non-empty |
| Search state | Cleared every time the UI closes | Avoids stale search on re-open |
| Wayland shortcut fallback | Try ydotool/libinput; if unavailable, send desktop notification | Best-effort; graceful degradation |
| macOS future support | Use platform abstractions throughout | `arboard`, Unix sockets, and TOML config are all macOS-compatible; isolate platform-specific code behind traits |

---

## 2. High-Level Architecture

```
 ┌───────────────────────────────────────────────────────┐
 │                   SINGLE BINARY                       │
 │   copieur --daemon   │   copieur (UI mode, default)   │
 └──────────┬───────────┴──────────────┬─────────────────┘
            │                          │
 ╔══════════▼═══════════╗   Unix sock  ╔══════════▼════════╗
 ║     DAEMON PROCESS   ║◄────────────►║    UI PROCESS     ║
 ║                      ║             ║                   ║
 ║  ClipboardMonitor    ║             ║  App (eframe)     ║
 ║  HistoryStore        ║             ║  CardList         ║
 ║  FocusTracker        ║             ║  SearchBar        ║
 ║  ShortcutListener    ║             ║  SettingsPanel    ║
 ║  IpcServer           ║             ║  IpcClient        ║
 ║  PasteExecutor       ║             ║                   ║
 ║  Persistence         ║             ╚═══════════════════╝
 ║  AutostartManager    ║
 ╚══════════════════════╝
            │
      ┌─────┴──────┐
      │ Config     │  (~/.config/copieur/config.toml)
      │ History DB │  (~/.local/share/copieur/history.db, optional)
      └────────────┘
```

The daemon is the single source of truth. The UI is a short-lived display layer that is spawned on demand and exits after user interaction.

---

## 3. Module Breakdown

### 3.1 `common` — Shared Types and Protocol

This crate/module is compiled into both the daemon and the UI. It contains no platform-specific code.

#### 3.1.1 `ClipboardEntry`

```rust
pub struct EntryId(pub u64);  // monotonically increasing, assigned by daemon

pub enum ContentPayload {
    PlainText(String),
    RichText { html: String, plain_preview: String },
    Image { data: Vec<u8>, mime: ImageMime },   // PNG or JPEG bytes
}

pub enum ImageMime { Png, Jpeg }

pub struct ClipboardEntry {
    pub id: EntryId,
    pub payload: ContentPayload,
    pub captured_at: SystemTime,
    pub pinned: bool,
}
```

When the clipboard contains multiple formats simultaneously (e.g. both HTML and plain text), all are stored in a single `ClipboardEntry` as `ContentPayload::RichText` so the correct format is restored at paste time.

#### 3.1.2 `IpcMessage` — Wire Protocol

Serialization: **bincode** (compact binary, fast, pure Rust). Each message is framed with a 4-byte little-endian length prefix.

```rust
pub enum DaemonRequest {
    GetHistory { offset: usize, limit: usize },
    PasteEntry { id: EntryId },
    DeleteEntry { id: EntryId },
    PinEntry   { id: EntryId },
    UnpinEntry { id: EntryId },
    ClearHistory { include_pinned: bool },
    GetConfig,
    UpdateConfig(Config),
}

pub enum DaemonResponse {
    History(Vec<ClipboardEntry>),
    Config(Config),
    Ok,
    Err(String),
    // Push notification: sent by daemon to open UI when new entry arrives
    NewEntry(ClipboardEntry),
}
```

The daemon can push `NewEntry` unsolicited while the UI is connected, allowing live updates without polling.

#### 3.1.3 `Config`

```rust
pub struct Config {
    pub shortcut: KeyCombo,          // default: Super+V
    pub max_entries: usize,          // default: 200, min: 10
    pub persist_history: bool,       // default: false
    pub autostart: bool,             // default: true
    pub window_position: WindowPos,  // NearCursor | Fixed(x, y)
    pub theme: Theme,                // System | Light | Dark
}

pub enum WindowPos { NearCursor, Fixed(i32, i32) }
pub enum Theme { System, Light, Dark }
```

Loaded from `~/.config/copieur/config.toml` at daemon startup. The UI receives the live `Config` via `GetConfig` IPC. Config changes flow: UI sends `UpdateConfig` → daemon validates, applies in memory, and writes TOML to disk.

---

### 3.2 `daemon` — Background Process

Entry point when invoked as `copieur --daemon`. Starts all subsystems, blocks on an async runtime (tokio), and runs until killed.

#### 3.2.1 `daemon::clipboard_monitor`

**Responsibility**: Detect clipboard changes and forward new entries to `HistoryStore`.

**Implementation**:
- Uses `arboard::Clipboard` for cross-platform access (X11 and Wayland on Linux; macOS-ready for the future).
- Runs on a dedicated thread (arboard is not async-friendly).
- Polling interval: 200 ms (configurable compile-time constant, not exposed to user).
- On each poll, reads all available formats in priority order: Image → RichText (HTML) → PlainText.
- Computes a content hash (SHA-256 of raw bytes) to detect actual changes.
- If hash differs from the last captured hash, constructs a `ClipboardEntry` and sends it to `HistoryStore` via a channel.

**Platform notes**:
- Arboard abstracts X11/Wayland/macOS. No direct protocol code needed here.
- On Wayland, arboard uses the `wl_data_device` protocol internally.

```
ClipboardMonitor thread
  │
  ├── poll every 200ms
  ├── read clipboard (arboard)
  ├── hash content
  ├── if changed: construct ClipboardEntry
  └── send via mpsc → HistoryStore
```

#### 3.2.2 `daemon::history_store`

**Responsibility**: Authoritative in-memory store of clipboard history. All reads and writes go through this module.

**Data structure**:
```rust
struct HistoryStore {
    entries: VecDeque<ClipboardEntry>,  // ordered: newest first
    id_counter: u64,
    config: Arc<RwLock<Config>>,
    persistence: Option<PersistenceHandle>,
}
```

**Operations**:

| Operation | Behavior |
|---|---|
| `push(entry)` | Deduplicate: if same content hash exists, move that entry to front (update `captured_at`). Otherwise prepend. Evict oldest non-pinned entry if `entries.len() > max_entries`. |
| `delete(id)` | Remove entry by id. |
| `pin(id)` | Set `pinned = true`. Pinned entries always sort before unpinned in list view (sort is applied at read time, not in the VecDeque). |
| `unpin(id)` | Set `pinned = false`. |
| `clear(include_pinned)` | Remove all non-pinned entries. If `include_pinned`, remove all. |
| `get_page(offset, limit)` | Return entries sorted: pinned first (by insertion order), then unpinned (by insertion order, newest first). |

**Eviction rule**: The oldest unpinned entry is the last entry in the unpinned section. Pinned entries are never evicted automatically.

**Thread safety**: `HistoryStore` is owned by a single async task. All access is via message passing (mpsc channels from `ClipboardMonitor` and `IpcServer`).

If persistence is enabled, every mutating operation sends a delta to `daemon::persistence` after applying it in memory.

#### 3.2.3 `daemon::focus_tracker`

**Responsibility**: Continuously track which window has focus so that, when the user selects a clipboard entry, we know where to restore focus and send Ctrl+V.

**X11 implementation**:
- Connect to the X server via `xcb`.
- Subscribe to `PropertyNotify` events on the root window.
- When `_NET_ACTIVE_WINDOW` changes, read the new window id and store it.
- Ignore changes where the new active window is the Copieur UI window itself.

**Wayland implementation**:
- Use the `ext-foreign-toplevel-list-v1` protocol to enumerate toplevels and detect focus changes.
- If the compositor does not support this protocol, fall back to recording the focused window at the moment the shortcut fires (best-effort; may be `None`).

**Output**: A `FocusHandle` that `PasteExecutor` can use.

```rust
pub struct FocusHandle {
    /// X11: window id. Wayland: toplevel handle.
    pub inner: PlatformFocusHandle,
}
```

#### 3.2.4 `daemon::shortcut_listener`

**Responsibility**: Capture the global keyboard shortcut and signal the UI to open.

**X11 implementation**:
- Use `xcb` to call `XGrabKey` on the root window for the configured key combo.
- Listen for `KeyPress` events in the X event loop.
- On match, record the focused window (from `FocusTracker`) and spawn/signal the UI process.

**Wayland implementation (in priority order)**:
1. Try `ext-global-shortcuts-v1` compositor protocol.
2. If unavailable, try opening `/dev/input/event*` via `evdev` (requires permission or `input` group membership) to intercept the key.
3. If that also fails, send a desktop notification: "Copieur: global shortcut not available on this compositor. Use `copieur` CLI or tray icon to open clipboard history."

**Spawning the UI**:
- The daemon forks a child process: `exec copieur` (same binary, no `--daemon` flag).
- The daemon passes the socket path via environment variable `COPIEUR_SOCKET`.
- The child connects to the daemon socket and renders the UI.

If a UI process is already open, the shortcut closes it (toggle behavior).

#### 3.2.5 `daemon::ipc_server`

**Responsibility**: Accept and serve IPC connections from UI processes.

**Socket path**: `$XDG_RUNTIME_DIR/copieur.sock` (fallback: `/tmp/copieur-$UID.sock`).

**Protocol**: One connection per UI instance. Each connection is handled in its own async task. Messages are length-prefixed bincode frames.

**Request handling**:

| Request | Handler |
|---|---|
| `GetHistory` | Forward to `HistoryStore`, return page |
| `PasteEntry` | Tell `HistoryStore` nothing; call `PasteExecutor::paste(id)` |
| `DeleteEntry` | Call `HistoryStore::delete(id)` |
| `PinEntry` | Call `HistoryStore::pin(id)` |
| `UnpinEntry` | Call `HistoryStore::unpin(id)` |
| `ClearHistory` | Call `HistoryStore::clear(include_pinned)` |
| `GetConfig` | Return current `Config` |
| `UpdateConfig` | Validate, update in memory, write TOML to disk |

**Push notifications**: After each `GetHistory` response, the server keeps the connection open. When `HistoryStore` receives a new entry, the server sends a `NewEntry` push to all open UI connections.

#### 3.2.6 `daemon::paste_executor`

**Responsibility**: Restore clipboard content and inject a synthetic Ctrl+V keystroke.

**Steps**:
1. Write the selected `ClipboardEntry`'s content to the system clipboard via `arboard`.
2. Wait ~50 ms for the UI window to close and focus to return to the previous window (tracked by `FocusTracker`).
3. Inject synthetic Ctrl+V:
   - **X11**: Execute `xdotool key --clearmodifiers ctrl+v`; if `xdotool` is absent, use `XSendEvent` via xcb.
   - **Wayland**: Execute `ydotool key 29:1 47:1 47:0 29:0`; if `ydotool` is absent, fall through to fallback.
4. **Fallback**: Send a desktop notification (via `notify-rust`): "Copieur: content copied to clipboard — press Ctrl+V to paste."

**Platform abstraction**: The paste step is behind a `PasteBackend` trait:
```rust
trait PasteBackend: Send + Sync {
    fn inject_paste(&self, focus: &FocusHandle) -> Result<(), PasteError>;
}
```
Implementations: `XdotoolBackend`, `XSendEventBackend`, `YdotoolBackend`, `NotificationFallbackBackend`.

#### 3.2.7 `daemon::persistence`

**Responsibility**: Optional SQLite-backed history storage.

**Activation**: Enabled when `Config::persist_history = true`. The setting can only be toggled through the UI, which presents a privacy warning that the user must explicitly acknowledge before the IPC `UpdateConfig` is sent.

**Database location**: `~/.local/share/copieur/history.db`.

**Schema**:
```sql
CREATE TABLE entries (
    id          INTEGER PRIMARY KEY,
    content_type TEXT NOT NULL,  -- 'text' | 'richtext' | 'image'
    payload     BLOB NOT NULL,   -- bincode-encoded ContentPayload
    captured_at INTEGER NOT NULL, -- Unix timestamp ms
    pinned      INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT NOT NULL
);
```

**Operation**:
- On daemon start with persistence enabled: load all rows into `HistoryStore`.
- On each mutation: apply the SQL change in a write-ahead-log (WAL) transaction.
- The `HistoryStore` calls into `PersistenceHandle` (an async channel to the SQLite writer task).

SQLite access is on a single dedicated thread to avoid lock contention (`rusqlite` is not `Send`).

#### 3.2.8 `daemon::autostart_manager`

**Responsibility**: Register or deregister the daemon as an autostart application.

**XDG autostart**: Write/delete `~/.config/autostart/copieur.desktop`.

```ini
[Desktop Entry]
Type=Application
Name=Copieur
Exec=/path/to/copieur --daemon
Hidden=false
X-GNOME-Autostart-enabled=true
```

**Systemd user unit** (alternative, not v1): Write `~/.config/systemd/user/copieur.service` and call `systemctl --user enable copieur`. Deferred to v2.

The `autostart` config toggle calls `AutostartManager::enable()` or `::disable()` when changed.

---

### 3.3 `ui` — Display Layer

Entry point when invoked as `copieur` (no `--daemon` flag). Connects to the daemon socket, renders the UI, then exits.

#### 3.3.1 `ui::app`

Top-level `eframe::App` implementation. Holds all UI state.

```rust
struct CopieurApp {
    ipc: IpcClient,
    entries: Vec<ClipboardEntry>,   // fetched from daemon on start
    filtered: Vec<usize>,           // indices into entries after search
    search_query: String,
    selected_idx: Option<usize>,    // currently highlighted card
    show_settings: bool,
    config: Config,
    pending_action: Option<UiAction>,
}

enum UiAction {
    Paste(EntryId),
    Delete(EntryId),
    Pin(EntryId),
    Unpin(EntryId),
    ClearHistory { include_pinned: bool },
    SaveConfig(Config),
}
```

**Lifecycle**:
1. On `new()`: connect to `COPIEUR_SOCKET`, send `GetHistory`, send `GetConfig`, render immediately.
2. Each frame: check for pushed `NewEntry` from daemon (non-blocking read on socket); if received, prepend to `entries` and re-apply search filter.
3. On close: reset `search_query = ""`, close socket. The daemon remains running.

**Close triggers**:
- User selects an entry (Enter or click on card).
- User presses Escape.
- User clicks outside the window (detected via `ctx.input().pointer.any_click()` when pointer is outside the window rect).

#### 3.3.2 `ui::window`

Configures the eframe window to match the demands:

```rust
fn build_native_options(cursor_pos: Option<(i32, i32)>, config: &Config) -> eframe::NativeOptions {
    NativeOptions {
        decorated: false,           // no OS title bar
        resizable: false,
        always_on_top: true,
        skip_taskbar: true,         // no taskbar entry
        initial_window_pos: compute_window_pos(cursor_pos, config),
        initial_window_size: Some(Vec2::new(380.0, 520.0)),
        ..Default::default()
    }
}
```

`compute_window_pos`: if `WindowPos::NearCursor`, place the window so the top-left corner is at the cursor position, adjusted to stay within the screen bounds.

The window is excluded from the Alt+Tab switcher via `skip_taskbar: true` (eframe/winit exposes this).

#### 3.3.3 `ui::components::search_bar`

- Rendered at the top of every frame.
- Focus rules:
  - Auto-focused on window open.
  - `Ctrl+F`: explicitly focus if not already focused.
  - Any printable character typed while a card is highlighted: redirect keystroke to search bar and start filtering.
- On text change: recompute `filtered` indices via case-insensitive substring match on the text content of each entry (image entries are excluded from results when query is non-empty).
- On close: `search_query` is cleared (not persisted).

#### 3.3.4 `ui::components::card_list`

Renders the scrollable list of `ClipboardEntry` items. Order: pinned entries first (by insertion order), then unpinned (newest first). The ordering is computed from `entries` at render time, not mutated in-place.

Keyboard navigation:
- `↑` / `↓`: move `selected_idx`.
- `Enter`: emit `UiAction::Paste(id)` and close window.
- `Delete`: emit `UiAction::Delete(id)`.
- `Escape`: close window without action.

Scrolling: the highlighted card is always scrolled into view.

#### 3.3.5 `ui::components::card`

Each card is a fixed-height `egui::Frame` with the following layout:

```
┌──────────────────────────────────────────────┐
│ [content preview]            [timestamp] [📌] │
│                                       [✕ btn] │
└──────────────────────────────────────────────┘
```

**Text card**: First ~2 lines, `…` if truncated. `egui::Tooltip` on hover shows full text.

**Rich text card**: Plain-text preview (HTML stripped at capture time, stored in `plain_preview`), with a small grey "HTML" badge in the top-right area.

**Image card**: Fixed 80 px tall thumbnail rendered via `egui::Image`. Tooltip shows the full image at a capped resolution (e.g. 400 px wide).

**Common elements**:
- Timestamp: top-right, formatted as relative time ("just now", "2 min ago", "3 h ago", "yesterday"). Absolute time on tooltip.
- Pin icon: shown only if `pinned = true`.
- Delete button (✕): visible only on hover. On click, emits `UiAction::Delete(id)`.
- Right-click context menu (via `egui::popup_above_or_below_widget`):
  - Pin / Unpin
  - Delete
  - Copy (re-writes to clipboard without pasting)

#### 3.3.6 `ui::components::settings_panel`

Opened via a gear icon in the bottom-right of the main window. Renders as an overlay panel (not a separate window).

Fields rendered:
- **Global shortcut**: text input showing current combo; click to re-bind (capture next key press).
- **Max history entries**: numeric input, min 10.
- **Persist history to disk**: toggle switch. When turned ON, show an inline warning box before applying:
  > ⚠️ Enabling this saves clipboard contents (including passwords and tokens) to disk. Confirm to proceed.
  User must click "I understand, enable" to send `UpdateConfig`. If they dismiss, the toggle reverts.
- **Autostart on login**: toggle; calls `AutostartManager`.
- **Window position**: dropdown (Near cursor / Fixed).
- **UI theme**: dropdown (System / Light / Dark).

On any change, the panel sends `UpdateConfig` to the daemon, which applies it and writes TOML.

#### 3.3.7 `ui::ipc_client`

```rust
struct IpcClient {
    stream: UnixStream,   // std::os::unix::net::UnixStream (set non-blocking for push reads)
}

impl IpcClient {
    fn send(&mut self, req: DaemonRequest) -> Result<DaemonResponse>;
    fn try_recv_push(&mut self) -> Option<DaemonResponse>;  // non-blocking
}
```

- Socket path is read from `COPIEUR_SOCKET` env var (set by daemon when spawning UI).
- `try_recv_push` is called each frame to collect any pushed `NewEntry` messages without blocking the render loop.

---

### 3.4 `config` — Configuration I/O

Single module used by both daemon and UI (via IPC).

**File**: `~/.config/copieur/config.toml`

**Read path**: Daemon loads on startup. Parsed with `toml` crate. If missing, writes defaults.

**Write path**: Daemon writes after `UpdateConfig` IPC is processed. Writes atomically: write to `.config.toml.tmp`, then `rename`.

**Default content**:
```toml
shortcut = "Super+V"
max_entries = 200
persist_history = false
autostart = true
window_position = "NearCursor"
theme = "System"
```

---

## 4. Module Dependency Graph

```
common
  └── used by: all modules (types + IPC protocol)

config
  ├── depends on: common (Config struct)
  └── used by: daemon (startup + UpdateConfig), autostart_manager

daemon::clipboard_monitor
  ├── depends on: arboard, common
  └── sends to: daemon::history_store (via channel)

daemon::history_store
  ├── depends on: common, config (max_entries)
  ├── receives from: clipboard_monitor
  ├── calls: daemon::persistence (optional)
  └── serves: daemon::ipc_server (via channel)

daemon::focus_tracker
  ├── depends on: xcb (X11) / wayland-client (Wayland)
  └── serves: daemon::paste_executor, daemon::shortcut_listener

daemon::shortcut_listener
  ├── depends on: xcb (X11) / wayland-client + evdev (Wayland)
  ├── reads: daemon::focus_tracker
  └── spawns: UI process

daemon::ipc_server
  ├── depends on: common, tokio
  ├── reads/writes: daemon::history_store
  └── calls: daemon::paste_executor

daemon::paste_executor
  ├── depends on: arboard, xdotool/ydotool (subprocess), notify-rust
  └── reads: daemon::focus_tracker

daemon::persistence
  ├── depends on: rusqlite, common
  └── called by: daemon::history_store

daemon::autostart_manager
  ├── depends on: std::fs
  └── called by: daemon::ipc_server (on UpdateConfig with autostart change)

ui::ipc_client
  ├── depends on: common (IpcMessage), std::os::unix::net
  └── used by: ui::app

ui::app
  ├── depends on: eframe, ui::ipc_client, common
  └── composes: ui::window, ui::components::*

ui::components::card
  └── depends on: common (ClipboardEntry), egui

ui::components::card_list
  ├── depends on: ui::components::card
  └── used by: ui::app

ui::components::search_bar
  └── used by: ui::app

ui::components::settings_panel
  ├── depends on: common (Config)
  └── used by: ui::app
```

---

## 5. Data Flow: Main Use Case (Select and Paste)

```
1. User copies text in browser
      │
      ▼
2. ClipboardMonitor detects change (200ms poll)
      │
      ▼
3. HistoryStore::push(entry)  ← deduplication + eviction applied
      │
      ▼
4. (if persistence enabled) PersistenceHandle::upsert(entry)
      │
      ▼
5. IpcServer pushes NewEntry to any open UI connection

--- (later) ---

6. User presses Super+V
      │
      ▼
7. ShortcutListener fires
      │  FocusTracker records active window BEFORE shortcut fires
      ▼
8. Daemon spawns:  exec copieur  (COPIEUR_SOCKET=/run/user/1000/copieur.sock)
      │
      ▼
9. UI connects to socket, sends GetHistory + GetConfig
      │
      ▼
10. UI renders window near cursor position
      │
      ▼
11. User navigates with ↓↓ and presses Enter on card #3
      │
      ▼
12. UI sends PasteEntry { id: 3 } to daemon, then exits (window closes)
      │
      ▼
13. PasteExecutor:
      a. arboard::write(entry.payload)       ← content in clipboard
      b. sleep 50ms                          ← wait for focus to return
      c. xdotool key ctrl+v                  ← synthetic paste
      (fallback: desktop notification)
```

---

## 6. Data Flow: Settings Change (Toggle Persistence)

```
1. User opens settings panel (gear icon)
2. UI renders toggle for "Persist history to disk" (currently OFF)
3. User flips toggle to ON
4. UI shows inline privacy warning
5. User clicks "I understand, enable"
6. UI sends UpdateConfig { config: { persist_history: true, … } }
7. Daemon::IpcServer:
      a. Validates config
      b. Updates in-memory Config
      c. Writes ~/.config/copieur/config.toml (atomic rename)
      d. Activates PersistenceHandle: opens SQLite, writes current history
8. Daemon sends Ok response
9. UI updates its local config copy
```

---

## 7. Crate Structure

```
copieur/                        ← workspace root
├── Cargo.toml                  ← workspace manifest
├── crates/
│   ├── common/                 ← ClipboardEntry, IpcMessage, Config, EntryId
│   │   └── src/lib.rs
│   ├── config/                 ← TOML read/write
│   │   └── src/lib.rs
│   ├── daemon/
│   │   ├── src/
│   │   │   ├── main.rs         ← --daemon entry point, wires subsystems
│   │   │   ├── clipboard_monitor.rs
│   │   │   ├── history_store.rs
│   │   │   ├── focus_tracker/
│   │   │   │   ├── mod.rs      ← FocusTracker trait
│   │   │   │   ├── x11.rs
│   │   │   │   └── wayland.rs
│   │   │   ├── shortcut_listener/
│   │   │   │   ├── mod.rs      ← ShortcutListener trait
│   │   │   │   ├── x11.rs
│   │   │   │   └── wayland.rs  ← tries ext-global-shortcuts, then evdev, then notify
│   │   │   ├── ipc_server.rs
│   │   │   ├── paste_executor/
│   │   │   │   ├── mod.rs      ← PasteBackend trait + selection logic
│   │   │   │   ├── xdotool.rs
│   │   │   │   ├── xsendevent.rs
│   │   │   │   ├── ydotool.rs
│   │   │   │   └── notification_fallback.rs
│   │   │   ├── persistence.rs
│   │   │   └── autostart_manager.rs
│   └── ui/
│       └── src/
│           ├── main.rs         ← UI entry point, eframe::run_native
│           ├── app.rs          ← CopieurApp (eframe::App)
│           ├── window.rs       ← NativeOptions builder
│           ├── ipc_client.rs
│           └── components/
│               ├── search_bar.rs
│               ├── card_list.rs
│               ├── card.rs
│               └── settings_panel.rs
└── src/
    └── main.rs                 ← Binary entry: dispatches to daemon or ui main
```

The top-level `src/main.rs` reads `std::env::args()` and calls either `daemon::main()` or `ui::main()` depending on the presence of `--daemon`.

---

## 8. Platform Abstraction Strategy

Each platform-specific subsystem exposes a Rust trait. This isolates platform code and lays the groundwork for future macOS support.

| Trait | Implementations (v1) | macOS (future) |
|---|---|---|
| `ClipboardAccess` | arboard (covers all) | arboard |
| `FocusTracker` | X11 (xcb), Wayland (ext-foreign-toplevel-list or best-effort) | NSWorkspace active app notifications |
| `ShortcutListener` | X11 (XGrabKey), Wayland (ext-global-shortcuts / evdev) | CGEventTap |
| `PasteBackend` | xdotool, XSendEvent, ydotool, notification fallback | CGEventPost |
| `AutostartManager` | XDG `.desktop` file | LaunchAgents plist |

Components that are already platform-neutral: `HistoryStore`, `IpcServer`/`IpcClient` (Unix sockets), `Persistence` (SQLite), `Config` (TOML), all UI code.

---

## 9. Error Handling and Degradation

| Failure scenario | Behavior |
|---|---|
| Daemon not running when UI starts | UI shows "Copieur daemon is not running. Start with: `copieur --daemon`" and exits. |
| Socket connection lost during UI session | UI shows a brief error banner; operations that require the daemon are disabled. |
| `xdotool` / `ydotool` absent | Paste writes to clipboard; desktop notification instructs user to press Ctrl+V. |
| Wayland compositor lacks global shortcut support | Notification on first launch; shortcut feature disabled until restart with a supported compositor. |
| SQLite write fails | Log error; in-memory history continues to work. User notified via status bar in settings panel. |
| Config file parse error | Daemon falls back to defaults; logs the parse error; does not overwrite the corrupt file. |

---

## 10. Out of Scope (v1)

Carried forward from `demands.md §9`, with one addition:

- Windows or macOS support *(macOS considered in abstraction design but not implemented)*.
- Browser extension or integration.
- Cloud sync.
- Clipboard encryption at rest.
- Drag-and-drop reordering of pinned cards.
- Multi-monitor awareness beyond near-cursor placement.
- Image entry labels / search-by-label.
- Systemd user unit for autostart (XDG `.desktop` only).
