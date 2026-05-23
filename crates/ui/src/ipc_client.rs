use std::io::{self, Read};
use std::os::unix::net::UnixStream;
use anyhow::Result;
use common::{DaemonRequest, DaemonResponse};

pub struct IpcClient {
    stream: UnixStream,
    /// Bytes received but not yet parsed into a complete frame.
    recv_buf: Vec<u8>,
}

impl IpcClient {
    pub fn connect() -> Result<Self> {
        let socket_path = std::env::var("COPIEUR_SOCKET")
            .map_err(|_| anyhow::anyhow!("COPIEUR_SOCKET environment variable not set"))?;

        let stream = UnixStream::connect(&socket_path)
            .map_err(|e| anyhow::anyhow!("Failed to connect to daemon at {}: {}", socket_path, e))?;

        stream.set_nonblocking(false)?;
        Ok(IpcClient { stream, recv_buf: Vec::new() })
    }

    pub fn connect_to(socket_path: &str) -> Result<Self> {
        let stream = UnixStream::connect(socket_path)
            .map_err(|e| anyhow::anyhow!("Failed to connect to {}: {}", socket_path, e))?;
        stream.set_nonblocking(false)?;
        Ok(IpcClient { stream, recv_buf: Vec::new() })
    }

    /// Blocking send + receive (used at startup for GetHistory / GetConfig).
    pub fn send(&mut self, req: &DaemonRequest) -> Result<DaemonResponse> {
        self.stream.set_nonblocking(false)?;
        common::write_frame(&mut self.stream, req)?;
        let response: DaemonResponse = common::read_frame(&mut self.stream)?;
        Ok(response)
    }

    /// Write a request without waiting for a response.
    pub fn send_fire_forget(&mut self, req: &DaemonRequest) -> Result<()> {
        self.stream.set_nonblocking(false)?;
        common::write_frame(&mut self.stream, req)?;
        Ok(())
    }

    /// Drain all available bytes from the socket into `recv_buf`, then try
    /// to parse one complete frame. Never blocks — safe to call every frame.
    ///
    /// The daemon sends `DaemonResponse::Ok` for every fire-and-forget request
    /// as well as `DaemonResponse::NewEntry` push notifications; both are read
    /// here and the caller filters by variant.
    pub fn try_recv_push(&mut self) -> Option<DaemonResponse> {
        // Switch to non-blocking so reads never stall the UI thread.
        self.stream.set_nonblocking(true).ok()?;

        // Drain whatever the kernel has buffered.
        let mut tmp = [0u8; 4096];
        loop {
            match self.stream.read(&mut tmp) {
                Ok(0) => break,
                Ok(n) => self.recv_buf.extend_from_slice(&tmp[..n]),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }

        // Attempt to extract one complete length-prefixed frame.
        if self.recv_buf.len() < 4 {
            return None;
        }
        let body_len = u32::from_le_bytes(self.recv_buf[..4].try_into().ok()?) as usize;
        if self.recv_buf.len() < 4 + body_len {
            return None; // frame not yet fully buffered; try again next frame
        }
        let body = self.recv_buf[4..4 + body_len].to_vec();
        self.recv_buf.drain(..4 + body_len);
        bincode::deserialize::<DaemonResponse>(&body).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::{DaemonRequest, DaemonResponse};
    use std::os::unix::net::UnixListener;
    use tempfile::tempdir;

    #[test]
    fn connect_fails_with_no_socket_env() {
        std::env::set_var("COPIEUR_SOCKET", "/tmp/__copieur_test_nonexistent__.sock");
        let result = IpcClient::connect();
        assert!(result.is_err());
    }

    #[test]
    fn connect_to_nonexistent_path_fails() {
        let result = IpcClient::connect_to("/nonexistent/path/copieur.sock");
        assert!(result.is_err());
    }

    #[test]
    fn send_and_receive_against_mock_server() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        let sock_path_str = sock_path.to_str().unwrap().to_string();

        let listener = UnixListener::bind(&sock_path).unwrap();
        let server_thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let req: DaemonRequest = common::read_frame(&mut stream).unwrap();
            assert!(matches!(req, DaemonRequest::GetHistory { .. }));
            let resp = DaemonResponse::History(vec![]);
            common::write_frame(&mut stream, &resp).unwrap();
        });

        std::thread::sleep(std::time::Duration::from_millis(10));

        let mut client = IpcClient::connect_to(&sock_path_str).unwrap();
        let response = client
            .send(&DaemonRequest::GetHistory { offset: 0, limit: 10 })
            .unwrap();

        assert!(matches!(response, DaemonResponse::History(v) if v.is_empty()));
        server_thread.join().unwrap();
    }

    #[test]
    fn try_recv_push_returns_none_when_no_data() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("test_push.sock");
        let sock_path_str = sock_path.to_str().unwrap().to_string();

        let listener = UnixListener::bind(&sock_path).unwrap();
        let _server = std::thread::spawn(move || {
            let (_stream, _) = listener.accept().unwrap();
            std::thread::sleep(std::time::Duration::from_millis(500));
        });

        std::thread::sleep(std::time::Duration::from_millis(10));

        let mut client = IpcClient::connect_to(&sock_path_str).unwrap();
        assert!(client.try_recv_push().is_none());
    }

    #[test]
    fn try_recv_push_parses_buffered_frame() {
        let dir = tempdir().unwrap();
        let sock_path = dir.path().join("test_buf.sock");
        let sock_path_str = sock_path.to_str().unwrap().to_string();

        let listener = UnixListener::bind(&sock_path).unwrap();
        let server_thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            // Send two frames back-to-back
            common::write_frame(&mut stream, &DaemonResponse::Ok).unwrap();
            common::write_frame(&mut stream, &DaemonResponse::Ok).unwrap();
        });

        std::thread::sleep(std::time::Duration::from_millis(10));

        let mut client = IpcClient::connect_to(&sock_path_str).unwrap();
        // Give server time to write both frames
        std::thread::sleep(std::time::Duration::from_millis(20));

        // First call drains both frames into recv_buf and returns the first
        let r1 = client.try_recv_push();
        assert!(matches!(r1, Some(DaemonResponse::Ok)));
        // Second call returns the already-buffered second frame
        let r2 = client.try_recv_push();
        assert!(matches!(r2, Some(DaemonResponse::Ok)));
        // Third call: nothing left
        assert!(client.try_recv_push().is_none());

        server_thread.join().unwrap();
    }
}
