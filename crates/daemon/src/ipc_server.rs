use std::path::PathBuf;
use std::sync::Arc;

use libc;

use anyhow::Result;
use common::{ClipboardEntry, Config, DaemonRequest, DaemonResponse};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, Mutex, RwLock};

use crate::autostart_manager::AutostartManager;
use crate::history_store::HistoryStore;

// ---------------------------------------------------------------------------
// Socket path helpers
// ---------------------------------------------------------------------------

pub fn socket_path() -> PathBuf {
    if let Ok(runtime_dir) = std::env::var("XDG_RUNTIME_DIR") {
        PathBuf::from(runtime_dir).join("copysl.sock")
    } else {
        // SAFETY: getuid() is always safe to call.
        let uid = unsafe { libc::getuid() };
        PathBuf::from(format!("/tmp/copysl-{uid}.sock"))
    }
}

// ---------------------------------------------------------------------------
// Async framing helpers
// ---------------------------------------------------------------------------

#[allow(dead_code)]
async fn read_frame_async<T>(
    reader: &mut (impl AsyncReadExt + Unpin),
) -> std::io::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    bincode::deserialize(&buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

async fn write_frame_async<T>(
    writer: &mut (impl AsyncWriteExt + Unpin),
    msg: &T,
) -> std::io::Result<()>
where
    T: serde::Serialize,
{
    let bytes = bincode::serialize(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let len = bytes.len() as u32;
    writer.write_all(&len.to_le_bytes()).await?;
    writer.write_all(&bytes).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// IpcServer
// ---------------------------------------------------------------------------

pub struct IpcServer {
    listener: UnixListener,
    pub path: PathBuf,
}

impl IpcServer {
    /// Bind to a Unix socket at `path`, removing any stale socket first.
    pub fn bind(path: PathBuf) -> Result<Self> {
        let _ = std::fs::remove_file(&path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let listener = UnixListener::bind(&path)?;
        // Restrict to owner-only access so other local users cannot read
        // clipboard history or trigger paste via the socket.
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        Ok(IpcServer { listener, path })
    }

    /// Accept connections in a loop, spawning a task per connection.
    pub async fn run(
        self,
        store: Arc<Mutex<HistoryStore>>,
        config: Arc<RwLock<Config>>,
        autostart: Arc<AutostartManager>,
        new_entry_tx: broadcast::Sender<ClipboardEntry>,
        paste_fn: Arc<dyn Fn(ClipboardEntry) -> Result<(), String> + Send + Sync>,
        shutdown_tx: tokio::sync::mpsc::Sender<()>,
    ) -> Result<()> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            let store = Arc::clone(&store);
            let config = Arc::clone(&config);
            let autostart = Arc::clone(&autostart);
            let new_entry_rx = new_entry_tx.subscribe();
            let paste_fn = Arc::clone(&paste_fn);
            let shutdown_tx = shutdown_tx.clone();
            tokio::spawn(async move {
                if let Err(e) =
                    handle_connection(stream, store, config, autostart, new_entry_rx, paste_fn, shutdown_tx)
                        .await
                {
                    log::debug!("IPC connection closed: {e}");
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Connection handler
// ---------------------------------------------------------------------------

async fn handle_connection(
    stream: UnixStream,
    store: Arc<Mutex<HistoryStore>>,
    config: Arc<RwLock<Config>>,
    autostart: Arc<AutostartManager>,
    mut new_entry_rx: broadcast::Receiver<ClipboardEntry>,
    paste_fn: Arc<dyn Fn(ClipboardEntry) -> Result<(), String> + Send + Sync>,
    shutdown_tx: tokio::sync::mpsc::Sender<()>,
) -> Result<()> {
    let (mut reader, mut writer) = stream.into_split();

    loop {
        let mut len_buf = [0u8; 4];

        tokio::select! {
            // Read an incoming request from the client.
            result = reader.read_exact(&mut len_buf) => {
                result?;
                let len = u32::from_le_bytes(len_buf) as usize;
                if len > 64 * 1024 * 1024 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("frame too large: {len} bytes"),
                    ).into());
                }
                let mut msg_buf = vec![0u8; len];
                reader.read_exact(&mut msg_buf).await?;

                let request: DaemonRequest = bincode::deserialize(&msg_buf)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

                let response = handle_request(
                    request,
                    &store,
                    &config,
                    &autostart,
                    &paste_fn,
                    &shutdown_tx,
                )
                .await;

                write_frame_async(&mut writer, &response).await?;
            }

            // Push a new clipboard entry to the client.
            entry = new_entry_rx.recv() => {
                match entry {
                    Ok(e) => {
                        let push = DaemonResponse::NewEntry(e);
                        write_frame_async(&mut writer, &push).await?;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        log::warn!("IPC push channel lagged by {n} messages");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Request routing
// ---------------------------------------------------------------------------

async fn handle_request(
    request: DaemonRequest,
    store: &Arc<Mutex<HistoryStore>>,
    config: &Arc<RwLock<Config>>,
    autostart: &Arc<AutostartManager>,
    paste_fn: &Arc<dyn Fn(ClipboardEntry) -> Result<(), String> + Send + Sync>,
    shutdown_tx: &tokio::sync::mpsc::Sender<()>,
) -> DaemonResponse {
    match request {
        DaemonRequest::GetHistory { offset, limit } => {
            let s = store.lock().await;
            let entries = s.get_page(offset, limit);
            DaemonResponse::History(entries)
        }

        DaemonRequest::DeleteEntry { id } => {
            let mut s = store.lock().await;
            s.delete(id);
            DaemonResponse::Ok
        }

        DaemonRequest::PinEntry { id } => {
            let mut s = store.lock().await;
            s.pin(id);
            DaemonResponse::Ok
        }

        DaemonRequest::UnpinEntry { id } => {
            let mut s = store.lock().await;
            s.unpin(id);
            DaemonResponse::Ok
        }

        DaemonRequest::ClearHistory { include_pinned } => {
            let mut s = store.lock().await;
            s.clear(include_pinned);
            DaemonResponse::Ok
        }

        DaemonRequest::GetConfig => {
            let cfg = config.read().await;
            DaemonResponse::Config(cfg.clone())
        }

        DaemonRequest::UpdateConfig(new_config) => {
            let old_autostart = config.read().await.autostart;
            *config.write().await = new_config.clone();
            if let Err(e) = config::save(&new_config) {
                log::error!("Failed to save config: {e}");
            }
            if new_config.autostart != old_autostart {
                let result = if new_config.autostart {
                    autostart.enable()
                } else {
                    autostart.disable()
                };
                if let Err(e) = result {
                    log::error!("Failed to update autostart: {e}");
                }
            }
            DaemonResponse::Ok
        }

        DaemonRequest::PasteEntry { id } => {
            log::info!("Paste requested for entry {:?}", id);
            let entry = {
                let s = store.lock().await;
                s.get_by_id(id)
            };
            match entry {
                None => {
                    log::warn!("Paste entry {:?} not found in store", id);
                    DaemonResponse::Err(format!("Entry {:?} not found", id))
                }
                Some(entry) => {
                    log::info!("Paste entry {:?} found, spawning paste sequence", id);
                    let paste_fn = Arc::clone(paste_fn);
                    // Respond immediately so the UI stays unblocked and its render
                    // loop keeps running.  The paste task (Alt+Tab → delay → Ctrl+V)
                    // runs in the background; the UI uses its own 600 ms timer to
                    // close the window after the sequence is expected to finish.
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = paste_fn(entry) {
                            log::error!("Paste failed: {e}");
                        }
                    });
                    DaemonResponse::Ok
                }
            }
        }

        DaemonRequest::CheckTools => {
            crate::tool_checker::check_tools()
        }

        DaemonRequest::Shutdown => {
            let _ = shutdown_tx.send(()).await;
            DaemonResponse::Ok
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use common::{ContentPayload, EntryId};
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::{broadcast, Mutex, RwLock};

    fn make_store(config: Arc<std::sync::RwLock<Config>>) -> Arc<Mutex<HistoryStore>> {
        Arc::new(Mutex::new(HistoryStore::new(config, None)))
    }

    fn noop_paste() -> Arc<dyn Fn(ClipboardEntry) -> Result<(), String> + Send + Sync> {
        Arc::new(|_id| Ok(()))
    }

    async fn start_server(
        sock_path: PathBuf,
        store: Arc<Mutex<HistoryStore>>,
        config: Arc<RwLock<Config>>,
        autostart: Arc<AutostartManager>,
        tx: broadcast::Sender<ClipboardEntry>,
        paste_fn: Arc<dyn Fn(ClipboardEntry) -> Result<(), String> + Send + Sync>,
    ) {
        let server = IpcServer::bind(sock_path).unwrap();
        let (shutdown_tx, _) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = server.run(store, config, autostart, tx, paste_fn, shutdown_tx).await;
        });
        // Give the server time to start listening.
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    #[tokio::test]
    async fn get_history_returns_empty() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("test1.sock");

        let sync_config = Arc::new(std::sync::RwLock::new(Config::default()));
        let store = make_store(sync_config);
        let config = Arc::new(RwLock::new(Config::default()));
        let autostart = Arc::new(AutostartManager::with_base_dir(dir.path().to_path_buf()));
        let (tx, _) = broadcast::channel(16);

        start_server(
            sock_path.clone(),
            store,
            config,
            autostart,
            tx,
            noop_paste(),
        )
        .await;

        let mut stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        write_frame_async(
            &mut stream,
            &DaemonRequest::GetHistory {
                offset: 0,
                limit: 10,
            },
        )
        .await
        .unwrap();

        let response: DaemonResponse = read_frame_async(&mut stream).await.unwrap();
        match response {
            DaemonResponse::History(entries) => assert!(entries.is_empty()),
            other => panic!("Expected History response, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn update_config_round_trip() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("test2.sock");

        let sync_config = Arc::new(std::sync::RwLock::new(Config::default()));
        let store = make_store(sync_config);
        let config = Arc::new(RwLock::new(Config::default()));
        let autostart = Arc::new(AutostartManager::with_base_dir(dir.path().to_path_buf()));
        let (tx, _) = broadcast::channel(16);

        start_server(
            sock_path.clone(),
            store,
            Arc::clone(&config),
            autostart,
            tx,
            noop_paste(),
        )
        .await;

        let mut stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();

        let mut new_cfg = Config::default();
        new_cfg.max_entries = 42;
        // Disable autostart so the manager doesn't touch the filesystem.
        new_cfg.autostart = false;

        write_frame_async(&mut stream, &DaemonRequest::UpdateConfig(new_cfg.clone()))
            .await
            .unwrap();
        let response: DaemonResponse = read_frame_async(&mut stream).await.unwrap();
        assert!(matches!(response, DaemonResponse::Ok));

        // Verify config was updated via GetConfig.
        write_frame_async(&mut stream, &DaemonRequest::GetConfig)
            .await
            .unwrap();
        let response: DaemonResponse = read_frame_async(&mut stream).await.unwrap();
        match response {
            DaemonResponse::Config(c) => assert_eq!(c.max_entries, 42),
            other => panic!("Expected Config, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn push_notification_received() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("test3.sock");

        let sync_config = Arc::new(std::sync::RwLock::new(Config::default()));
        let store = make_store(sync_config);
        let config = Arc::new(RwLock::new(Config::default()));
        let autostart = Arc::new(AutostartManager::with_base_dir(dir.path().to_path_buf()));
        let (tx, _) = broadcast::channel(16);
        let tx_clone = tx.clone();

        start_server(
            sock_path.clone(),
            store,
            config,
            autostart,
            tx,
            noop_paste(),
        )
        .await;

        let mut stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();

        // Send a request so the connection is open and the push subscription is active.
        write_frame_async(
            &mut stream,
            &DaemonRequest::GetHistory {
                offset: 0,
                limit: 10,
            },
        )
        .await
        .unwrap();
        let _: DaemonResponse = read_frame_async(&mut stream).await.unwrap();

        // Broadcast a new entry.
        let entry = ClipboardEntry {
            id: EntryId(99),
            payload: ContentPayload::PlainText("pushed".to_string()),
            captured_at: std::time::SystemTime::now(),
            pinned: false,
        };
        let _ = tx_clone.send(entry.clone());

        // The connection task should forward it as a push notification.
        let push: DaemonResponse = read_frame_async(&mut stream).await.unwrap();
        match push {
            DaemonResponse::NewEntry(e) => assert_eq!(e.id, entry.id),
            other => panic!("Expected NewEntry push, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn paste_entry_calls_paste_fn() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("test4.sock");

        let sync_config = Arc::new(std::sync::RwLock::new(Config::default()));
        let store = make_store(Arc::clone(&sync_config));
        // Pre-populate the store with an entry so the handler can find it.
        // HistoryStore::push reassigns IDs starting from 0, so the first entry gets EntryId(0).
        let paste_id = {
            let entry = ClipboardEntry {
                id: EntryId(0), // will be overwritten by push
                payload: ContentPayload::PlainText("hello".to_string()),
                captured_at: std::time::SystemTime::now(),
                pinned: false,
            };
            let mut s = store.lock().await;
            s.push(entry);
            s.get_page(0, 1)[0].id
        };
        let config = Arc::new(RwLock::new(Config::default()));
        let autostart = Arc::new(AutostartManager::with_base_dir(dir.path().to_path_buf()));
        let (tx, _) = broadcast::channel(16);

        let called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let called_clone = Arc::clone(&called);
        let paste_fn: Arc<dyn Fn(ClipboardEntry) -> Result<(), String> + Send + Sync> =
            Arc::new(move |_entry| {
                called_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            });

        start_server(
            sock_path.clone(),
            store,
            config,
            autostart,
            tx,
            paste_fn,
        )
        .await;

        let mut stream = tokio::net::UnixStream::connect(&sock_path).await.unwrap();
        write_frame_async(&mut stream, &DaemonRequest::PasteEntry { id: paste_id })
            .await
            .unwrap();
        let response: DaemonResponse = read_frame_async(&mut stream).await.unwrap();
        assert!(matches!(response, DaemonResponse::Ok));
        // Paste runs in a background thread; give it a moment to complete.
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        assert!(called.load(std::sync::atomic::Ordering::SeqCst));
    }
}
