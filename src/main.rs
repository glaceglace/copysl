pub fn parse_mode(args: &[String]) -> RunMode {
    if args.iter().any(|a| a == "--daemon") {
        RunMode::Daemon
    } else if args.iter().any(|a| a == "--ui") {
        RunMode::Ui
    } else {
        RunMode::DaemonWithUi
    }
}

/// Returns `true` when `--debug` is present in `args`.
///
/// Debug mode sets `RUST_LOG=debug` before the daemon starts so that
/// internal paste decisions are printed to stderr, e.g.:
///   "Paste: X11 target known — injecting directly (no delay)"
///   "Paste: waiting 300ms for window close and WM focus transfer"
pub fn has_debug_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "--debug")
}

#[derive(Debug, PartialEq)]
pub enum RunMode {
    /// --daemon: run daemon silently, no UI (e.g. for systemd/autostart)
    Daemon,
    /// --ui: show UI window only, daemon must already be running
    Ui,
    /// no args: start daemon + show UI window on startup
    DaemonWithUi,
}

fn daemon_running(socket_path: &std::path::Path) -> bool {
    std::os::unix::net::UnixStream::connect(socket_path).is_ok()
}

fn wait_for_daemon(socket_path: &std::path::Path) {
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if daemon_running(socket_path) {
            break;
        }
    }
}

/// Detach from the terminal using the classic Unix double-fork technique.
///
/// After this call the process is a background daemon: the shell gets its
/// prompt back, the process is immune to SIGHUP when the terminal closes,
/// and stdin/stdout/stderr are redirected to /dev/null.
///
/// Not called when `--debug` is active so that log output stays visible.
#[cfg(unix)]
fn daemonize() {
    unsafe {
        // First fork — let the parent exit so the shell regains its prompt.
        let pid = libc::fork();
        assert!(pid >= 0, "daemonize: first fork failed");
        if pid > 0 {
            libc::_exit(0);
        }
        // Become a new session leader, detaching from the controlling terminal.
        libc::setsid();
        // Second fork — prevents the daemon from ever reacquiring a terminal.
        let pid = libc::fork();
        assert!(pid >= 0, "daemonize: second fork failed");
        if pid > 0 {
            libc::_exit(0);
        }
        // Redirect stdin / stdout / stderr to /dev/null.
        let null = libc::open(
            b"/dev/null\0".as_ptr() as *const libc::c_char,
            libc::O_RDWR,
        );
        if null >= 0 {
            libc::dup2(null, libc::STDIN_FILENO);
            libc::dup2(null, libc::STDOUT_FILENO);
            libc::dup2(null, libc::STDERR_FILENO);
            if null > libc::STDERR_FILENO {
                libc::close(null);
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if has_debug_flag(&args) {
        // Enable debug logging unless the user already configured RUST_LOG.
        if std::env::var("RUST_LOG").is_err() {
            std::env::set_var("RUST_LOG", "debug");
        }
    }
    match parse_mode(&args) {
        RunMode::Daemon => {
            // Detach from the terminal unless --debug keeps it in the foreground.
            #[cfg(unix)]
            if !has_debug_flag(&args) {
                daemonize();
            }
            daemon::daemon_main();
        }
        RunMode::Ui => {
            let socket_path = daemon::ipc_server::socket_path();
            if !daemon_running(&socket_path) {
                std::thread::spawn(daemon::daemon_main);
                wait_for_daemon(&socket_path);
            }
            ui::ui_main(&socket_path.to_string_lossy());
        }
        RunMode::DaemonWithUi => {
            let socket_path = daemon::ipc_server::socket_path();
            let daemon_thread = if !daemon_running(&socket_path) {
                let t = std::thread::spawn(daemon::daemon_main);
                wait_for_daemon(&socket_path);
                Some(t)
            } else {
                None
            };

            ui::ui_main(&socket_path.to_string_lossy());

            // If we own the daemon thread, block here so the daemon keeps running
            // after the UI window is closed.
            if let Some(t) = daemon_thread {
                let _ = t.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_selects_daemon_with_ui() {
        assert_eq!(parse_mode(&[]), RunMode::DaemonWithUi);
    }

    #[test]
    fn daemon_flag_selects_daemon() {
        assert_eq!(
            parse_mode(&["copysl".to_string(), "--daemon".to_string()]),
            RunMode::Daemon
        );
    }

    #[test]
    fn ui_flag_selects_ui() {
        assert_eq!(
            parse_mode(&["copysl".to_string(), "--ui".to_string()]),
            RunMode::Ui
        );
    }

    #[test]
    fn debug_flag_detected() {
        assert!(has_debug_flag(&["copysl".to_string(), "--debug".to_string()]));
    }

    #[test]
    fn debug_flag_absent_without_arg() {
        assert!(!has_debug_flag(&["copysl".to_string()]));
    }

    #[test]
    fn debug_flag_combines_with_mode_flags() {
        let args = vec![
            "copysl".to_string(),
            "--daemon".to_string(),
            "--debug".to_string(),
        ];
        assert_eq!(parse_mode(&args), RunMode::Daemon);
        assert!(has_debug_flag(&args));
    }
}
