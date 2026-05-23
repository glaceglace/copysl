//! Tool availability checker for the paste flow.
//!
//! ## The paste flow requires three things
//!
//! 1. **Clipboard setter** — writes content to the system clipboard so Ctrl+V
//!    has something to paste.  In order of preference:
//!    - `wl-copy` (wl-clipboard package) — Wayland; spawns a persistent background
//!      daemon that serves the clipboard until another app takes ownership.
//!    - `xclip` — X11 / XWayland; same persistent-daemon semantics.
//!    - arboard (built-in Rust crate) — always available but its background
//!      thread dies when the `Clipboard` handle is dropped, so we must hold it
//!      alive during the paste sequence (less reliable).
//!
//! 2. **Key injector** — simulates Ctrl+V so the target application pastes the
//!    clipboard content.  In order of preference:
//!    - `ydotool` — Wayland-native; needs the `ydotoold` daemon running.
//!    - `wtype` — Wayland-native; works on wlroots-based compositors.
//!    - `xdotool` — X11 / XWayland only.
//!    - **xcb XTest** (built-in, via `xcb` crate + `xtest` feature) — used when
//!      `DISPLAY` env var is set (XWayland or native X11).  Reaches X11 and
//!      XWayland apps but NOT native Wayland apps.
//!    - NotificationFallback — silent fallback; Ctrl+V is NOT injected.
//!
//! 3. **Focus return** — after the Copieur window closes the window manager
//!    automatically returns focus to the previously active window.  No external
//!    tool is needed for this.
//!
//! ## Critical vs. recommended
//!
//! | Situation | Severity |
//! |-----------|----------|
//! | Pure Wayland (no DISPLAY) + no ydotool + no wtype | **Critical** — paste silently does nothing |
//! | ydotool installed but ydotoold daemon not running | **Critical** — ydotool will error at paste time |
//! | Wayland + XWayland + no Wayland inject tool | **Recommended** — X11/XWayland apps work; native Wayland apps do not |
//! | No persistent clipboard daemon (wl-copy / xclip) | **Recommended** — arboard may lose clipboard content |
//!
//! ## Adding a new required tool
//!
//! 1. Add a `has_<tool>: bool` parameter to `check_tools_with()` so tests can
//!    control it without touching the filesystem.
//! 2. Add the business logic (critical / recommended) inside `check_tools_with()`,
//!    following the pattern of the existing checks with a comment explaining why.
//! 3. Update `check_tools()` to detect the new tool via `tool_available("<name>")`
//!    or a dedicated helper (e.g. `ydotoold_running()` for process/socket checks).
//! 4. Add unit tests covering presence and absence of the new tool.

use common::{DaemonResponse, ToolRequirement};

// ---------------------------------------------------------------------------
// Package-manager detection
// ---------------------------------------------------------------------------

/// Returns the install command prefix for the first package manager found.
/// Used to build human-readable `install_hint` strings.
///
/// Add new package managers here as more Linux distributions need support.
fn install_prefix() -> &'static str {
    for (manager, prefix) in [
        ("dnf", "dnf install"),
        ("apt", "apt install"),
        ("pacman", "pacman -S"),
        ("zypper", "zypper install"),
    ] {
        if tool_available(manager) {
            return prefix;
        }
    }
    "install"
}

// ---------------------------------------------------------------------------
// Display server helpers
// ---------------------------------------------------------------------------

/// Human-readable description of the current display server setup.
pub fn detect_display_server_name() -> String {
    let has_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
    let has_x11 = std::env::var("DISPLAY").is_ok();
    match (has_wayland, has_x11) {
        (true, true) => "Wayland + XWayland".to_string(),
        (true, false) => "Wayland".to_string(),
        (false, true) => "X11".to_string(),
        (false, false) => "Unknown".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tool detection
// ---------------------------------------------------------------------------

/// Returns `true` if `name` resolves to an executable via `which`.
///
/// All per-tool binary checks go through here so they are easy to mock in
/// tests (by calling `check_tools_with()` directly with bool parameters).
pub fn tool_available(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Returns `true` if the `ydotoold` daemon is currently running.
///
/// Checks the socket file that `ydotoold` creates at
/// `$XDG_RUNTIME_DIR/.ydotool_socket`.  A missing socket means the daemon
/// is not running even if the `ydotool` binary is installed.
///
/// We use the socket file rather than `pgrep` so the check works without
/// `procps` and reflects the actual usable state — a running `ydotoold` is
/// only useful once its socket exists.
pub fn ydotoold_running() -> bool {
    let socket_path = std::env::var("XDG_RUNTIME_DIR")
        .map(|dir| std::path::PathBuf::from(dir).join(".ydotool_socket"))
        .unwrap_or_else(|_| {
            // Fall back to /run/user/<uid>/.ydotool_socket using /proc/self/status
            let uid = std::fs::read_to_string("/proc/self/status")
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find(|l| l.starts_with("Uid:"))
                        .and_then(|l| l.split_whitespace().nth(1))
                        .and_then(|uid| uid.parse::<u32>().ok())
                })
                .unwrap_or(1000);
            std::path::PathBuf::from(format!("/run/user/{}/.ydotool_socket", uid))
        });
    socket_path.exists()
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run tool availability checks against the real environment.
///
/// Reads `WAYLAND_DISPLAY` and `DISPLAY` env vars, calls `which` for each
/// tool, then delegates to `check_tools_with()`.  Returns a
/// `DaemonResponse::ToolsStatus` ready to send over IPC.
pub fn check_tools() -> DaemonResponse {
    let has_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
    let has_display = std::env::var("DISPLAY").is_ok();

    check_tools_with(
        has_wayland,
        has_display,
        tool_available("wl-copy"),
        tool_available("xclip"),
        tool_available("xdotool"),
        tool_available("ydotool"),
        ydotoold_running(),
        tool_available("wtype"),
    )
}

// ---------------------------------------------------------------------------
// Parameterized inner implementation (for unit testing)
// ---------------------------------------------------------------------------

/// Core tool-check logic — all external state is injected so tests run
/// without touching the filesystem or environment.
///
/// | parameter | meaning |
/// |-----------|---------|
/// | `has_wayland` | `WAYLAND_DISPLAY` is set |
/// | `has_display` | `DISPLAY` is set (X11 or XWayland is running) |
/// | `has_wl_copy` | `wl-copy` binary present |
/// | `has_xclip` | `xclip` binary present |
/// | `has_xdotool` | `xdotool` binary present |
/// | `has_ydotool` | `ydotool` binary present |
/// | `ydotoold_daemon` | `ydotoold` socket exists (daemon is running) |
/// | `has_wtype` | `wtype` binary present |
pub(crate) fn check_tools_with(
    has_wayland: bool,
    has_display: bool,
    has_wl_copy: bool,
    has_xclip: bool,
    has_xdotool: bool,
    has_ydotool: bool,
    ydotoold_daemon: bool,
    has_wtype: bool,
) -> DaemonResponse {
    // ── Display server label ───────────────────────────────────────────────
    let display_server = match (has_wayland, has_display) {
        (true, true) => "Wayland + XWayland",
        (true, false) => "Wayland",
        (false, true) => "X11",
        (false, false) => "Unknown",
    }
    .to_string();

    // ── Inject backend — mirrors paste_executor::select_backend() logic ────
    // Keep this in sync if that logic changes.
    let inject_backend = if !has_wayland {
        // X11 path
        if has_xdotool {
            "xdotool (X11)"
        } else {
            "xcb XTest (X11 built-in)"
        }
    } else {
        // Wayland path
        if has_ydotool {
            "ydotool (Wayland)"
        } else if has_wtype {
            "wtype (Wayland)"
        } else if has_xdotool {
            "xdotool via XWayland"
        } else if has_display {
            "xcb XTest via XWayland (built-in)"
        } else {
            "none (paste will fail)"
        }
    }
    .to_string();

    // ── Clipboard backend ──────────────────────────────────────────────────
    let clipboard_backend = if has_wayland && has_wl_copy {
        "wl-copy"
    } else if has_xclip {
        "xclip"
    } else {
        "arboard (built-in, less reliable)"
    }
    .to_string();

    let pm = install_prefix();
    let mut missing_critical: Vec<ToolRequirement> = vec![];
    let mut missing_recommended: Vec<ToolRequirement> = vec![];

    // ── CRITICAL: key injection completely broken ──────────────────────────
    // On pure Wayland (no DISPLAY) without ydotool or wtype, xcb XTest cannot
    // reach the display server, so Ctrl+V injection silently does nothing.
    // xdotool requires DISPLAY and does not work on native Wayland either.
    let injection_broken =
        has_wayland && !has_display && !has_ydotool && !has_wtype;
    if injection_broken {
        missing_critical.push(ToolRequirement {
            tool_name: "ydotool".to_string(),
            purpose: "Inject Ctrl+V into the target window on Wayland".to_string(),
            install_hint: format!("{pm} ydotool"),
        });
        missing_critical.push(ToolRequirement {
            tool_name: "wtype".to_string(),
            purpose: "Inject Ctrl+V on wlroots-based Wayland compositors (alternative to ydotool)"
                .to_string(),
            install_hint: format!("{pm} wtype"),
        });
    }

    // ── CRITICAL: ydotool installed but ydotoold daemon not running ────────
    // ydotool is a client that talks to the ydotoold daemon via a Unix socket.
    // If the daemon is not running, every ydotool call fails with
    // "failed to connect socket ... No such file or directory".
    // We report this as critical because ydotool was selected as the injection
    // backend, so paste WILL fail without ydotoold.
    //
    // NOTE: ydotoold.service does NOT ship with the ydotool package — the user
    // must create the unit file manually before enabling it.  The install_hint
    // below contains the complete sequence of commands to do so.
    if has_ydotool && !ydotoold_daemon {
        missing_critical.push(ToolRequirement {
            tool_name: "ydotoold".to_string(),
            purpose: "ydotool is installed but its daemon is not running — paste injection will fail".to_string(),
            install_hint: "\
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
systemctl --user enable --now ydotoold".to_string(),
        });
    }

    // ── RECOMMENDED: Wayland + XWayland, injection only reaches XWayland apps
    // xcb XTest built-in works but only for X11/XWayland apps.  Native Wayland
    // apps (e.g. Firefox, GNOME apps using Wayland protocol) are unreachable.
    // ydotool or wtype would cover all apps.
    let xtest_only_on_wayland =
        has_wayland && has_display && !has_ydotool && !has_wtype && !has_xdotool;
    if xtest_only_on_wayland {
        missing_recommended.push(ToolRequirement {
            tool_name: "ydotool".to_string(),
            purpose:
                "Inject Ctrl+V into native Wayland apps (currently only XWayland apps work)"
                    .to_string(),
            install_hint: format!("{pm} ydotool"),
        });
        missing_recommended.push(ToolRequirement {
            tool_name: "wtype".to_string(),
            purpose: "Inject Ctrl+V on wlroots-based compositors (alternative to ydotool)"
                .to_string(),
            install_hint: format!("{pm} wtype"),
        });
    }

    // ── RECOMMENDED: persistent clipboard daemon ───────────────────────────
    // arboard's clipboard is served by a background thread that dies when the
    // Clipboard handle is dropped.  A persistent daemon (wl-copy / xclip) keeps
    // the clipboard alive after the Copieur window closes, making paste more
    // reliable.
    let has_persistent_clipboard = (has_wayland && has_wl_copy) || has_xclip;
    if !has_persistent_clipboard {
        if has_wayland && !has_wl_copy {
            missing_recommended.push(ToolRequirement {
                tool_name: "wl-copy".to_string(),
                purpose:
                    "Persist clipboard content after Copieur closes (wl-clipboard package)"
                        .to_string(),
                install_hint: format!("{pm} wl-clipboard"),
            });
        }
        if !has_xclip {
            missing_recommended.push(ToolRequirement {
                tool_name: "xclip".to_string(),
                purpose: "Persist clipboard content after Copieur closes on X11".to_string(),
                install_hint: format!("{pm} xclip"),
            });
        }
    }

    DaemonResponse::ToolsStatus {
        display_server,
        inject_backend,
        clipboard_backend,
        missing_critical,
        missing_recommended,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use common::DaemonResponse;

    fn unwrap_status(
        resp: DaemonResponse,
    ) -> (
        String,
        String,
        String,
        Vec<common::ToolRequirement>,
        Vec<common::ToolRequirement>,
    ) {
        match resp {
            DaemonResponse::ToolsStatus {
                display_server,
                inject_backend,
                clipboard_backend,
                missing_critical,
                missing_recommended,
            } => (
                display_server,
                inject_backend,
                clipboard_backend,
                missing_critical,
                missing_recommended,
            ),
            _ => panic!("Expected ToolsStatus"),
        }
    }

    // Shorthand: (has_wayland, has_display, has_wl_copy, has_xclip,
    //             has_xdotool, has_ydotool, ydotoold_daemon, has_wtype)

    // -- Pure Wayland, no tools at all (worst case) -------------------------

    #[test]
    fn pure_wayland_no_tools_critical_injection() {
        let resp = check_tools_with(
            true, false,                      // Wayland, no DISPLAY
            false, false, false, false, false, false, // no tools
        );
        let (ds, inject_backend, _, critical, _) = unwrap_status(resp);
        assert_eq!(ds, "Wayland");
        assert_eq!(inject_backend, "none (paste will fail)");
        assert!(!critical.is_empty(), "should flag critical: no inject tool");
        assert!(critical.iter().any(|t| t.tool_name == "ydotool"));
        assert!(critical.iter().any(|t| t.tool_name == "wtype"));
    }

    // -- Pure Wayland with wtype present ------------------------------------

    #[test]
    fn pure_wayland_wtype_no_critical() {
        let resp = check_tools_with(
            true, false,                       // Wayland, no DISPLAY
            false, false, false, false, false, true, // wtype present
        );
        let (_, inject_backend, _, critical, _) = unwrap_status(resp);
        assert_eq!(inject_backend, "wtype (Wayland)");
        assert!(critical.is_empty());
    }

    // -- Pure Wayland with ydotool + ydotoold running -----------------------

    #[test]
    fn pure_wayland_ydotool_and_daemon_no_critical() {
        let resp = check_tools_with(
            true, false,                       // Wayland, no DISPLAY
            false, false, false, true, true, false, // ydotool + daemon running
        );
        let (_, inject_backend, _, critical, _) = unwrap_status(resp);
        assert_eq!(inject_backend, "ydotool (Wayland)");
        assert!(critical.is_empty());
    }

    // -- ydotool installed but ydotoold not running -------------------------

    #[test]
    fn ydotool_installed_but_daemon_not_running_is_critical() {
        let resp = check_tools_with(
            true, true,                        // Wayland + XWayland
            true, false, false, true, false, false, // ydotool present, daemon NOT running
        );
        let (_, inject_backend, _, critical, _) = unwrap_status(resp);
        assert_eq!(inject_backend, "ydotool (Wayland)");
        assert!(
            critical.iter().any(|t| t.tool_name == "ydotoold"),
            "ydotoold should be flagged critical when ydotool is installed but daemon is down"
        );
    }

    // -- ydotoold hint contains the full service-creation sequence ---------

    #[test]
    fn ydotoold_install_hint_contains_full_service_setup() {
        let resp = check_tools_with(
            true, true,
            true, false, false, true, false, false, // ydotool present, daemon NOT running
        );
        let (_, _, _, critical, _) = unwrap_status(resp);
        let req = critical.iter().find(|t| t.tool_name == "ydotoold").unwrap();
        let hint = &req.install_hint;
        assert!(hint.contains("mkdir"), "hint should create the systemd user dir");
        assert!(hint.contains("ydotoold.service"), "hint should create the service file");
        assert!(hint.contains("[Unit]"), "hint should include the Unit section");
        assert!(hint.contains("ExecStart=ydotoold"), "hint should set ExecStart");
        assert!(hint.contains("daemon-reload"), "hint should reload systemd after creating the file");
        assert!(hint.contains("enable --now ydotoold"), "hint should enable and start the service");
    }

    // -- ydotool not installed → no ydotoold warning -----------------------

    #[test]
    fn no_ydotool_means_no_ydotoold_warning() {
        let resp = check_tools_with(
            true, true,
            true, false, false, false, false, false, // ydotool absent
        );
        let (_, _, _, critical, _) = unwrap_status(resp);
        assert!(
            !critical.iter().any(|t| t.tool_name == "ydotoold"),
            "ydotoold warning should only appear when ydotool is installed"
        );
    }

    // -- Wayland + XWayland, no inject tools (user's actual setup) ----------

    #[test]
    fn wayland_xwayland_no_inject_tools_recommended_only() {
        let resp = check_tools_with(
            true, true,                         // Wayland + XWayland
            true, false, false, false, false, false, // wl-copy present, inject tools absent
        );
        let (ds, inject_backend, clipboard_backend, critical, recommended) =
            unwrap_status(resp);
        assert_eq!(ds, "Wayland + XWayland");
        assert!(inject_backend.contains("XWayland"), "got: {inject_backend}");
        assert!(clipboard_backend.contains("wl-copy"), "got: {clipboard_backend}");
        assert!(
            critical.is_empty(),
            "xcb XTest handles X11/XWayland so no critical tools needed"
        );
        assert!(
            recommended.iter().any(|t| t.tool_name == "ydotool"),
            "ydotool recommended for native Wayland apps"
        );
    }

    // -- Wayland + XWayland with ydotool + daemon — no recommendations -----

    #[test]
    fn wayland_xwayland_ydotool_with_daemon_no_inject_recommendation() {
        let resp = check_tools_with(
            true, true,                         // Wayland + XWayland
            true, false, false, true, true, false, // wl-copy + ydotool + daemon running
        );
        let (_, inject_backend, _, critical, recommended) = unwrap_status(resp);
        assert_eq!(inject_backend, "ydotool (Wayland)");
        assert!(critical.is_empty());
        assert!(
            !recommended.iter().any(|t| t.tool_name == "ydotool"),
            "ydotool is present so should not be recommended"
        );
    }

    // -- Pure X11, no tools -------------------------------------------------

    #[test]
    fn x11_no_tools_no_critical() {
        let resp = check_tools_with(
            false, true,                        // X11 only
            false, false, false, false, false, false, // no tools
        );
        let (ds, inject_backend, _, critical, recommended) = unwrap_status(resp);
        assert_eq!(ds, "X11");
        assert!(inject_backend.contains("XTest"), "got: {inject_backend}");
        assert!(critical.is_empty(), "xcb XTest always works on X11");
        assert!(
            recommended.iter().any(|t| t.tool_name == "xclip"),
            "xclip recommended for reliable clipboard"
        );
    }

    // -- X11 fully equipped ------------------------------------------------

    #[test]
    fn x11_fully_equipped_no_warnings() {
        let resp = check_tools_with(
            false, true,                        // X11 only
            false, true, true, false, false, false, // xclip + xdotool
        );
        let (_, inject_backend, clipboard_backend, critical, recommended) =
            unwrap_status(resp);
        assert_eq!(inject_backend, "xdotool (X11)");
        assert!(clipboard_backend.contains("xclip"));
        assert!(critical.is_empty());
        assert!(recommended.is_empty());
    }

    // -- Unknown display server (no WAYLAND_DISPLAY or DISPLAY) ------------

    #[test]
    fn unknown_display_no_wayland_no_critical() {
        let resp = check_tools_with(
            false, false,
            false, false, false, false, false, false,
        );
        let (ds, _, _, critical, _) = unwrap_status(resp);
        assert_eq!(ds, "Unknown");
        // injection_broken only triggers when has_wayland is true, so no critical here
        assert!(critical.is_empty());
    }

    // -- Install hints contain the tool name --------------------------------

    #[test]
    fn critical_tool_install_hint_mentions_tool_or_package() {
        let resp = check_tools_with(
            true, false,                        // pure Wayland, injection broken
            false, false, false, false, false, false,
        );
        let (_, _, _, critical, _) = unwrap_status(resp);
        assert!(!critical.is_empty());
        for req in &critical {
            let hint_lower = req.install_hint.to_lowercase();
            let name_lower = req.tool_name.to_lowercase();
            assert!(
                hint_lower.contains(&name_lower) || hint_lower.contains("wl-clipboard"),
                "install_hint '{}' should mention tool '{}'",
                req.install_hint,
                req.tool_name
            );
        }
    }

    // -- Recommended clipboard hint (Wayland, no wl-copy) ------------------

    #[test]
    fn wayland_xwayland_no_wl_copy_recommends_wl_copy() {
        let resp = check_tools_with(
            true, true,
            false, false, false, false, false, false, // no clipboard tools
        );
        let (_, _, clipboard_backend, _, recommended) = unwrap_status(resp);
        assert!(clipboard_backend.contains("arboard"));
        assert!(
            recommended.iter().any(|t| t.tool_name == "wl-copy"),
            "wl-copy should be recommended on Wayland"
        );
    }

    // -- check_tools() smoke test against real environment -----------------
    // Only verifies it returns ToolsStatus without panicking; does not assert
    // specific tool presence since the CI/dev environment varies.

    #[test]
    fn check_tools_real_env_returns_status() {
        assert!(matches!(check_tools(), DaemonResponse::ToolsStatus { .. }));
    }
}
