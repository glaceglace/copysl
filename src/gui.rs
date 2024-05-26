use fltk::{app, prelude::*, text, window::Window};
use fltk::app::{event_x, event_y};
use fltk::draw::set_cursor;
use fltk::enums::{Damage, Event};
use fltk::group::Scroll;
use fltk::text::TextDisplay;
use fltk::window::DoubleWindow;

use copysl::clipboard_obs::paste_action;

use crate::{CLIPBOARD_OBSERVER, CLIPBOARD_STACK, ClipBoardElement, DISPLAY_VEC};

pub fn prepare_gui() -> (i32, i32, DoubleWindow, Scroll) {
    let (w, h): (i32, i32) = adapted_window_size_wh();
    let mut my_window: DoubleWindow = Window::new(100, 100, w, h, "copysl");
    let mut scroll = Scroll::new(0, 0, w, h + 7, None);
    my_window.set_damage(false);
    my_window.set_damage_type(Damage::None);
    scroll.set_scrollbar_size(7);
    my_window.end();
    my_window.show();
    (w, h, my_window, scroll)
}

pub fn app_internal_action(w: &i32, h: &i32, my_window: &mut DoubleWindow, scroll: &mut Scroll) {
    unsafe {
        CLIPBOARD_OBSERVER.observe(&mut |last_content| {
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
                    TextDisplay::new(0, 0 - scroll_y, *w, height, None)
                } else {
                    TextDisplay::new(0, 0 - scroll_y, *w, height, None).below_of(DISPLAY_VEC.get(i - 1).unwrap(), 3)
                };


                let my_window_clone = my_window.clone();

                td_element.handle(move |td: &mut TextDisplay, event: Event| {
                    if event == Event::Push {
                        //TODO verify right click or left click
                        let text = td.buffer().expect("The content shall sure be existed").text();
                        match CLIPBOARD_OBSERVER.clipboard.set_text(text) {
                            Ok(_) => {
                                // CLIPBOARD_STACK.push(text);
                                paste_action();
                                my_window_clone.platform_hide();
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
            my_window.redraw();
        });
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

