use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

// ---------------------------------------------------------------------------
// EntryId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntryId(pub u64);

// ---------------------------------------------------------------------------
// ImageMime
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageMime {
    Png,
    Jpeg,
}

// ---------------------------------------------------------------------------
// ContentPayload
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ContentPayload {
    PlainText(String),
    RichText {
        html: String,
        plain_preview: String,
    },
    Image {
        data: Vec<u8>,
        mime: ImageMime,
    },
}

// ---------------------------------------------------------------------------
// ClipboardEntry
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: EntryId,
    pub payload: ContentPayload,
    pub captured_at: std::time::SystemTime,
    pub pinned: bool,
}

impl ClipboardEntry {
    /// Returns a SHA-256 hex digest of the raw payload bytes.
    pub fn content_hash(&self) -> String {
        let mut hasher = Sha256::new();
        match &self.payload {
            ContentPayload::PlainText(text) => hasher.update(text.as_bytes()),
            ContentPayload::RichText { html, .. } => hasher.update(html.as_bytes()),
            ContentPayload::Image { data, .. } => hasher.update(data),
        }
        format!("{:x}", hasher.finalize())
    }
}

// ---------------------------------------------------------------------------
// WindowPos and Theme
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowPos {
    NearCursor,
    Fixed(i32, i32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    System,
    Light,
    Dark,
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

fn default_paste_delay_ms() -> u32 { 150 }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub max_entries: usize,
    pub persist_history: bool,
    pub autostart: bool,
    pub window_position: WindowPos,
    pub theme: Theme,
    /// Wayland-only: milliseconds the daemon waits after the Copysl window
    /// closes before injecting Ctrl+V.  The WM must return focus to the
    /// previous app in this window.  On X11/XWayland this field is ignored
    /// because the target window is addressed directly via XSetInputFocus.
    ///
    /// Too short → keystrokes arrive before focus returns, wrong window pastes.
    /// Too long  → noticeable lag between clicking and text appearing.
    /// Range: 10–3000 ms.  Default: 150 ms.
    #[serde(default = "default_paste_delay_ms")]
    pub paste_delay_ms: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            max_entries: 200,
            persist_history: false,
            autostart: true,
            window_position: WindowPos::NearCursor,
            theme: Theme::System,
            paste_delay_ms: default_paste_delay_ms(),
        }
    }
}

// ---------------------------------------------------------------------------
// ToolRequirement — describes a missing external tool
// ---------------------------------------------------------------------------

/// One entry in the tool-check report: a tool that is absent but needed.
///
/// Displayed in the startup warning popup when the daemon detects that paste
/// will be degraded or broken.  The list is produced by `tool_checker::check_tools`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolRequirement {
    /// Binary / package name shown to the user (e.g. `"ydotool"`).
    pub tool_name: String,
    /// One-line human description of what the tool is used for.
    pub purpose: String,
    /// Ready-to-run shell command the user can copy-paste to install it.
    pub install_hint: String,
}

// ---------------------------------------------------------------------------
// DaemonRequest
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DaemonRequest {
    GetHistory { offset: usize, limit: usize },
    PasteEntry { id: EntryId },
    DeleteEntry { id: EntryId },
    PinEntry { id: EntryId },
    UnpinEntry { id: EntryId },
    ClearHistory { include_pinned: bool },
    GetConfig,
    UpdateConfig(Config),
    /// Ask the daemon to run a tool-availability check and return the result.
    /// The UI calls this once at startup to decide whether to show a warning.
    CheckTools,
    Shutdown,
}

// ---------------------------------------------------------------------------
// DaemonResponse
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DaemonResponse {
    History(Vec<ClipboardEntry>),
    Config(Config),
    Ok,
    Err(String),
    NewEntry(ClipboardEntry),
    /// Result of a CheckTools request.
    ToolsStatus {
        /// Human-readable display server description, e.g. "Wayland + XWayland".
        display_server: String,
        /// Name of the injection backend that was selected at daemon startup.
        inject_backend: String,
        /// Name of the clipboard backend that will be used.
        clipboard_backend: String,
        /// Tools that MUST be installed for paste to work at all.
        missing_critical: Vec<ToolRequirement>,
        /// Tools that would improve paste reliability but are not strictly required.
        missing_recommended: Vec<ToolRequirement>,
    },
}

// ---------------------------------------------------------------------------
// IPC framing helpers
// ---------------------------------------------------------------------------

/// Serialize `msg` with bincode and write it with a 4-byte little-endian length prefix.
pub fn write_frame<W: std::io::Write, T: serde::Serialize>(
    writer: &mut W,
    msg: &T,
) -> std::io::Result<()> {
    let bytes = bincode::serialize(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let len = bytes.len() as u32;
    writer.write_all(&len.to_le_bytes())?;
    writer.write_all(&bytes)?;
    Ok(())
}

const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024; // 64 MiB

/// Read a length-prefixed bincode frame and deserialize it into `T`.
pub fn read_frame<R: std::io::Read, T: serde::de::DeserializeOwned>(
    reader: &mut R,
) -> std::io::Result<T> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame too large: {len} bytes (max {MAX_FRAME_BYTES})"),
        ));
    }
    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf)?;
    bincode::deserialize(&buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::time::SystemTime;

    fn round_trip_request(req: DaemonRequest) -> DaemonRequest {
        let mut buf = Vec::new();
        write_frame(&mut buf, &req).expect("write_frame failed");
        let mut cursor = Cursor::new(buf);
        read_frame::<_, DaemonRequest>(&mut cursor).expect("read_frame failed")
    }

    fn round_trip_response(resp: DaemonResponse) -> DaemonResponse {
        let mut buf = Vec::new();
        write_frame(&mut buf, &resp).expect("write_frame failed");
        let mut cursor = Cursor::new(buf);
        read_frame::<_, DaemonResponse>(&mut cursor).expect("read_frame failed")
    }

    fn sample_entry() -> ClipboardEntry {
        ClipboardEntry {
            id: EntryId(1),
            payload: ContentPayload::PlainText("hello".to_string()),
            captured_at: SystemTime::UNIX_EPOCH,
            pinned: false,
        }
    }

    // -- DaemonRequest round-trips -------------------------------------------

    #[test]
    fn request_get_history() {
        let req = DaemonRequest::GetHistory {
            offset: 0,
            limit: 10,
        };
        assert_eq!(round_trip_request(req.clone()), req);
    }

    #[test]
    fn request_paste_entry() {
        let req = DaemonRequest::PasteEntry { id: EntryId(42) };
        assert_eq!(round_trip_request(req.clone()), req);
    }

    #[test]
    fn request_delete_entry() {
        let req = DaemonRequest::DeleteEntry { id: EntryId(7) };
        assert_eq!(round_trip_request(req.clone()), req);
    }

    #[test]
    fn request_pin_entry() {
        let req = DaemonRequest::PinEntry { id: EntryId(3) };
        assert_eq!(round_trip_request(req.clone()), req);
    }

    #[test]
    fn request_unpin_entry() {
        let req = DaemonRequest::UnpinEntry { id: EntryId(3) };
        assert_eq!(round_trip_request(req.clone()), req);
    }

    #[test]
    fn request_clear_history() {
        let req = DaemonRequest::ClearHistory {
            include_pinned: true,
        };
        assert_eq!(round_trip_request(req.clone()), req);
    }

    #[test]
    fn request_get_config() {
        let req = DaemonRequest::GetConfig;
        assert_eq!(round_trip_request(req.clone()), req);
    }

    #[test]
    fn request_update_config() {
        let req = DaemonRequest::UpdateConfig(Config::default());
        assert_eq!(round_trip_request(req.clone()), req);
    }

    // -- DaemonResponse round-trips ------------------------------------------

    #[test]
    fn response_history() {
        let resp = DaemonResponse::History(vec![sample_entry()]);
        assert_eq!(round_trip_response(resp.clone()), resp);
    }

    #[test]
    fn response_config() {
        let resp = DaemonResponse::Config(Config::default());
        assert_eq!(round_trip_response(resp.clone()), resp);
    }

    #[test]
    fn response_ok() {
        let resp = DaemonResponse::Ok;
        assert_eq!(round_trip_response(resp.clone()), resp);
    }

    #[test]
    fn response_err() {
        let resp = DaemonResponse::Err("something went wrong".to_string());
        assert_eq!(round_trip_response(resp.clone()), resp);
    }

    #[test]
    fn response_new_entry() {
        let resp = DaemonResponse::NewEntry(sample_entry());
        assert_eq!(round_trip_response(resp.clone()), resp);
    }

    // -- content_hash --------------------------------------------------------

    #[test]
    fn content_hash_same_content_same_hash() {
        let entry1 = ClipboardEntry {
            id: EntryId(1),
            payload: ContentPayload::PlainText("hello world".to_string()),
            captured_at: SystemTime::UNIX_EPOCH,
            pinned: false,
        };
        let entry2 = ClipboardEntry {
            id: EntryId(99),
            payload: ContentPayload::PlainText("hello world".to_string()),
            captured_at: SystemTime::now(),
            pinned: true,
        };
        assert_eq!(entry1.content_hash(), entry2.content_hash());
    }

    #[test]
    fn content_hash_different_content_different_hash() {
        let entry1 = ClipboardEntry {
            id: EntryId(1),
            payload: ContentPayload::PlainText("foo".to_string()),
            captured_at: SystemTime::UNIX_EPOCH,
            pinned: false,
        };
        let entry2 = ClipboardEntry {
            id: EntryId(2),
            payload: ContentPayload::PlainText("bar".to_string()),
            captured_at: SystemTime::UNIX_EPOCH,
            pinned: false,
        };
        assert_ne!(entry1.content_hash(), entry2.content_hash());
    }

    #[test]
    fn content_hash_rich_text_uses_html() {
        let entry = ClipboardEntry {
            id: EntryId(1),
            payload: ContentPayload::RichText {
                html: "<b>bold</b>".to_string(),
                plain_preview: "bold".to_string(),
            },
            captured_at: SystemTime::UNIX_EPOCH,
            pinned: false,
        };
        // Hash should be sha256 of the html bytes
        let mut hasher = Sha256::new();
        hasher.update(b"<b>bold</b>");
        let expected = format!("{:x}", hasher.finalize());
        assert_eq!(entry.content_hash(), expected);
    }

    #[test]
    fn request_check_tools_round_trip() {
        let req = DaemonRequest::CheckTools;
        assert_eq!(round_trip_request(req.clone()), req);
    }

    #[test]
    fn response_tools_status_round_trip() {
        let resp = DaemonResponse::ToolsStatus {
            display_server: "Wayland + XWayland".to_string(),
            inject_backend: "xcb XTest via XWayland (built-in)".to_string(),
            clipboard_backend: "wl-copy".to_string(),
            missing_critical: vec![],
            missing_recommended: vec![ToolRequirement {
                tool_name: "ydotool".to_string(),
                purpose: "Inject Ctrl+V into native Wayland apps".to_string(),
                install_hint: "dnf install ydotool".to_string(),
            }],
        };
        assert_eq!(round_trip_response(resp.clone()), resp);
    }

    #[test]
    fn tool_requirement_round_trip() {
        let req = DaemonRequest::CheckTools;
        assert_eq!(round_trip_request(req.clone()), req);
        let resp = DaemonResponse::ToolsStatus {
            display_server: "X11".to_string(),
            inject_backend: "xdotool (X11)".to_string(),
            clipboard_backend: "xclip".to_string(),
            missing_critical: vec![ToolRequirement {
                tool_name: "xdotool".to_string(),
                purpose: "test".to_string(),
                install_hint: "apt install xdotool".to_string(),
            }],
            missing_recommended: vec![],
        };
        assert_eq!(round_trip_response(resp.clone()), resp);
    }

    #[test]
    fn read_frame_truncated_length_returns_err() {
        // Only 3 bytes — not enough for the 4-byte length prefix.
        let truncated = [0u8; 3];
        let mut cursor = Cursor::new(truncated);
        let result = read_frame::<_, DaemonRequest>(&mut cursor);
        assert!(result.is_err(), "expected Err for truncated length prefix");
    }

    #[test]
    fn read_frame_oversized_rejects() {
        // Write a length prefix of 128 MiB — well above the 64 MiB limit.
        let oversized_len: u32 = (128 * 1024 * 1024) as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&oversized_len.to_le_bytes());
        // No payload bytes needed — the guard must reject before allocating.
        let mut cursor = Cursor::new(buf);
        let result = read_frame::<_, DaemonRequest>(&mut cursor);
        assert!(result.is_err(), "expected Err for oversized frame");
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn read_frame_zero_length_payload() {
        // A 0-byte frame is technically valid for bincode (e.g. unit types),
        // but bincode will reject it for non-unit types.  The guard must not
        // reject a 0-length frame before allocating.
        let mut buf = Vec::new();
        buf.extend_from_slice(&0u32.to_le_bytes());
        let mut cursor = Cursor::new(buf);
        // This will fail at the bincode deserialization step, not the size guard.
        let result = read_frame::<_, DaemonRequest>(&mut cursor);
        assert!(result.is_err());
    }

    #[test]
    fn read_frame_exact_limit_is_allowed() {
        // A frame exactly at MAX_FRAME_BYTES should pass the size guard
        // (it will fail later at bincode deserialization since the bytes are
        // not valid, but it must not fail at the guard).
        let len: u32 = MAX_FRAME_BYTES as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_le_bytes());
        // Do NOT add payload bytes — read_exact will return UnexpectedEof,
        // which is distinct from InvalidData.
        let mut cursor = Cursor::new(buf);
        let result = read_frame::<_, DaemonRequest>(&mut cursor);
        assert!(result.is_err());
        assert_ne!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::InvalidData,
            "exact-limit frame must not be rejected by the size guard"
        );
    }

    #[test]
    fn read_frame_one_over_limit_rejects() {
        let len: u32 = MAX_FRAME_BYTES as u32 + 1;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_le_bytes());
        let mut cursor = Cursor::new(buf);
        let result = read_frame::<_, DaemonRequest>(&mut cursor);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::InvalidData,
            "one-over-limit frame must be rejected by the size guard"
        );
    }

    #[test]
    fn read_frame_max_u32_rejects() {
        // 0xFFFFFFFF would cause a ~4 GB allocation without the guard.
        let len: u32 = u32::MAX;
        let mut buf = Vec::new();
        buf.extend_from_slice(&len.to_le_bytes());
        let mut cursor = Cursor::new(buf);
        let result = read_frame::<_, DaemonRequest>(&mut cursor);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn content_hash_image() {
        let data = vec![0u8, 1, 2, 3, 4];
        let entry = ClipboardEntry {
            id: EntryId(1),
            payload: ContentPayload::Image {
                data: data.clone(),
                mime: ImageMime::Png,
            },
            captured_at: SystemTime::UNIX_EPOCH,
            pinned: false,
        };
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let expected = format!("{:x}", hasher.finalize());
        assert_eq!(entry.content_hash(), expected);
    }

}
