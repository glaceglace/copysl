use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;
use fltk::prelude::WindowExt;
use fltk::window::DoubleWindow;
use once_cell::unsync::Lazy;
use uinput::Device;
use uinput::event::keyboard;

static mut DEVICE: Lazy<Device> = Lazy::new(|| {
    uinput::default().unwrap()
        .name("test").unwrap()
        .event(uinput::event::Keyboard::All).unwrap()
        .create()
        .unwrap()
});

pub fn paste_action(
    // rc_window:&Rc<RefCell<DoubleWindow>>
) {
//TODO borrow window from main
//     let mut ref_window = rc_window.try_borrow_mut();
//     if ref_window.is_err(){
//         println!("Failed to borrow window when doing paste action");
//         return;
//     }
    // let mut window = ref_window.unwrap();

    unsafe {
        println!("+++++{:#?}", DEVICE.type_id());
            println!("===========================================");
            println!("+++++----{:#?}", DEVICE.type_id());
            // window.iconize();
            DEVICE.press(&keyboard::Key::LeftAlt).unwrap();
            DEVICE.press(&keyboard::Key::Tab).unwrap();
        DEVICE.synchronize().unwrap();
            DEVICE.release(&keyboard::Key::LeftAlt).unwrap();
            DEVICE.release(&keyboard::Key::Tab).unwrap();
        DEVICE.synchronize().unwrap();
            thread::sleep(std::time::Duration::from_millis(50));
            DEVICE.press(&keyboard::Key::LeftControl).unwrap();
            DEVICE.press(&keyboard::Key::V).unwrap();
        DEVICE.synchronize().unwrap();
        println!("+++++++++++++++++++++++++++++++++++++++++++");
            DEVICE.release(&keyboard::Key::LeftControl).unwrap();
            DEVICE.release(&keyboard::Key::V).unwrap();
            DEVICE.synchronize().unwrap();

    }
}
