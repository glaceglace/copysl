use std::fs::File;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use evdev_rs::{Device, ReadFlag};
use evdev_rs::enums::{EV_KEY, EventCode};
use fltk::window::DoubleWindow;

pub fn observe_linux_kbd(sender: Sender<i32>) {
    thread::spawn(move || {
        let device_file = File::open("/dev/input/event5");
        if device_file.is_err() {
            panic!("Failed to open device file, nested error is {}", device_file.err().unwrap());
        }
        let device = Device::new_from_file(device_file.unwrap()).unwrap();
        let mut key_rec: Vec<EventCode> = vec![];
        loop {
            let (_, event) = device.next_event(ReadFlag::NORMAL).unwrap();
            let super_key = EventCode::EV_KEY(EV_KEY::KEY_LEFTMETA);
            let v_key = EventCode::EV_KEY(EV_KEY::KEY_V);

            if event.event_code == super_key || event.event_code == v_key {
                if event.value == 1 {
                    key_rec.push(event.event_code)
                }
                if event.value == 0 {
                    key_rec.retain(|it| { it != &event.event_code })
                }

                if key_rec.contains(&super_key) && key_rec.contains(&v_key) {
                    sender.send(1).unwrap();
                    println!("Key event: {:?}", event.as_raw());
                    println!("Key event: {:?}", event.event_code);
                    println!("Key event: {:?}", event.value);
                }
            }
        }
    });
}
pub fn wait_for_key_press(receiver: &Receiver<i32>, my_window: &mut DoubleWindow, visible: &mut bool) {
    'recv_loop: while !*visible {
        // thread::sleep(std::time::Duration::from_millis(1000));
        let recv_result = receiver.recv();
        if recv_result.is_ok() && !*visible {
            my_window.platform_show();
            thread::sleep(std::time::Duration::from_millis(20));
            // my_window.wait_for_expose(); doesn't work well to block while re-showing the window
            // my_window.wait_for_expose();
            break 'recv_loop;
        }
    }
    if *visible { let _ = receiver.try_recv(); }
}
