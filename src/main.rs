use std::env;
use std::fmt::Debug;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

use arboard::Clipboard;
use fltk::{app, prelude::*};
use fltk::app::App;
use fltk::text::TextDisplay;
use once_cell::sync::Lazy;

use copysl::clipboard_utils::ClipboardObserver;
use copysl::stack::Stack;

use crate::keyboard_obs::observe_linux_kbd;

mod keyboard_obs;
mod daemonization;
mod gui;

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
static APP: Lazy<App> = Lazy::new(|| { App::default().with_scheme(app::Scheme::Plastic) });


fn main() {
    // Verify root privilege
    if (env::var("USER").unwrap()) != "root" { panic!("Copysl needs root privileges to know shortcut is pressed when it is running in background.") }

    //channel for listening keyboard event
    let (sender, receiver): (Sender<i32>, Receiver<i32>) = mpsc::channel();

    // Prepare application daemon
    // daemonization::daemonize();

    // Prepare window and its widgets
    let (w, h, mut my_window, mut scroll) = gui::prepare_gui();

    observe_linux_kbd(sender);

    while APP.wait() {
        let mut visible = my_window.visible();
        keyboard_obs::wait_for_key_press(&receiver, &mut my_window, &mut visible);
        gui::app_internal_action(&w, &h, &mut my_window, &mut scroll);
    };


    println!("Exit app loop");
}


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

