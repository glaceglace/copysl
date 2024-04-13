use crate::clipboard_utils::ClipboardObserver;

pub mod clipboard_utils {
    use arboard::{Clipboard, Error};

    pub struct ClipboardObserver {
        pub last_content: String,
        pub clipboard: Clipboard,
    }

    impl ClipboardObserver {
        pub fn new_with_clipboard(clipboard: Clipboard) -> ClipboardObserver {
            ClipboardObserver {
                last_content: String::from(""),
                clipboard: clipboard,
            }
        }
        pub fn new() -> Result<ClipboardObserver, Error> {
            let clipboard_result = Clipboard::new();
            match clipboard_result {
                Ok(good_clipboard) => {
                    Result::Ok(
                        ClipboardObserver {
                            last_content: String::from(""),
                            clipboard: good_clipboard,
                        }
                    )
                }
                Err(err) => { Result::Err(err) }
            }
        }
        pub fn observe<'a>(&'a mut self, callback: &'a mut dyn FnMut(&'a str)) {
            let clipboard_text_result: Result<String, Error> = self.clipboard.get_text();
            match clipboard_text_result {
                Ok(clipboard_text) => {
                    match (self.last_content.as_str(), clipboard_text.as_str()) {
                        ("", "") => {}
                        ("", _) => {
                            self.last_content = clipboard_text.clone();
                            callback(self.last_content.as_str());
                        }
                        (_, "") => {}
                        (_, _) => {
                            if clipboard_text.ne(&self.last_content) {
                                self.last_content = clipboard_text.clone();
                                callback(self.last_content.as_str());
                            }
                        }
                    }
                }
                Err(err) => {
                    println!("Error while retrieving clipboard content, nested error is {}", err);
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use arboard::Clipboard;
    use super::ClipboardObserver;

    #[test]
    fn test_closure() {
        let mut clipboard_observer = ClipboardObserver::new().unwrap();
        let mut code = 1;
        clipboard_observer.observe(&mut |_| {
            code = 0;
        });
        assert_eq!(code, 0)
    }

    #[test]
    fn test_observer_good_capture() {
        let mut clipboard_observer = ClipboardObserver::new().unwrap();
        let mut clipboard = Clipboard::new().unwrap();
        assert_eq!(clipboard_observer.last_content, "");
        clipboard.set_text("ABC").unwrap();
        clipboard_observer.observe(&mut |content| {
            assert_eq!(content, "ABC");
        });
        assert_eq!(clipboard_observer.last_content.as_str(), "ABC");
        clipboard.set_text("").unwrap();
        clipboard_observer.observe(&mut |content| {
            assert_eq!(content, "ABC");
        });
        assert_eq!(clipboard_observer.last_content.as_str(), "ABC");
        clipboard.set_text("DEF").unwrap();
        clipboard_observer.observe(&mut |content| {
            assert_eq!(content, "DEF");
        });
        assert_eq!(clipboard_observer.last_content.as_str(), "DEF");
    }
}