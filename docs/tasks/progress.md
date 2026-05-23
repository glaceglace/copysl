# Copieur — Global Progress

> Check off each module when all its tasks are complete. See the linked file for per-task detail.

---

## Foundation

- [x] [workspace-setup](workspace-setup.md) — Cargo workspace, root binary dispatch, CI skeleton
- [x] [common](common.md) — Shared types: `ClipboardEntry`, `IpcMessage`, `Config`, framing helpers
- [x] [config](config.md) — TOML config read/write with atomic save and defaults

---

## Daemon Modules

- [x] [daemon-main](daemon-main.md) — Entry point, subsystem wiring, signal handling
- [x] [daemon-clipboard-monitor](daemon-clipboard-monitor.md) — 200 ms clipboard polling, change detection
- [x] [daemon-history-store](daemon-history-store.md) — In-memory store: dedup, eviction, pin, paging
- [x] [daemon-focus-tracker](daemon-focus-tracker.md) — X11/Wayland active window tracking
- [x] [daemon-shortcut-listener](daemon-shortcut-listener.md) — Global shortcut capture (X11/Wayland), UI spawn/toggle
- [x] [daemon-ipc-server](daemon-ipc-server.md) — Unix socket server, request routing, push notifications
- [x] [daemon-paste-executor](daemon-paste-executor.md) — Clipboard restore + Ctrl+V injection (xdotool/ydotool/fallback)
- [x] [daemon-persistence](daemon-persistence.md) — Optional SQLite history storage
- [x] [daemon-autostart-manager](daemon-autostart-manager.md) — XDG autostart `.desktop` file management

---

## UI Modules

- [x] [ui-main](ui-main.md) — UI entry point, eframe initialization
- [x] [ui-window](ui-window.md) — Window options: borderless, always-on-top, near-cursor placement
- [x] [ui-ipc-client](ui-ipc-client.md) — Unix socket client, blocking send + non-blocking push reads
- [x] [ui-app](ui-app.md) — Top-level `eframe::App`: state, action handling, close triggers
- [x] [ui-components-search-bar](ui-components-search-bar.md) — Search input + filter logic
- [x] [ui-components-card](ui-components-card.md) — Single entry card: text/image/rich-text rendering, context menu
- [x] [ui-components-card-list](ui-components-card-list.md) — Scrollable card list, keyboard navigation, auto-scroll
- [x] [ui-components-settings-panel](ui-components-settings-panel.md) — Settings overlay: all config fields, privacy warning

---

## Summary

| Category | Done | Total |
|---|---|---|
| Foundation | 3 | 3 |
| Daemon | 9 | 9 |
| UI | 8 | 8 |
| **Total** | **20** | **20** |
