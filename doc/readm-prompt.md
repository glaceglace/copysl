# README Generation Prompt for Copysl

> **Usage**: Feed this file to an LLM to generate `README.md`.
> The output should be a self-contained README that a new user can follow
> from zero to a working clipboard manager without reading the source code.

---

## Your task

Write a `README.md` for **Copysl**, a Linux clipboard history manager written
in pure Rust. Use only the facts given below — do not invent features,
commands, or file paths. All information comes from the actual source code as
of the most recent commit.

The README must cover, in this order:

1. Project tagline + one-paragraph description
2. Screenshots / demo (placeholder callout — no real screenshots exist yet)
3. Features
4. Prerequisites
5. Building from source
6. Installation
7. Running / usage
8. Configuration reference
9. Keyboard shortcuts
10. Wayland-specific notes
11. Troubleshooting
12. Architecture overview (brief)
13. Contributing
14. License (placeholder — license not yet chosen)

Use GitHub-flavored Markdown. Prefer tables for reference sections.
Keep prose tight — one clear sentence beats three vague ones.

---

## Project facts (source: actual code)

### Identity

| Field | Value |
|---|---|
| Binary name | `copysl` |
| Project name (display) | Copysl |
| Crate package name | `copysl` |
| Language | Rust (Cargo workspace) |
| UI framework | egui / eframe |
| Platform | Linux only (X11 and Wayland) |
| Inspiration | Windows clipboard history (Win+V) |

### What it does

Copysl is a lightweight clipboard history manager. A long-running daemon
captures every clipboard change (plain text, rich text/HTML, images) and stores
them in an in-memory list. Pressing the global keyboard shortcut opens a
card-based history window near the cursor. The user selects an entry and it is
pasted into the previously-focused window via a synthetic Ctrl+V.

### Run modes

The single binary dispatches based on command-line flags:

| Command | Behaviour |
|---|---|
| `copysl` | Start daemon (if not already running) **and** open the UI window immediately. The daemon keeps running after the UI closes. |
| `copysl --daemon` | Start daemon only, no UI. Use this for autostart / systemd / `.desktop` entries. |
| `copysl --ui` | Open UI window only; starts the daemon silently if it is not already running. |
| `copysl --debug` | Combinable with any mode. Sets `RUST_LOG=debug` (unless `RUST_LOG` is already set) so internal decisions are printed to stderr. |

`--debug` can be combined with other flags, e.g. `copysl --daemon --debug`.

### Global shortcut

The global shortcut is **not yet configurable via the UI** (the shortcut field
exists in `Config` but the shortcut-listener module is still being implemented).
The design target is `Super+V`. Update this section once the shortcut listener
is wired up.

### Configuration file

**Path**: `~/.config/copysl/config.toml`

Created automatically with defaults on first run. Edited by the in-app settings
panel or directly in a text editor.

| Key | Type | Default | Description |
|---|---|---|---|
| `max_entries` | integer (≥ 10) | `200` | Maximum clipboard entries stored. Oldest non-pinned entries are evicted when the limit is reached. Pinned entries are never evicted. |
| `persist_history` | bool | `false` | Save history to `~/.copysl/history.db` (SQLite). Disabled by default for privacy — the settings panel shows a warning before enabling. |
| `autostart` | bool | `true` | Write/remove `~/.config/autostart/copysl.desktop` so the daemon starts on login. |
| `window_position` | `"NearCursor"` or `"Fixed"` | `"NearCursor"` | Where the UI window appears. |
| `window_position_x` | integer | — | X coordinate when `window_position = "Fixed"`. |
| `window_position_y` | integer | — | Y coordinate when `window_position = "Fixed"`. |
| `theme` | `"System"`, `"Light"`, `"Dark"` | `"System"` | UI colour theme. |
| `paste_delay_ms` | integer (10–3000) | `150` | **Wayland only.** Milliseconds the daemon waits after the Copysl window closes before injecting Ctrl+V, giving the window manager time to return focus to the target app. Ignored on X11 (which uses direct window addressing). Increase if paste lands in the wrong window; decrease for snappier feel. |

Example minimal config:

```toml
max_entries = 100
persist_history = false
autostart = true
window_position = "NearCursor"
theme = "System"
paste_delay_ms = 150
```

Config writes are atomic: the daemon writes to `.config.toml.tmp` then renames
it, so a crash never corrupts the live file.

### File locations

| File | Path | Notes |
|---|---|---|
| Config | `~/.config/copysl/config.toml` | Created with defaults on first run |
| History database | `~/.copysl/history.db` | SQLite; only created when `persist_history = true` |
| Autostart entry | `~/.config/autostart/copysl.desktop` | Written/removed by the daemon based on the `autostart` setting |
| IPC socket (primary) | `$XDG_RUNTIME_DIR/copysl.sock` | Created by the daemon on startup |
| IPC socket (fallback) | `/tmp/copysl-<uid>.sock` | Used when `XDG_RUNTIME_DIR` is not set |

### IPC

Daemon and UI communicate over a Unix domain socket:

- **Primary path**: `$XDG_RUNTIME_DIR/copysl.sock`
- **Fallback path**: `/tmp/copysl-<uid>.sock`

### Autostart

When `autostart = true` (default), the daemon writes:
`~/.config/autostart/copysl.desktop`

This file uses the XDG autostart spec, so it works on GNOME, KDE, XFCE, and
any desktop that honours `~/.config/autostart/`.

### Clipboard content types

| Type | Description |
|---|---|
| Plain text | UTF-8 strings |
| Rich text | HTML markup (stored with a plain-text preview for display) |
| Images | PNG or JPEG raster images |

### UI features

- Borderless floating window, appears near the cursor.
- Search bar (always visible at top) filters in real time.
- Cards show: content preview, relative timestamp, pin icon.
- Image cards show an 80 px thumbnail; hover for a larger preview.
- Right-click context menu: Pin/Unpin, Delete, Copy.
- Settings panel (gear icon) — overlay, not a separate window.
- Tool-check popup on startup if required external tools are missing.
- Drag the window by its title area.
- Window closes on: Escape, click outside, or selecting an entry.

### Keyboard shortcuts (UI window)

| Key | Action |
|---|---|
| `↑` / `↓` | Move selection between cards |
| `Enter` | Paste selected entry and close window |
| `Delete` | Delete selected entry |
| `Escape` | Close window without action |
| `Ctrl+F` | Focus search bar |
| Any printable character | Focus search bar and start filtering |

### Paste mechanism

1. The selected entry's content is written to the system clipboard.
2. The Copysl window closes; focus returns to the previously active window.
3. A synthetic Ctrl+V is injected into that window.

The daemon auto-selects the best available injection backend at startup:

| Display server | Priority order |
|---|---|
| X11 only | xdotool → xcb XTest (built-in) |
| Wayland (pure, no DISPLAY) | ydotool → wtype → notification fallback |
| Wayland + XWayland (DISPLAY set) | ydotool → wtype → xdotool → xcb XTest (built-in) |

Clipboard persistence (so the clipboard survives after Copysl closes):

| Display server | Priority order |
|---|---|
| Wayland | `wl-copy` (wl-clipboard package) → arboard built-in |
| X11 | `xclip` → arboard built-in |

arboard built-in is always available but less reliable (its background thread
dies when the handle is dropped).

### External tool requirements

**Critical** (paste silently fails without these):

| Situation | Required tool |
|---|---|
| Pure Wayland (no `DISPLAY`) | `ydotool` **and** `ydotoold` daemon running, OR `wtype` |
| `ydotool` installed but `ydotoold` daemon not running | Start/enable `ydotoold` |

**Recommended** (paste works but is degraded without these):

| Situation | Recommended tool |
|---|---|
| Wayland + XWayland, no Wayland inject tool | `ydotool` or `wtype` (only XWayland apps work without them) |
| No persistent clipboard daemon | `wl-copy` (Wayland) or `xclip` (X11) |

The app displays a startup popup listing any missing tools with copy-paste
install commands for the detected package manager (dnf, apt, pacman, zypper).

**ydotoold setup** (needed if you use ydotool):

```bash
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/ydotoold.service << 'EOF'
[Unit]
Description=ydotoold input injection daemon

[Service]
ExecStart=ydotoold
Restart=always

[Install]
WantedBy=default.target
EOF
systemctl --user daemon-reload
systemctl --user enable --now ydotoold
```

### Build prerequisites

- **Rust toolchain**: stable, edition 2021. Install via [rustup](https://rustup.rs).
- **System libraries** required at compile time:
  - `xcb` development headers (package: `libxcb-devel` on Fedora/RHEL, `libxcb-dev` on Debian/Ubuntu)
  - OpenSSL (usually already present)
- **Cargo workspace**: four crates — `common`, `config`, `daemon`, `ui`.
- SQLite is bundled via `rusqlite` with the `bundled` feature — no system SQLite needed.

### Runtime dependencies (optional but recommended)

Install the tools that match your display server. See the External tool
requirements section above.

### Building

```bash
git clone <repo-url>
cd copieur

# Debug build
cargo build --workspace

# Release build (recommended for daily use)
cargo build --workspace --release
```

Binary location after build: `target/release/copysl`

### Tests

```bash
cargo test --workspace
```

Platform-specific modules (X11, Wayland) use mock implementations in tests
so a display server is not required to run the test suite.

### Installing

There is no installer yet. Copy the binary to a directory on `$PATH`:

```bash
sudo cp target/release/copysl /usr/local/bin/copysl
```

Then run `copysl` once to create the config file and autostart entry.

---

## Tone and style guidelines

- Address the reader as "you".
- Use active voice.
- Lead each section with the most important sentence.
- For the Wayland section, be specific about `ydotoold` — many users install
  ydotool but forget the daemon, and it is a common source of confusion.
- Mark anything that is not yet implemented with a clear "not yet implemented"
  note rather than omitting it or implying it works.
- Do not mention internal implementation details (crate names, struct names,
  module paths) unless they are directly actionable for the user.
