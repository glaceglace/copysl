# Copysl — Clipboard History Manager for Linux

A lightweight, keyboard-driven clipboard history manager inspired by Windows' Win+V experience, built entirely in Rust. A background daemon silently captures every clipboard change; press your global shortcut to open a searchable card view, pick an entry, and it pastes directly into your active window.

---

## Screenshots / Demo

> **No screenshots yet.** Contributions welcome — open a PR with a recording or image.

---

## Features

- Captures plain text, rich text (HTML), and images automatically
- Card-based history window appears near your cursor
- Real-time search across all entries
- Pin entries so they are never evicted
- Right-click context menu: Pin/Unpin, Delete, Copy
- Image thumbnails with hover-to-enlarge preview
- Configurable max history size (oldest non-pinned entries evicted first)
- Optional persistent history via SQLite (opt-in, off by default)
- XDG autostart — daemon starts on login automatically
- Works on X11, Wayland, and Wayland + XWayland

---

## Prerequisites

### Build dependencies

| Dependency | Notes |
|---|---|
| Rust (stable, edition 2021) | Install via [rustup](https://rustup.rs) |
| `libxcb` development headers | `libxcb-devel` (Fedora/RHEL) · `libxcb-dev` (Debian/Ubuntu) |
| OpenSSL | Usually pre-installed |

SQLite is bundled — no system SQLite package needed.

### Runtime tools (install what matches your display server)

**X11**

| Tool | Purpose |
|---|---|
| `xdotool` | Key injection (preferred) |
| `xclip` | Clipboard persistence (recommended) |

**Wayland (pure — no `DISPLAY`)**

| Tool | Purpose |
|---|---|
| `ydotool` + `ydotoold` daemon | Key injection (**critical** — paste silently fails without this or `wtype`) |
| `wtype` | Key injection alternative |
| `wl-clipboard` (`wl-copy`) | Clipboard persistence (recommended) |

**Wayland + XWayland (`DISPLAY` set)**

Same as pure Wayland, but `xdotool` / xcb XTest are used as fallbacks for XWayland apps when no Wayland inject tool is present.

Copysl shows a startup popup listing any missing tools with copy-paste install commands for your package manager (dnf, apt, pacman, zypper).

---

## Building from Source

```bash
git clone <repo-url>
cd copieur

# Release build (recommended for daily use)
cargo build --workspace --release

# Debug build
cargo build --workspace
```

The binary is at `target/release/copysl`.

### Running tests

```bash
cargo test --workspace
```

Platform-specific modules use mock implementations so a display server is not required to run the test suite.

---

## Installation

There is no installer yet. Copy the binary to a directory on `$PATH`:

```bash
sudo cp target/release/copysl /usr/local/bin/copysl
```

Then run `copysl` once to create the config file and the XDG autostart entry.

---

## Running / Usage

The single `copysl` binary selects its behaviour from command-line flags:

| Command | Behaviour |
|---|---|
| `copysl` | Start the daemon (if not already running) **and** open the UI window immediately. The daemon keeps running after the UI closes. |
| `copysl --daemon` | Start the daemon only, no UI. Use this for autostart / systemd / `.desktop` entries. |
| `copysl --ui` | Open the UI window only; starts the daemon silently if it is not already running. |
| `copysl --debug` | Combinable with any mode. Sets `RUST_LOG=debug` (unless already set) so decisions are printed to stderr. |

`--debug` can be combined freely, e.g. `copysl --daemon --debug`.

---

## Configuration Reference

**File**: `~/.config/copysl/config.toml`

Created automatically with defaults on first run. Edit it directly or use the in-app settings panel (gear icon).

| Key | Type | Default | Description |
|---|---|---|---|
| `max_entries` | integer (≥ 10) | `200` | Maximum entries stored. Oldest non-pinned entries are evicted when the limit is reached. Pinned entries are never evicted. |
| `persist_history` | bool | `false` | Save history to `~/.copysl/history.db` (SQLite). Disabled by default for privacy — the settings panel shows a warning before enabling. |
| `autostart` | bool | `true` | Write/remove `~/.config/autostart/copysl.desktop` so the daemon starts on login. |
| `window_position` | `"NearCursor"` or `"Fixed"` | `"NearCursor"` | Where the UI window appears. |
| `window_position_x` | integer | — | X coordinate when `window_position = "Fixed"`. |
| `window_position_y` | integer | — | Y coordinate when `window_position = "Fixed"`. |
| `theme` | `"System"`, `"Light"`, `"Dark"` | `"System"` | UI colour theme. |
| `paste_delay_ms` | integer (10–3000) | `150` | **Wayland only.** Milliseconds to wait after the Copysl window closes before injecting Ctrl+V. Gives the window manager time to return focus to the target app. Ignored on X11. Increase if paste lands in the wrong window; decrease for a snappier feel. |

Minimal example:

```toml
max_entries = 100
persist_history = false
autostart = true
window_position = "NearCursor"
theme = "System"
paste_delay_ms = 150
```

Config writes are atomic — Copysl writes to a temporary file then renames it, so a crash never corrupts the live config.

### File locations

| File | Path |
|---|---|
| Config | `~/.config/copysl/config.toml` |
| History database | `~/.copysl/history.db` (only when `persist_history = true`) |
| Autostart entry | `~/.config/autostart/copysl.desktop` |
| IPC socket | `$XDG_RUNTIME_DIR/copysl.sock` (fallback: `/tmp/copysl-<uid>.sock`) |

---

## Keyboard Shortcuts

### UI window

| Key | Action |
|---|---|
| `↑` / `↓` | Move selection between cards |
| `Enter` | Paste selected entry and close window |
| `Delete` | Delete selected entry |
| `Escape` | Close window without action |
| `Ctrl+F` | Focus search bar |
| Any printable character | Focus search bar and start filtering |

### Global shortcut

> **Not yet implemented.** The design target is `Super+V`. This section will be updated once the shortcut listener is wired up.

---

## Wayland-Specific Notes

Wayland does not allow applications to inject keystrokes without an input-injection daemon. You must install one of the following:

**Option A — ydotool (recommended)**

Install `ydotool`, then set up the `ydotoold` background daemon. Many users install `ydotool` but forget to start `ydotoold` — paste will silently fail without it.

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

**Option B — wtype**

Install `wtype`. No daemon required.

**Clipboard persistence on Wayland**

Install `wl-clipboard` (`wl-copy`) so clipboard contents survive after the Copysl window closes. Without it Copysl falls back to an internal mechanism that is less reliable.

**`paste_delay_ms` tuning**

If paste lands in the wrong window, increase `paste_delay_ms` in your config. If paste feels slow, decrease it. The default of `150` ms works for most setups.

---

## Troubleshooting

| Symptom | Likely cause | Fix |
|---|---|---|
| Paste silently does nothing (Wayland) | `ydotoold` daemon not running | Enable it: `systemctl --user enable --now ydotoold` |
| Paste lands in the wrong window | `paste_delay_ms` too low | Increase `paste_delay_ms` in config |
| Clipboard content disappears after closing | No clipboard persistence daemon | Install `wl-copy` (Wayland) or `xclip` (X11) |
| Startup popup about missing tools | Required inject tool not installed | Follow the install commands shown in the popup |
| History not saved between reboots | `persist_history` is `false` | Enable it in the settings panel or config file |

---

## Architecture Overview

Copysl is a Cargo workspace of four crates:

- **`common`** — shared types used by both the daemon and UI
- **`config`** — reads and writes `config.toml`; provides typed config access
- **`daemon`** — long-running process that monitors the clipboard, manages history, and handles IPC over a Unix domain socket
- **`ui`** — egui/eframe frontend that connects to the daemon over IPC and renders the card view

Daemon and UI communicate exclusively through the IPC socket, so the UI can be opened and closed freely without affecting clipboard capture. The daemon auto-selects the best available key-injection and clipboard-persistence backend at startup based on the detected display server and installed tools.

---

## Contributing

1. Fork the repository and create a feature branch.
2. Run `cargo test --workspace` — all tests must pass.
3. Open a pull request describing what you changed and why.

Issues and PRs are welcome for bug reports, missing tool support, or new features.

---

## License

> **License not yet chosen.** This section will be updated once a license is selected.
