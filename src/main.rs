use std::{env, fs, thread};
use std::fmt::Debug;
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

use arboard::Clipboard;
use evdev_rs::{Device, ReadFlag};
use evdev_rs::enums::{EV_KEY, EventCode};
use fltk::{app, prelude::*, text, window::Window};
use fltk::app::{App, event_x, event_y};
use fltk::draw::set_cursor;
use fltk::enums::{Damage, Event};
use fltk::group::Scroll;
use fltk::text::TextDisplay;
use fltk::window::DoubleWindow;
use once_cell::sync::Lazy;

use copysl::clipboard_utils::ClipboardObserver;
use copysl::stack::Stack;
use copysl::system::paste_action;

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
static APP: Lazy<App> = Lazy::new(|| { App::default().with_scheme(app::Scheme::Plastic) });


fn main() {

    //channel for listening keyboard event
    let (sender, receiver):(Sender<i32>, Receiver<i32>) = mpsc::channel();

    //TODO optimize output_dir for macOS
    let home_folder = env::var("HOME").unwrap_or_else(|err| {
        panic!("Failed to get home directory, nested error is {}", err);
    });
    let mut output_dir = home_folder.to_string() + "/.copysl/";
    if fs::metadata(&output_dir).is_err() {
        fs::create_dir(&output_dir).unwrap_or_else(|err| {
            panic!("Failed to create directory, nested error is {}", err)
        });
    }

    let mut output_dir = PathBuf::from(&output_dir);
    output_dir.push("copysl.log");
    let stdout = File::create(&output_dir);
    output_dir.pop();
    output_dir.push("copysl.err");
    let stderr = File::create(&output_dir);

    let mut pid_dir = PathBuf::new();
    pid_dir.push(home_folder);
    pid_dir.push(".copysl");
    pid_dir.push("copysl.pid");

    let daemonize = daemonize::Daemonize::new()
        .pid_file(pid_dir)
        // .chown_pid_file(true)
        // .working_directory("~/.copysl")
        // .user("nb")
        // .group("nb")
        // .umask(0o777)
        .stdout(stdout.unwrap())
        .stderr(stderr.unwrap())
        .privileged_action(|| "Executed before drop privileges");


    let (w, h): (i32, i32) = adapted_window_size_wh();
    let mut my_window: DoubleWindow = Window::new(100, 100, w, h, "copysl");
    // let mut my_window: Arc<Mutex<RefCell<DoubleWindow>>> = Arc::new(Mutex::new(RefCell::new(Window::new(100, 100, w, h, "My Window"))));
    let mut scroll = Scroll::new(0, 0, w, h + 7, None);
    my_window.set_damage(false);
    my_window.set_damage_type(Damage::None);
    scroll.set_scrollbar_size(7);

    my_window.end();
    my_window.show();




    // unsafe {
    //     unsafe extern "C" fn handle_hotkey(data: *mut std::ffi::c_void, _ev: *mut std::ffi::c_void) -> i32 {
    // 
    //         if app::is_event_ctrl() && app::event_key()==Key::from_char('v') {
    //             // Check for hotkey (e.g., Ctrl+H)
    //             println!("Hotkey pressed");
    //             1 thread::sleep(std::time::Duration::from_millis(10)); thread::sleep(std::time::Duration::from_millis(10));
    //         } else {
    //             0
    //         }
    //     }
    //     extern "C" fn system_handler(event: *mut raw::c_void, data: *mut raw::c_void) -> i32 {
    //         let event = event as i32;
    //        unsafe {
    //             if event == Event::KeyDown.bits() {
    //                 println!("Key down");
    //             }
    //         }
    //         println!("not yet pressed");
    //         if app::is_event_command() && (app::event_key()==Key::from_char('v') ||app::event_key()==Key::from_char('V')){
    //             // let mut window =data as *mut DoubleWindow;
    //             println!("pressed");
    //             0
    //         }else { 0 }
    //      
    //     }
    //     app::add_system_handler(
    //         Some(         system_handler),
    //         my_window.clone().as_widget_ptr() as *mut raw::c_void,
    //     );
    // }


    // match daemonize.start(){
    //     Ok(_) => {
    //         println!("Daemonized successfully");
    // 
    //     }
    //     Err(err) => {panic!("Failed to daemonize: {}", err)}
    // };


    thread::spawn(move || {
        let device_file = File::open("/dev/input/event9");
        if device_file.is_err() {
            panic!("Failed to open device file, nested error is {}", device_file.err().unwrap());
        }
        let mut device = Device::new_from_file(device_file.unwrap()).unwrap();
        let mut key_rec: Vec<EventCode> = vec![];
        loop {
            let (status, event) = device.next_event(ReadFlag::NORMAL).unwrap();
            let super_key = EventCode::EV_KEY(EV_KEY::KEY_LEFTMETA);
            let v_key = EventCode::EV_KEY(EV_KEY::KEY_V);
          
            if  event.event_code == super_key || event.event_code == v_key{
               
                   
                    if event.value == 1 {
                        key_rec.push(event.event_code)
                    }
                    if event.value == 0 {
                        key_rec.retain(|it| { it != &event.event_code })
                    }

                    if key_rec.contains(&super_key) && key_rec.contains(&&&v_key) {
                        println!("Paste Hotkey pressed");
                        println!("=================================================================================");

                        sender.send(1).unwrap();
                        
                        sender.send(1).unwrap();


                        println!("Key event: {:?}", event.as_raw());
                        println!("Key event: {:?}", event.event_code);
                        println!("Key event: {:?}", event.value);
                }
            }


          
        }
    });

    while APP.wait() {
        let mut visible = my_window.visible();
        'recv_loop: while !visible {
            // thread::sleep(std::time::Duration::from_millis(1000));
            let recv_result = receiver.recv();
            if recv_result.is_ok(){
                my_window.show();
                thread::sleep(std::time::Duration::from_millis(20));
                break 'recv_loop;
            }
        }
        
        app_internal_action(&w, &h, &mut my_window, &mut scroll);



    };


    println!("Exit app loop");


    println!("Exit loop");


    //TODO try to combine with the loop of hotkey
}


// fn app_main() {
//     let (w, h): (i32, i32) = adapted_window_size_wh();
//     let mut my_window: Arc<Mutex<RefCell<DoubleWindow>>> = Arc::new(Mutex::new(RefCell::new(Window::new(100, 100, w, h, "My Window"))));
//     let mut scroll = Scroll::new(0, 0, w, h + 7, None);
//     scroll.set_scrollbar_size(7);
// 
//     my_window.lock().unwrap().borrow().end();
//     my_window.lock().unwrap().borrow_mut().show();
// 
//     while APP.wait() {
//         app_internal_action(&w, &h, &mut my_window, &mut scroll);
//         my_window.lock().unwrap().borrow_mut().redraw();
//     };
// }

fn app_internal_action(w: &i32, h: &i32, my_window: &mut DoubleWindow, mut scroll: &mut Scroll) {
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


                let mut my_window_clone = my_window.clone();

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
