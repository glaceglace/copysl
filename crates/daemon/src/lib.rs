pub mod autostart_manager;
pub mod clipboard_monitor;
pub mod focus_tracker;
pub mod history_store;
pub mod ipc_server;
pub mod paste_executor;
pub mod persistence;
pub mod tool_checker;

use std::sync::Arc;
use tokio::sync::{broadcast, Mutex, RwLock};

pub fn daemon_main() {
    // Initialize logger
    env_logger::init();

    // Build tokio runtime
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async_daemon_main());
}

async fn async_daemon_main() {
    // 1. Load config
    let cfg = config::load().unwrap_or_else(|e| {
        log::error!("Failed to load config: {e}");
        common::Config::default()
    });

    // Two Arc wrappers for Config:
    // - std::sync::RwLock for HistoryStore (sync operations)
    // - tokio::sync::RwLock for IpcServer (async operations)
    let std_config = Arc::new(std::sync::RwLock::new(cfg.clone()));
    let tokio_config = Arc::new(RwLock::new(cfg.clone()));

    // 2. Optionally open persistence and load history
    let (persistence, initial_entries) = if cfg.persist_history {
        match persistence::PersistenceHandle::open() {
            Ok((handle, _thread)) => {
                let entries = handle.load_all();
                (Some(handle), entries)
            }
            Err(e) => {
                log::error!("Failed to open persistence: {e}");
                (None, vec![])
            }
        }
    } else {
        (None, vec![])
    };

    // 3. Initialize HistoryStore
    let mut store = history_store::HistoryStore::new(std_config, persistence);
    if !initial_entries.is_empty() {
        store.load_initial(initial_entries);
    }
    let store = Arc::new(Mutex::new(store));

    // 4. Start FocusTracker
    // create_focus_tracker returns Box<dyn FocusTracker>; wrap in a newtype so it
    // can be stored as Arc<dyn FocusTracker> and passed to PasteExecutor.
    struct BoxedFocusTracker(Box<dyn focus_tracker::FocusTracker>);
    impl focus_tracker::FocusTracker for BoxedFocusTracker {
        fn start(&self) -> anyhow::Result<()> {
            self.0.start()
        }
        fn current_focus(&self) -> Option<focus_tracker::FocusHandle> {
            self.0.current_focus()
        }
    }

    let focus_tracker: Arc<dyn focus_tracker::FocusTracker> =
        Arc::new(BoxedFocusTracker(focus_tracker::create_focus_tracker()));
    if let Err(e) = focus_tracker.start() {
        log::error!("Failed to start focus tracker: {e}");
    }

    // 5. Bind IpcServer
    let socket_path = ipc_server::socket_path();
    let server = match ipc_server::IpcServer::bind(socket_path.clone()) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to bind IPC socket: {e}");
            return;
        }
    };
    log::info!("Copysl daemon started at {}", socket_path.display());

    // 6. Broadcast channel for new entries
    let (new_entry_tx, _) = broadcast::channel::<common::ClipboardEntry>(64);

    // 7. Start ClipboardMonitor (gated so we don't call it in tests)
    #[cfg(not(test))]
    let mut clip_rx = {
        let (std_tx, std_rx) = std::sync::mpsc::channel::<common::ClipboardEntry>();
        let (tok_tx, tok_rx) = tokio::sync::mpsc::channel::<common::ClipboardEntry>(64);
        // Bridge thread: converts std mpsc -> tokio mpsc
        std::thread::spawn(move || {
            for entry in std_rx {
                if tok_tx.blocking_send(entry).is_err() {
                    break;
                }
            }
        });
        clipboard_monitor::ClipboardMonitor::spawn(std_tx);
        tok_rx
    };

    // 8. Task to push new clipboard entries from monitor into the store
    #[cfg(not(test))]
    {
        let store_clone = Arc::clone(&store);
        let new_entry_tx_clone = new_entry_tx.clone();
        tokio::spawn(async move {
            while let Some(entry) = clip_rx.recv().await {
                let mut s = store_clone.lock().await;
                s.push(entry.clone());
                let _ = new_entry_tx_clone.send(entry);
            }
        });
    }

    // 9. Initialize PasteExecutor
    let paste_backend =
        paste_executor::select_backend(paste_executor::detect_display_server());
    let executor = Arc::new(paste_executor::PasteExecutor::new(
        paste_backend,
        Arc::clone(&focus_tracker) as Arc<dyn focus_tracker::FocusTracker>,
        Arc::clone(&tokio_config),
    ));

    // 10. Wire paste callback for IpcServer
    let executor_clone = Arc::clone(&executor);
    let paste_fn: Arc<dyn Fn(common::ClipboardEntry) -> Result<(), String> + Send + Sync> =
        Arc::new(move |entry| executor_clone.paste(&entry).map_err(|e| e.to_string()));

    // 11. Autostart
    let autostart = Arc::new(autostart_manager::AutostartManager::new());
    if cfg.autostart {
        if let Err(e) = autostart.enable() {
            log::error!("Failed to enable autostart: {e}");
        }
    }

    // 12. Run IpcServer (blocks until shutdown)
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let server_task = tokio::spawn({
        let store = Arc::clone(&store);
        let config = Arc::clone(&tokio_config);
        let autostart = Arc::clone(&autostart);
        let new_entry_tx = new_entry_tx.clone();
        async move {
            if let Err(e) = server.run(store, config, autostart, new_entry_tx, paste_fn, shutdown_tx).await {
                log::error!("IPC server error: {e}");
            }
        }
    });

    // 13. Wait for SIGTERM, Ctrl+C, or IPC shutdown request
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm =
            signal(SignalKind::terminate()).expect("Failed to register SIGTERM");
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log::info!("Received Ctrl+C, shutting down");
            }
            _ = sigterm.recv() => {
                log::info!("Received SIGTERM, shutting down");
            }
            _ = shutdown_rx.recv() => {
                log::info!("Received shutdown request via IPC, shutting down");
            }
        }
    }
    #[cfg(not(unix))]
    {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                log::info!("Received Ctrl+C, shutting down");
            }
            _ = shutdown_rx.recv() => {
                log::info!("Received shutdown request via IPC, shutting down");
            }
        }
    }

    // 14. Graceful shutdown
    server_task.abort();
    log::info!("Copysl daemon stopped");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use common::DaemonRequest;
    use tempfile::tempdir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn daemon_socket_reachable_returns_empty_history() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("smoke_test.sock");

        // Start just the IPC server (not the full daemon) with empty store
        let std_config = Arc::new(std::sync::RwLock::new(common::Config::default()));
        let store = Arc::new(Mutex::new(history_store::HistoryStore::new(
            std_config,
            None,
        )));
        let tok_config = Arc::new(RwLock::new(common::Config::default()));
        let autostart = Arc::new(autostart_manager::AutostartManager::with_base_dir(
            dir.path().to_path_buf(),
        ));
        let (tx, _) = broadcast::channel(16);
        let paste_fn: Arc<dyn Fn(common::ClipboardEntry) -> Result<(), String> + Send + Sync> =
            Arc::new(|_| Ok(()));

        let server = ipc_server::IpcServer::bind(sock_path.clone()).unwrap();
        let (shutdown_tx, _) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = server.run(store, tok_config, autostart, tx, paste_fn, shutdown_tx).await;
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let mut stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();

        // Send GetHistory
        let req_bytes = bincode::serialize(&DaemonRequest::GetHistory {
            offset: 0,
            limit: 10,
        })
        .unwrap();
        let len = req_bytes.len() as u32;
        stream.write_all(&len.to_le_bytes()).await.unwrap();
        stream.write_all(&req_bytes).await.unwrap();

        // Read response
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await.unwrap();
        let len = u32::from_le_bytes(len_buf) as usize;
        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await.unwrap();
        let response: common::DaemonResponse = bincode::deserialize(&buf).unwrap();

        assert!(matches!(
            response,
            common::DaemonResponse::History(v) if v.is_empty()
        ));
    }
}
