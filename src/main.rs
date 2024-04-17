use std::cell::RefCell;
use std::fmt::Debug;
use std::path::Display;
use std::rc::Rc;
use arboard::{Clipboard, Error};

use fltk::{app, prelude::*, text, window::Window};
use fltk::app::{event_dy, event_x, event_y, windows};
use fltk::button::Button;
use fltk::draw::set_cursor;
use fltk::enums::Cursor::Hand;
use fltk::enums::Event;
use fltk::group::Scroll;
use fltk::text::{Cursor, TextDisplay, TextEditor};
use fltk::widget::Widget;
use fltk_sys::fl;
use once_cell::sync::Lazy;
use uinput::Device;

use copysl::clipboard_utils::ClipboardObserver;
use copysl::stack::Stack;
use copysl::system::paste_action;

mod ui;

// let CLIPBOARD_VEC_PTR: *mut Vec<ClipBoardElement> = vec![].as_mut_ptr();
static mut CLIPBOARD_STACK: Lazy<Stack<ClipBoardElement>> = Lazy::new(|| { Stack::new() });
static mut DISPLAY_VEC: Lazy<Vec<TextDisplay>> = Lazy::new(|| { vec![] });
static mut CLIPBOARD_OBSERVER: Lazy<ClipboardObserver> = Lazy::new(|| {
    match Clipboard::new() {
        Ok(good_clipboard) => {
            ClipboardObserver::new_with_clipboard(good_clipboard)
        }
        Err(err) => { panic!("Failed to observe clipboard, nested error is {}", err) }
    }
});



fn main() {


    let app = app::App::default().with_scheme(app::Scheme::Plastic);

    let (w, h): (i32, i32) = adapted_window_size_wh();
    let mut my_window = Rc::new(RefCell::new(Window::new(100, 100, w, h, "My Window")));



    let mut scroll = Scroll::new(0, 0, w, h + 7, None);
    scroll.set_scrollbar_size(7);


    my_window.borrow().end();
    my_window.borrow_mut().show();
    // app.run().unwrap()
    let my_window_own = my_window.to_owned();
    while app.wait() {

        
        unsafe {
            CLIPBOARD_OBSERVER.observe(&mut |last_content| {
                println!("+++++++{:#?}", last_content);

                let found_cb_element = CLIPBOARD_STACK.collection.iter().enumerate()
                    .find(|(_, cb_element)| {
                        last_content.eq(&cb_element.content)
                    });

                if found_cb_element.is_some() {
                    let (idx, _) = found_cb_element.unwrap();
                    let removed = CLIPBOARD_STACK.collection.remove(idx);
                    CLIPBOARD_STACK.push(removed);
                } else {
                    CLIPBOARD_STACK.push(ClipBoardElement::build(last_content));
                }

                DISPLAY_VEC.clear();

                let height = ((h - 10) as f32 / 5.0).round() as i32;
                for (i, cb) in CLIPBOARD_STACK.collection.iter().enumerate() {
                    let scroll_y = scroll.yposition();
                    let mut td_element: TextDisplay = if i == 0 {
                        TextDisplay::new(0, 0 - scroll_y, w, height, None)
                    } else {
                        TextDisplay::new(0, 0 - scroll_y, w, height, None).below_of(DISPLAY_VEC.get(i - 1).unwrap(), 3)
                    };


                    let my_window_clone = Rc::clone(&my_window_own);
                   
                    td_element.handle( move |td: &mut TextDisplay, event: Event| {

                        if event == Event::Push {
                            //TODO verify right click or left click
                            let text = td.buffer().expect("The content shall sure be existed").text();
                            match CLIPBOARD_OBSERVER.clipboard.set_text(text) {
                                Ok(_) => {
                                    // CLIPBOARD_STACK.push(text);
                                    paste_action();
                                    my_window_clone.try_borrow_mut().expect("The window shall be existed").iconize();
                                }
                                Err(err) => { println!("Error while clicking on a existed content, nested error is {}", err) }
                            }

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
                    td_element.set_scrollbar_size(1);
                    let mut buf = text::TextBuffer::default();
                    td_element.set_buffer(buf.clone());
                    buf.set_text(&cb.content);
                    scroll.add(&td_element);
                    DISPLAY_VEC.push(td_element);
                }
                scroll.end();
                my_window.try_borrow_mut().expect("The window shall be existed").redraw();
            });
        }
    };
}

#[derive(Debug)]
struct ClipboardVec(Stack<ClipBoardElement>);

#[derive(Debug)]
struct DisplaydVec(Stack<TextDisplay>);


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
            .find(|screen| {
                screen.is_valid()
            })
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
            .find(|screen| {
                screen.is_valid()
            })
            .map(|screen| screen.h())
            .unwrap_or(1440);
        (self.0 as f32 * current_screen_h as f32 * scale / Self::TIMES_CONST).round() as i32
    }
}

pub fn adapted_window_size_wh() -> (i32, i32) {
    let coef_w: f32 = 7.5;
    let coef_h: f32 = 2.0;
    let (current_screen_w, current_screen_h): (i32, i32) = app::Screen::all_screens().iter()
        .find(|screen| {
            screen.is_valid()
        })
        .map(|screen| (screen.w(), screen.h()))
        .unwrap_or((2560, 1440));


    ((current_screen_w as f32 / coef_w).round() as i32, (current_screen_h as f32 / coef_h).round() as i32)
}
