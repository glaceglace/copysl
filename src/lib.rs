pub mod clipboard_obs;

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

pub mod stack {
    use std::fmt::{Debug, Formatter};

    pub struct Stack<T> {
        pub collection: Vec<T>,
    }

    impl<T: Debug> Debug for Stack<T> {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            Debug::fmt(&self.collection, f)
        }
    }

    impl<T> Default for Stack<T> {
        fn default() -> Self {
            Self::new()
        }
    }

    impl<T> Iterator for Stack<T> {
        type Item = T;

        fn next(&mut self) -> Option<Self::Item> {
            self.pop()
        }
    }

    impl<T> Stack<T> {
        pub fn new() -> Self {
            Stack {
                collection: vec![]
            }
        }
        pub fn is_empty(&self) -> bool {
            self.collection.is_empty()
        }
        pub fn push(&mut self, element: T) {
            self.collection.insert(0, element)
        }
        pub fn pop(&mut self) -> Option<T> {
            if self.is_empty() {
                None
            } else {
                Some(self.collection.remove(0))
            }
        }
        pub fn peek(&mut self) -> Option<&T> {
            self.collection.first()
        }
        pub fn len(&self) -> usize {
            self.collection.len()
        }
        pub fn clear(&mut self) {
            self.collection.clear();
        }
        pub fn get(&mut self, idx: usize) -> Option<&T> {
            self.collection.get(idx)
        }
    }
}

#[cfg(test)]
mod stack_tests {
    use crate::stack::Stack;

    #[test]
    fn test_stack() {
        let mut stack: Stack<i32> = Stack::new();
        stack.push(0);
        stack.push(1);
        stack.push(2);
        stack.push(3);
        assert_eq!(*stack.collection.first().unwrap(), 3);
        assert_eq!(*stack.collection.get(1).unwrap(), 2);
        assert_eq!(*stack.collection.get(2).unwrap(), 1);
        assert_eq!(*stack.collection.last().unwrap(), 0);

        assert_eq!(*stack.peek().unwrap(), 3);
        assert_eq!(stack.len(), 4);
        assert_eq!(stack.pop().unwrap(), 3);
        assert_eq!(stack.pop().unwrap(), 2);
        assert_eq!(stack.len(), 2);
        assert_eq!(*stack.peek().unwrap(), 1);
        assert_eq!(stack.is_empty(), false);
        stack.pop();
        stack.pop();
        assert_eq!(stack.is_empty(), true);
        assert_eq!(stack.pop(), None);
    }
}

#[cfg(test)]
mod clipboard_observer_tests {
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