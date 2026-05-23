use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use common::{ClipboardEntry, ContentPayload, EntryId, ImageMime};

const POLL_INTERVAL_MS: u64 = 200;

pub trait ClipboardReader: Send + 'static {
    fn read_image(&mut self) -> Option<(Vec<u8>, ImageMime)>;
    fn read_html(&mut self) -> Option<(String, String)>; // (html, plain_preview)
    fn read_text(&mut self) -> Option<String>;
}

pub struct ClipboardMonitor;

impl ClipboardMonitor {
    #[cfg(not(test))]
    pub fn spawn(tx: mpsc::Sender<ClipboardEntry>) -> thread::JoinHandle<()> {
        Self::spawn_with_reader(tx, RealClipboardReader::new())
    }

    pub fn spawn_with_reader<R: ClipboardReader>(
        tx: mpsc::Sender<ClipboardEntry>,
        mut reader: R,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let mut last_hash: Option<String> = None;
            loop {
                let payload = reader
                    .read_image()
                    .map(|(data, mime)| ContentPayload::Image { data, mime })
                    .or_else(|| {
                        reader.read_html().map(|(html, plain_preview)| {
                            ContentPayload::RichText { html, plain_preview }
                        })
                    })
                    .or_else(|| reader.read_text().map(ContentPayload::PlainText));

                if let Some(payload) = payload {
                    let entry = ClipboardEntry {
                        id: EntryId(0),
                        payload,
                        captured_at: std::time::SystemTime::now(),
                        pinned: false,
                    };
                    let hash = entry.content_hash();
                    if last_hash.as_deref() != Some(&hash) {
                        last_hash = Some(hash);
                        if tx.send(entry).is_err() {
                            break;
                        }
                    }
                }

                thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
            }
        })
    }
}

#[cfg(not(test))]
struct RealClipboardReader {
    clipboard: arboard::Clipboard,
}

#[cfg(not(test))]
impl RealClipboardReader {
    fn new() -> Self {
        RealClipboardReader {
            clipboard: arboard::Clipboard::new().expect("Failed to open clipboard"),
        }
    }
}

#[cfg(not(test))]
impl ClipboardReader for RealClipboardReader {
    fn read_image(&mut self) -> Option<(Vec<u8>, ImageMime)> {
        None
    }

    fn read_html(&mut self) -> Option<(String, String)> {
        None
    }

    fn read_text(&mut self) -> Option<String> {
        self.clipboard.get_text().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::mpsc;

    /// A reader backed by a finite deque of items.
    ///
    /// While items remain, it returns them one per poll cycle.  Once the deque
    /// is exhausted it returns a unique poison-pill string on every subsequent
    /// call so the thread always attempts a `tx.send()`.  When the test drops
    /// the receiver the next send will fail and the thread exits cleanly.
    struct FiniteReader {
        items: VecDeque<Option<String>>,
        exhausted_counter: u64,
    }

    impl FiniteReader {
        fn new(items: Vec<Option<String>>) -> Self {
            FiniteReader {
                items: items.into(),
                exhausted_counter: 0,
            }
        }
    }

    impl ClipboardReader for FiniteReader {
        fn read_image(&mut self) -> Option<(Vec<u8>, ImageMime)> {
            None
        }
        fn read_html(&mut self) -> Option<(String, String)> {
            None
        }
        fn read_text(&mut self) -> Option<String> {
            if !self.items.is_empty() {
                self.items.pop_front().unwrap_or(None)
            } else {
                // Poison pill: always return a unique value so send() is
                // attempted and the thread notices the closed channel.
                self.exhausted_counter += 1;
                Some(format!("__stop__{}", self.exhausted_counter))
            }
        }
    }

    // -------------------------------------------------------------------------
    // Test 1: identical reads produce only one send
    // -------------------------------------------------------------------------
    #[test]
    fn identical_reads_do_not_produce_two_sends() {
        let (tx, rx) = mpsc::channel();

        // Items: "hello", "hello" (dup — should be suppressed), "world"
        // After these, poison-pill strings keep the thread active until we
        // drop rx.
        let reader = FiniteReader::new(vec![
            Some("hello".to_string()),
            Some("hello".to_string()),
            Some("world".to_string()),
        ]);

        let handle = ClipboardMonitor::spawn_with_reader(tx, reader);

        // Collect up to 3 messages; we expect exactly 2 real ones.
        // The third recv_timeout will either get a poison pill (which we don't
        // want) or time out.  We stop collecting after we see "world".
        let mut received = Vec::new();
        loop {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(entry) => {
                    let is_stop = matches!(&entry.payload,
                        ContentPayload::PlainText(t) if t.starts_with("__stop__"));
                    if !is_stop {
                        received.push(entry);
                    }
                    // Stop collecting once we have world
                    if received.len() == 2 {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        // Dropping rx causes the next tx.send() to fail → thread exits.
        drop(rx);
        handle.join().ok();

        assert_eq!(received.len(), 2, "expected 2 unique entries, got {}", received.len());

        assert!(
            matches!(&received[0].payload, ContentPayload::PlainText(t) if t == "hello"),
            "first entry should be 'hello'"
        );
        assert!(
            matches!(&received[1].payload, ContentPayload::PlainText(t) if t == "world"),
            "second entry should be 'world'"
        );
    }

    // -------------------------------------------------------------------------
    // Test 2: distinct reads each produce a send
    // -------------------------------------------------------------------------
    #[test]
    fn distinct_reads_each_produce_a_send() {
        let (tx, rx) = mpsc::channel();

        let reader = FiniteReader::new(vec![
            Some("alpha".to_string()),
            Some("beta".to_string()),
            Some("gamma".to_string()),
        ]);

        let handle = ClipboardMonitor::spawn_with_reader(tx, reader);

        // Collect real (non-poison) entries until we have 3 or time out.
        let mut received = Vec::new();
        loop {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(entry) => {
                    let is_stop = matches!(&entry.payload,
                        ContentPayload::PlainText(t) if t.starts_with("__stop__"));
                    if !is_stop {
                        received.push(entry);
                    }
                    if received.len() == 3 {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        drop(rx);
        handle.join().ok();

        assert_eq!(received.len(), 3, "expected 3 distinct entries");
    }

    // -------------------------------------------------------------------------
    // Test 3: None reads produce no sends (before poison pills kick in)
    // -------------------------------------------------------------------------
    #[test]
    fn none_reads_produce_no_sends() {
        let (tx, rx) = mpsc::channel();

        // Three None items.  After they're exhausted the reader returns poison
        // pills, but we don't care about those — we only assert that the
        // very first message is a poison pill (meaning no real content was sent
        // before it).
        let reader = FiniteReader::new(vec![None, None, None]);

        let handle = ClipboardMonitor::spawn_with_reader(tx, reader);

        // The first thing to arrive should be a poison pill, not real content.
        let first = rx.recv_timeout(Duration::from_millis(500)).ok();

        drop(rx);
        handle.join().ok();

        match first {
            None => {} // timed out — also acceptable, means nothing real sent
            Some(entry) => {
                assert!(
                    matches!(&entry.payload,
                        ContentPayload::PlainText(t) if t.starts_with("__stop__")),
                    "expected only poison-pill entries after None reads, got {:?}",
                    entry.payload
                );
            }
        }
    }

    // -------------------------------------------------------------------------
    // Test 4: thread stops when receiver is dropped mid-stream
    // -------------------------------------------------------------------------
    #[test]
    fn thread_stops_when_receiver_dropped() {
        let (tx, rx) = mpsc::channel();

        // Many unique strings — thread will keep sending.
        let items: Vec<Option<String>> = (0..1000).map(|i| Some(i.to_string())).collect();
        let reader = FiniteReader::new(items);

        let handle = ClipboardMonitor::spawn_with_reader(tx, reader);

        // Receive a few, then drop the receiver.
        let _ = rx.recv_timeout(Duration::from_millis(500));
        drop(rx);

        let result = handle.join();
        assert!(result.is_ok(), "thread should exit cleanly after receiver drop");
    }
}
