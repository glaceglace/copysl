# Copieur — Clipboard History App: Demand Document

## 1. Overview

Copieur is a lightweight clipboard history manager for Linux (X11 and Wayland). It runs as a background daemon, captures clipboard events continuously, and exposes a card-based history UI triggered by a global keyboard shortcut. The UI design is inspired by the Windows clipboard history (Win+V).

---

## 2. Framework

**Selected framework: egui (via eframe)**

Rationale:
- Pure Rust: consistent code quality, no unsafe FFI wrappers needed beyond system calls.
- Supports both X11 and Wayland via `winit`.
- Can render without a GPU using the `softbuffer` software-rasterization backend — GPU is used when available but is never required.
- Not based on Electron or WebView.
- Active ecosystem, well-documented, and well-suited to a card-list UI pattern.

---

## 3. Architecture

### 3.1 Two-process / two-thread model

| Component | Role |
|---|---|
| **Daemon** | Long-running background process. Monitors the clipboard via `arboard` (X11/Wayland abstraction). Stores history in memory. Accepts IPC commands from the UI. |
| **UI window** | Spawned on demand when the shortcut fires. Reads history from the daemon over IPC (Unix socket or shared memory). Closes itself after a selection or Escape. |

The daemon must be registered as an autostart application (XDG autostart or systemd user unit) so it survives across reboots without user action.

### 3.2 Global shortcut

- Default shortcut: **Super+V** (Linux).
- The shortcut is captured globally (daemon grabs the key on X11 via `xcb`; on Wayland via a compositor protocol such as `ext-global-shortcuts` or `zwlr-layer-shell` where available).
- The shortcut **overrides** any conflicting binding set by the desktop environment. The user is responsible for removing conflicting DE shortcuts.
- The shortcut is configurable in the app settings (see §7).

### 3.3 Paste mechanism

When a history entry is selected:
1. The UI window closes and returns focus to the previously focused window.
2. The selected content is written to the system clipboard.
3. A synthetic **Ctrl+V** keypress is injected into the previously focused window:
   - X11: via `xdotool` (must be installed) or `XSendEvent`.
   - Wayland: via `ydotool` (must be installed) or compositor-specific virtual input.
4. If synthetic input is unavailable, the daemon writes the content to the clipboard and notifies the user to paste manually (fallback).

---

## 4. Clipboard Content Types

The daemon captures and stores three content types:

| Type | Description |
|---|---|
| **Plain text** | UTF-8 strings |
| **Rich text / HTML** | HTML markup copied from browsers or rich-text editors |
| **Images** | Raster images (PNG/JPEG) copied from apps or screenshots |

When a clipboard event contains multiple formats, all available formats are stored together as a single entry so the correct format can be restored on paste.

---

## 5. UI Design

### 5.1 Window behaviour

- The window is a **borderless floating panel** (no OS title bar).
- It appears **near the current cursor position** when the shortcut fires.
- It closes when:
  - The user selects an entry (Enter or click).
  - The user presses **Escape**.
  - The user clicks outside the window.
- The window does not appear in the taskbar or the Alt+Tab switcher.

### 5.2 Layout

```
┌──────────────────────────────────┐
│ 🔍 Search...                     │  ← search bar, always visible at top
├──────────────────────────────────┤
│ ┌──────────────────────────────┐ │
│ │ 📌 [Pinned] Some pinned text │ │  ← pinned cards appear first
│ └──────────────────────────────┘ │
│ ┌──────────────────────────────┐ │
│ │ Lorem ipsum dolor sit amet…  │ │  ← regular card (text, truncated)
│ └──────────────────────────────┘ │
│ ┌──────────────────────────────┐ │
│ │ [image thumbnail]            │ │  ← image card
│ └──────────────────────────────┘ │
│              …                   │
│ ┌──────────────────────────────┐ │
│ │ <b>Bold</b> rich text…       │ │  ← rich text card (rendered preview)
│ └──────────────────────────────┘ │
└──────────────────────────────────┘
```

### 5.3 Card design

Each card shows:
- **Text entries**: first ~2 lines of text, truncated with `…` if longer. Tooltip on hover shows the full content.
- **Image entries**: a fixed-height thumbnail (e.g. 80 px tall). Tooltip shows full-resolution preview.
- **Rich text entries**: a plain-text preview (HTML stripped) with a small "HTML" badge.
- **Timestamp**: shown in the top-right corner of each card (e.g. "2 min ago").
- **Pin icon**: displayed if the entry is pinned.
- **Delete button**: a small ✕ button, visible on hover, to remove the entry.

### 5.4 Keyboard navigation

| Key | Action |
|---|---|
| ↑ / ↓ | Move highlight between cards |
| Enter | Select highlighted card (paste & close) |
| Delete | Delete highlighted card |
| Escape | Close window without selecting |
| Any printable character | Focus search bar and start filtering |

### 5.5 Search

- A search bar is always visible at the top of the window.
- Typing instantly filters the card list (case-insensitive substring match on text content; image entries are hidden when there is a non-empty query unless they have a user-assigned label).
- Clearing the search restores the full list.
- The search bar can be focused explicitly with **Ctrl+F** or by simply starting to type.

---

## 6. History Management

### 6.1 Capacity

- The maximum number of stored entries is **configurable in app settings**.
- When the limit is reached, the oldest non-pinned entry is evicted.
- Pinned entries are never evicted automatically.

### 6.2 Deduplication

- If an item identical to an existing entry is copied again, the existing entry is moved to the top of the list; no duplicate is created.

### 6.3 Pinning

- Any card can be pinned. Pinned cards appear at the top of the list (above unpinned entries) and are never evicted by the capacity limit.
- Pin/unpin is toggled via a pin icon in the card UI or a right-click context menu.

### 6.4 Deletion

- **Delete individual entry**: via the ✕ button on a card, the Delete key when a card is highlighted, or a right-click context menu.
- **Clear all history**: available in the right-click context menu and in app settings. Pinned entries are exempt unless the user explicitly confirms "clear including pinned".

---

## 7. Persistence (Optional)

By default, history is **in-memory only** and is lost when the daemon stops.

The user can opt into **disk persistence** in the app settings. When enabled:
- History is saved to `~/.local/share/copieur/history.db` (SQLite) after every change.
- History is reloaded from disk when the daemon starts.
- A prominent **warning** is displayed in the settings UI when the user enables this option:

> ⚠️ **Privacy warning**: Enabling persistence saves your clipboard contents to disk, including potentially sensitive data such as passwords, tokens, and personal information. Only enable this if you understand and accept the risk.

The user must explicitly acknowledge the warning before persistence is activated.

---

## 8. App Settings

A settings panel is accessible from within the clipboard window (e.g. a gear icon in the corner). Settings include:

| Setting | Default | Notes |
|---|---|---|
| Global shortcut | `Super+V` | Configurable key combo |
| Max history entries | 200 | Integer, min 10 |
| Persist history to disk | Off | Requires privacy warning acknowledgement |
| Autostart daemon on login | On | Writes/removes XDG autostart entry |
| Window position | Near cursor | Alternative: fixed position |
| UI theme | System | Light / Dark / System |

---

## 9. Non-Requirements

The following are explicitly **out of scope** for the initial version:

- Windows or macOS support.
- Browser extension or integration.
- Cloud sync of clipboard history.
- Clipboard content encryption at rest.
- Drag-and-drop reordering of pinned cards.
- Multi-monitor awareness beyond placing the window near the cursor.

---

## 10. Dependencies & Constraints

| Constraint | Detail |
|---|---|
| No Electron / WebView | The UI must be a native Rust binary |
| No mandatory GPU | Must render correctly in software-only environments (VMs, remote desktops) |
| X11 + Wayland | Both display protocols must be supported |
| Rust toolchain | Implementation language is Rust |
| External tools for paste | `xdotool` (X11) or `ydotool` (Wayland) must be available on the user's system for the auto-paste feature; graceful fallback if absent |
