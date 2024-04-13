use std::fmt::Debug;
use arboard::Clipboard;

use fltk::{app, prelude::*, text, window::Window};
use fltk::app::{event_dy, event_x, event_y};
use fltk::button::Button;
use fltk::draw::set_cursor;
use fltk::enums::Cursor::Hand;
use fltk::enums::Event;
use fltk::group::Scroll;
use fltk::text::{Cursor, TextDisplay, TextEditor};
use fltk::widget::Widget;
use fltk_sys::fl;

use copysl::clipboard_utils::ClipboardObserver;

mod ui;

// let CLIPBOARD_VEC_PTR: *mut Vec<ClipBoardElement> = vec![].as_mut_ptr();
static mut CLIPBOARD_VEC: ClipboardVec = ClipboardVec(vec![]);
static mut DISPLAY_VEC: DisplaydVec = DisplaydVec(vec![]);


fn main() {
    let mut clipboard_observer = match Clipboard::new() {
        Ok(good_clipboard) => {
            ClipboardObserver::new_with_clipboard(good_clipboard)
        }
        Err(err) => { panic!("Failed to observe clipboard, nested error is {}", err) }
    };


    let app = app::App::default().with_scheme(app::Scheme::Plastic);

    let (w, h): (i32, i32) = adapted_window_size_wh();
    let mut my_window = Window::new(100, 100, w, h, "My Window");


    let mut scroll = Scroll::new(0, 0, w, h + 7, None);
    scroll.set_scrollbar_size(7);


    my_window.end();
    my_window.show();
    let mut total_height = 0;

    // app.run().unwrap()
    while app.wait() {
        unsafe {
            clipboard_observer.observe(&mut |last_content| {
                // let last_content_clone = last_content.to_owned();
                CLIPBOARD_VEC.0.push(ClipBoardElement::build(last_content));

                let height = ((h - 10) as f32 / 5.0).round() as i32;
                let mut display_element = TextDisplay::new(0, total_height, w, height, None);
                let last_element = DISPLAY_VEC.0.last();
                if last_element.is_some() {
                    display_element = display_element.below_of(last_element.unwrap(), 3)
                }
                // display_element.deactivate();
                display_element.handle(move |td: &mut TextDisplay, event: Event| {
                    if event == Event::Push {
                        let mut cb = match Clipboard::new() {
                            Ok(good_clipboard) => {
                                good_clipboard
                            }
                            Err(err) => {
                                println!("Failed to observe clipboard, nested error is {}", err);
                                return true;
                            }
                        };

                        cb.set_text(
                            &CLIPBOARD_VEC.0.last()
                                .expect(
                                    "The content shall sure be existed").content
                        )
                            .unwrap_or_else(|err|
                                {
                                    println!("Error while clicking on a existed content, nested error is {}", err)
                                }
                            );
                        return true;
                    }

                    if event == Event::Move {
                        let mouse_x = event_x();
                        let mouse_y = event_y();

                        return if mouse_x >= td.x() && mouse_x <= td.x() + td.w() && mouse_y >= td.y() && mouse_y <= td.y() + td.h() {
                            set_cursor(fltk::enums::Cursor::Hand);
                            true
                        } else {
                            set_cursor(fltk::enums::Cursor::Default);
                            true
                        };
                    }

                    false
                });

                display_element.set_scrollbar_size(1);
                let mut buf = text::TextBuffer::default();
                display_element.set_buffer(buf.clone());
                buf.set_text(last_content);
                scroll.add(&display_element);
                scroll.end();

                DISPLAY_VEC.0.push(display_element);
                total_height += height;
                my_window.redraw();
            });
        }
    };
}

#[derive(Debug)]
struct ClipboardVec(Vec<ClipBoardElement>);

#[derive(Debug)]
struct DisplaydVec(Vec<TextDisplay>);


#[derive(Debug)]
pub struct ClipBoardElement {
    pub content: String,
    pub element_type: ElementType,
    pub thumbnail: Option<String>,
    pub pined: bool,
}

impl ClipBoardElement {
    pub fn build(content: &str) -> ClipBoardElement {
        ClipBoardElement {
            content: content.to_string(),
            element_type: ElementType::TEXT,
            thumbnail: None,
            pined: false,
        }
    }
}

#[derive(Debug)]
pub enum ElementType {
    TEXT,
    IMAGE,
}

pub struct WSize(i32);

pub struct HSize(i32);

trait Size {
    const TIMES_CONST: f32 = 1000.0;
    fn to_px(&self) -> i32;
}

impl Size for WSize {
    fn to_px(&self) -> i32 {
        //TODO make a responsive pixel
        let scale: f32 = 1.0;
        let current_screen_w: i32 = app::Screen::all_screens().iter()
            .filter(|screen| {
                screen.is_valid()
            })
            .next()
            .map(|screen| screen.w())
            .unwrap_or(2560);
        (self.0 as f32 * current_screen_w as f32 * scale / Self::TIMES_CONST).round() as i32
    }
}

impl Size for HSize {
    fn to_px(&self) -> i32 {
        //TODO make a responsive pixel
        let scale: f32 = 1.0;
        let current_screen_h: i32 = app::Screen::all_screens().iter()
            .filter(|screen| {
                screen.is_valid()
            })
            .next()
            .map(|screen| screen.h())
            .unwrap_or(1440);
        (self.0 as f32 * current_screen_h as f32 * scale / Self::TIMES_CONST).round() as i32
    }
}

pub fn adapted_window_size_wh() -> (i32, i32) {
    let coef_w: f32 = 7.5;
    let coef_h: f32 = 2.0;
    let (current_screen_w, current_screen_h): (i32, i32) = app::Screen::all_screens().iter()
        .filter(|screen| {
            screen.is_valid()
        })
        .next()
        .map(|screen| (screen.w(), screen.h()))
        .unwrap_or((2560, 1440));


    ((current_screen_w as f32 / coef_w).round() as i32, (current_screen_h as f32 / coef_h).round() as i32)
}