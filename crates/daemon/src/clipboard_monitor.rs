use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use common::{ClipboardEntry, ContentPayload, EntryId, ImageMime};
#[cfg(not(test))]
use image::ImageEncoder as _;

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
        let img = self.clipboard.get_image().ok()?;
        let mut png_bytes: Vec<u8> = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png_bytes)
            .write_image(
                &img.bytes,
                img.width as u32,
                img.height as u32,
                image::ExtendedColorType::Rgba8,
            )
            .ok()?;
        Some((png_bytes, ImageMime::Png))
    }

    fn read_html(&mut self) -> Option<(String, String)> {
        // Skip the subprocess call right after a paste so the desktop compositor
        // does not show a "clipboard read by wl-paste" notification for content
        // we just wrote via wl-copy.
        if crate::paste_executor::paste_recently() {
            return None;
        }
        // Try Wayland (wl-paste) then X11 (xclip).
        let html = read_html_wl_paste().or_else(read_html_xclip)?;
        let plain = strip_html_tags(&html);
        Some((html, plain))
    }

    fn read_text(&mut self) -> Option<String> {
        self.clipboard.get_text().ok()
    }
}

/// Read `text/html` from the Wayland clipboard via `wl-paste`.
#[cfg(not(test))]
fn read_html_wl_paste() -> Option<String> {
    let out = std::process::Command::new("wl-paste")
        .args(["--type", "text/html", "--no-newline"])
        .output()
        .ok()?;
    if !out.status.success() { return None; }
    let html = String::from_utf8(out.stdout).ok()?;
    // Reject empty output or output that looks like plain text (no tags).
    if html.trim().is_empty() || !html.contains('<') { return None; }
    Some(html)
}

/// Read `text/html` from the X11 clipboard via `xclip`.
#[cfg(not(test))]
fn read_html_xclip() -> Option<String> {
    let out = std::process::Command::new("xclip")
        .args(["-selection", "clipboard", "-t", "text/html", "-o"])
        .output()
        .ok()?;
    if !out.status.success() { return None; }
    let html = String::from_utf8(out.stdout).ok()?;
    if html.trim().is_empty() || !html.contains('<') { return None; }
    Some(html)
}

/// Strip HTML tags and decode common entities to produce a plain-text preview.
///
/// Block-level tags (`<p>`, `<br>`, `<div>`, `<li>`, headings, …) emit `\n`
/// so that paragraph/line structure is preserved.  Inline tags emit a space.
/// `<style>`, `<script>`, and `<!-- -->` blocks are skipped entirely.
pub(crate) fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut s = html;

    while !s.is_empty() {
        if s.starts_with("<!--") {
            // Skip comment: <!-- ... -->
            s = s[4..].find("-->").map_or("", |i| &s[4 + i + 3..]);
        } else if html_open_tag(s, "style") || html_open_tag(s, "script") {
            // Skip entire style/script block
            let tag = if html_open_tag(s, "style") { "style" } else { "script" };
            let close = format!("</{tag}>");
            let s_lo = s.to_ascii_lowercase();
            s = s_lo.find(close.as_str()).map_or("", |i| &s[i + close.len()..]);
        } else if s.starts_with('<') {
            // Tag: emit '\n' for block-level, ' ' for inline, then skip to '>'.
            let sep = block_sep(s);
            s = s.find('>').map_or("", |i| { out.push(sep); &s[i + 1..] });
        } else {
            // Text content up to next '<'
            match s.find('<') {
                Some(i) => { out.push_str(&s[..i]); s = &s[i..]; }
                None    => { out.push_str(s); break; }
            }
        }
    }

    let decoded = out
        .replace("&amp;",  "&")
        .replace("&lt;",   "<")
        .replace("&gt;",   ">")
        .replace("&nbsp;", " ")
        .replace("&#39;",  "'")
        .replace("&quot;", "\"");

    // Collapse spaces within each line; drop blank lines; join with newline.
    decoded
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// True when `s` starts with `<tag` (case-insensitive) followed by `>`, space, or `/`.
fn html_open_tag(s: &str, tag: &str) -> bool {
    let n = 1 + tag.len();
    s.len() > n
        && s.as_bytes()[0] == b'<'
        && s[1..].get(..tag.len()).map_or(false, |t| t.eq_ignore_ascii_case(tag))
        && matches!(s.as_bytes()[n], b'>' | b' ' | b'\t' | b'\n' | b'\r' | b'/')
}

/// Returns `'\n'` for block-level tags, `' '` for everything else.
fn block_sep(s: &str) -> char {
    const BLOCK: &[&str] = &[
        "p", "br", "div", "h1", "h2", "h3", "h4", "h5", "h6",
        "li", "tr", "dt", "dd", "blockquote", "pre", "hr",
    ];
    // Extract tag name: skip '<' and optional '/' (closing tag).
    let rest = s[1..].trim_start_matches('/');
    let end = rest.find(|c: char| c == '>' || c.is_ascii_whitespace() || c == '/').unwrap_or(rest.len());
    let tag = rest[..end].to_ascii_lowercase();
    if BLOCK.contains(&tag.as_str()) { '\n' } else { ' ' }
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

    /// A reader that produces one image entry then poison-pills.
    struct ImageReader {
        image: Option<(Vec<u8>, ImageMime)>,
        counter: u64,
    }

    impl ImageReader {
        fn new(png: Vec<u8>) -> Self {
            ImageReader { image: Some((png, ImageMime::Png)), counter: 0 }
        }
    }

    impl ClipboardReader for ImageReader {
        fn read_image(&mut self) -> Option<(Vec<u8>, ImageMime)> {
            self.image.take()
        }
        fn read_html(&mut self) -> Option<(String, String)> { None }
        fn read_text(&mut self) -> Option<String> {
            self.counter += 1;
            Some(format!("__stop__{}", self.counter))
        }
    }

    /// Encode a minimal 1×1 red RGBA PNG for use in tests.
    fn tiny_png() -> Vec<u8> {
        use image::ImageEncoder as _;
        let rgba = [255u8, 0, 0, 255]; // one red pixel
        let mut buf = Vec::new();
        image::codecs::png::PngEncoder::new(&mut buf)
            .write_image(&rgba, 1, 1, image::ExtendedColorType::Rgba8)
            .expect("encode failed");
        buf
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

    // -------------------------------------------------------------------------
    // Test 5: image payload is captured and sent as Image entry
    // -------------------------------------------------------------------------
    #[test]
    fn image_reader_produces_image_entry() {
        let (tx, rx) = mpsc::channel();
        let reader = ImageReader::new(tiny_png());
        let handle = ClipboardMonitor::spawn_with_reader(tx, reader);

        let mut image_entry = None;
        loop {
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(entry) => {
                    if matches!(&entry.payload, ContentPayload::Image { .. }) {
                        image_entry = Some(entry);
                        break;
                    }
                    // poison-pill text: keep waiting
                }
                Err(_) => break,
            }
        }

        drop(rx);
        handle.join().ok();

        let entry = image_entry.expect("should have received an image entry");
        assert!(matches!(entry.payload, ContentPayload::Image { mime: ImageMime::Png, .. }));
    }

    // -------------------------------------------------------------------------
    // Test 6: image is not re-sent if the clipboard hasn't changed
    // -------------------------------------------------------------------------
    #[test]
    fn image_deduplication_suppresses_identical_image() {
        // ImageReader sends one image then only poison-pills (text).
        // After the image arrives, any text with a new value will be sent.
        // The image itself should appear exactly once.
        let (tx, rx) = mpsc::channel();
        let reader = ImageReader::new(tiny_png());
        let handle = ClipboardMonitor::spawn_with_reader(tx, reader);

        let mut image_count = 0usize;
        let mut received = 0usize;
        loop {
            match rx.recv_timeout(Duration::from_millis(300)) {
                Ok(entry) => {
                    if matches!(&entry.payload, ContentPayload::Image { .. }) {
                        image_count += 1;
                    }
                    received += 1;
                    if received >= 3 { break; }
                }
                Err(_) => break,
            }
        }

        drop(rx);
        handle.join().ok();

        assert_eq!(image_count, 1, "image should appear exactly once");
    }

    // -------------------------------------------------------------------------
    // Test 7: tiny_png encodes a valid PNG decodable by the image crate
    // -------------------------------------------------------------------------
    #[test]
    fn tiny_png_is_valid_png() {
        let png = tiny_png();
        let img = image::load_from_memory(&png).expect("should decode");
        let rgba = img.to_rgba8();
        assert_eq!(rgba.dimensions(), (1, 1));
        assert_eq!(rgba.get_pixel(0, 0).0, [255, 0, 0, 255]);
    }

    // -------------------------------------------------------------------------
    // Tests for strip_html_tags
    // -------------------------------------------------------------------------

    #[test]
    fn strip_plain_text_unchanged() {
        assert_eq!(strip_html_tags("hello world"), "hello world");
    }

    #[test]
    fn strip_simple_inline_tags() {
        assert_eq!(strip_html_tags("<b>hello</b> <i>world</i>"), "hello world");
    }

    #[test]
    fn strip_skips_style_block() {
        let html = "<style>body { color: red; }</style><p>Hello</p>";
        assert_eq!(strip_html_tags(html), "Hello");
    }

    #[test]
    fn strip_skips_script_block() {
        let html = "<script>alert('xss')</script><p>Safe</p>";
        assert_eq!(strip_html_tags(html), "Safe");
    }

    #[test]
    fn strip_skips_html_comment() {
        let html = "<!-- hidden -->visible";
        assert_eq!(strip_html_tags(html), "visible");
    }

    #[test]
    fn strip_full_browser_html() {
        let html = "<html><head><style>p{color:red}</style></head>\
                    <body><p>Hello <b>world</b></p></body></html>";
        assert_eq!(strip_html_tags(html), "Hello world");
    }

    #[test]
    fn strip_decodes_entities() {
        assert_eq!(strip_html_tags("A &amp; B &lt;3 &gt;"), "A & B <3 >");
    }

    #[test]
    fn strip_collapses_spaces_within_line() {
        assert_eq!(strip_html_tags("<p>  hello   world  </p>"), "hello world");
    }

    #[test]
    fn strip_paragraphs_become_separate_lines() {
        let html = "<p>First</p><p>Second</p><p>Third</p>";
        let result = strip_html_tags(html);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines, ["First", "Second", "Third"]);
    }

    #[test]
    fn strip_br_inserts_newline() {
        let html = "Line one<br>Line two<br/>Line three";
        let result = strip_html_tags(html);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines, ["Line one", "Line two", "Line three"]);
    }

    #[test]
    fn strip_div_inserts_newline() {
        let html = "<div>Alpha</div><div>Beta</div>";
        let result = strip_html_tags(html);
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines.contains(&"Alpha") && lines.contains(&"Beta"),
            "expected separate lines, got: {:?}", result);
    }

    #[test]
    fn strip_list_items_become_lines() {
        let html = "<ul><li>One</li><li>Two</li><li>Three</li></ul>";
        let result = strip_html_tags(html);
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines.contains(&"One"), "missing 'One' in {:?}", result);
        assert!(lines.contains(&"Two"), "missing 'Two' in {:?}", result);
    }

    // ── strip_html_tags: additional edge cases ────────────────────────────────

    #[test]
    fn strip_empty_input_returns_empty() {
        assert_eq!(strip_html_tags(""), "");
    }

    #[test]
    fn strip_only_tags_returns_empty() {
        assert_eq!(strip_html_tags("<html><body></body></html>"), "");
    }

    #[test]
    fn strip_unclosed_tag_does_not_panic() {
        // Unclosed '<' — should not panic, just stop at the unterminated tag.
        let _ = strip_html_tags("hello <b world");
    }

    #[test]
    fn strip_uppercase_tags_treated_as_inline() {
        // <B> and <I> are uppercase inline tags — produce spaces, not newlines.
        let result = strip_html_tags("<B>bold</B> <I>italic</I>");
        assert_eq!(result, "bold italic");
    }

    #[test]
    fn strip_uppercase_block_tags_produce_newlines() {
        // <P> and <BR> in uppercase must still be block-level.
        let result = strip_html_tags("<P>One</P><P>Two</P>");
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines.contains(&"One") && lines.contains(&"Two"),
            "expected two lines, got: {:?}", result);
    }

    #[test]
    fn strip_headings_produce_newlines() {
        let html = "<h1>Title</h1><h2>Subtitle</h2>";
        let result = strip_html_tags(html);
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines.contains(&"Title"), "h1 missing in {:?}", result);
        assert!(lines.contains(&"Subtitle"), "h2 missing in {:?}", result);
    }

    #[test]
    fn strip_pre_block_produces_newline() {
        let html = "<pre>code here</pre><p>text</p>";
        let result = strip_html_tags(html);
        let lines: Vec<&str> = result.lines().collect();
        assert!(lines.contains(&"code here"), "pre content missing in {:?}", result);
        assert!(lines.contains(&"text"), "p content missing in {:?}", result);
    }

    #[test]
    fn strip_nbsp_entity_decoded() {
        assert_eq!(strip_html_tags("hello&nbsp;world"), "hello world");
    }

    #[test]
    fn strip_quot_entity_decoded() {
        assert_eq!(strip_html_tags("say &quot;hi&quot;"), "say \"hi\"");
    }

    #[test]
    fn strip_apos_entity_decoded() {
        assert_eq!(strip_html_tags("it&#39;s"), "it's");
    }

    #[test]
    fn strip_nested_style_inside_body() {
        // Some rich-text editors emit <style> mid-document.
        let html = "<p>Before</p><style>.cls{font:bold}</style><p>After</p>";
        let result = strip_html_tags(html);
        assert!(!result.contains("font"), "CSS leaked into preview: {result}");
        assert!(result.contains("Before") && result.contains("After"),
            "content missing: {result}");
    }

    #[test]
    fn strip_comment_mid_text() {
        let html = "start <!-- ignored section --> end";
        assert_eq!(strip_html_tags(html), "start end");
    }

    #[test]
    fn strip_self_closing_br_produces_newline() {
        let html = "A<br />B";
        let result = strip_html_tags(html);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines, ["A", "B"], "self-closing <br /> should split lines");
    }

    // ── html_open_tag ─────────────────────────────────────────────────────────

    #[test]
    fn html_open_tag_matches_exact() {
        assert!(html_open_tag("<style>", "style"));
        assert!(html_open_tag("<script>", "script"));
    }

    #[test]
    fn html_open_tag_matches_with_attrs() {
        assert!(html_open_tag("<style type=\"text/css\">", "style"));
        assert!(html_open_tag("<script src=\"x.js\">", "script"));
    }

    #[test]
    fn html_open_tag_matches_self_closing() {
        assert!(html_open_tag("<style/>", "style"));
    }

    #[test]
    fn html_open_tag_case_insensitive() {
        assert!(html_open_tag("<STYLE>", "style"));
        assert!(html_open_tag("<Style>", "style"));
    }

    #[test]
    fn html_open_tag_does_not_match_prefix() {
        // <stylesheet> must NOT match tag "style"
        assert!(!html_open_tag("<stylesheet>", "style"));
    }

    #[test]
    fn html_open_tag_does_not_match_closing_tag() {
        // </style> starts with '<' + '/' — open-tag check should still return true
        // because we call it on the raw `s` pointer before any '/'-stripping;
        // actually the fn checks s[1..] against tag, and '</style>' has '/' at [1],
        // so it should NOT match for "style".
        assert!(!html_open_tag("</style>", "style"));
    }

    #[test]
    fn html_open_tag_empty_string_does_not_panic() {
        assert!(!html_open_tag("", "style"));
        assert!(!html_open_tag("<", "style"));
    }

    // ── block_sep ─────────────────────────────────────────────────────────────

    #[test]
    fn block_sep_p_is_newline() {
        assert_eq!(block_sep("<p>"), '\n');
        assert_eq!(block_sep("</p>"), '\n');
    }

    #[test]
    fn block_sep_br_is_newline() {
        assert_eq!(block_sep("<br>"), '\n');
        assert_eq!(block_sep("<br/>"), '\n');
        assert_eq!(block_sep("<br />"), '\n');
    }

    #[test]
    fn block_sep_div_is_newline() {
        assert_eq!(block_sep("<div>"), '\n');
        assert_eq!(block_sep("</div>"), '\n');
    }

    #[test]
    fn block_sep_headings_are_newline() {
        for tag in ["<h1>", "<h2>", "<h3>", "<h4>", "<h5>", "<h6>"] {
            assert_eq!(block_sep(tag), '\n', "{tag} should be block");
        }
    }

    #[test]
    fn block_sep_li_tr_are_newline() {
        assert_eq!(block_sep("<li>"), '\n');
        assert_eq!(block_sep("<tr>"), '\n');
    }

    #[test]
    fn block_sep_inline_tags_are_space() {
        for tag in ["<b>", "<i>", "<span>", "<a>", "<strong>", "<em>"] {
            assert_eq!(block_sep(tag), ' ', "{tag} should be inline");
        }
    }

    #[test]
    fn block_sep_uppercase_block_is_newline() {
        assert_eq!(block_sep("<P>"), '\n');
        assert_eq!(block_sep("<BR>"), '\n');
        assert_eq!(block_sep("<DIV>"), '\n');
    }

}
