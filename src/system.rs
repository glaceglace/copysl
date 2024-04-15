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

pub fn paste_action() {
    unsafe {
        DEVICE.press(&keyboard::Key::LeftMeta).unwrap();
        DEVICE.press(&keyboard::Key::L).unwrap();
        DEVICE.release(&keyboard::Key::LeftMeta).unwrap();
        DEVICE.release(&keyboard::Key::L).unwrap();


        DEVICE.synchronize().unwrap();
    }
}