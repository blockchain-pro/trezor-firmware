use super::ffi;

pub use super::ffi::{BTN_EVT_DOWN, BTN_EVT_UP, BTN_LEFT, BTN_RIGHT};

pub fn io_touch_read() -> u32 {
    unsafe { ffi::touch_read() }
}

pub fn io_button_read() -> u32 {
    unsafe { ffi::button_read() }
}
