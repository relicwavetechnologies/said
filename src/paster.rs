use arboard::Clipboard;
use core_graphics::event::{CGEvent, CGEventFlags, CGKeyCode};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use std::thread;
use std::time::Duration;

const KEY_V: CGKeyCode = 9;

pub fn paste(text: &str) {
    let mut cb = match Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[paste] ✗ clipboard error: {e}");
            return;
        }
    };

    let original = cb.get_text().unwrap_or_default();
    let _ = cb.set_text(text.to_string());

    thread::sleep(Duration::from_millis(80));

    // Simulate Cmd+V
    if let Ok(source) = CGEventSource::new(CGEventSourceStateID::HIDSystemState) {
        if let Ok(key_down) = CGEvent::new_keyboard_event(source.clone(), KEY_V, true) {
            key_down.set_flags(CGEventFlags::CGEventFlagCommand);
            key_down.post(core_graphics::event::CGEventTapLocation::HID);
        }
        if let Ok(key_up) = CGEvent::new_keyboard_event(source, KEY_V, false) {
            key_up.set_flags(CGEventFlags::CGEventFlagCommand);
            key_up.post(core_graphics::event::CGEventTapLocation::HID);
        }
    }

    thread::sleep(Duration::from_millis(400));

    let _ = cb.set_text(original);
}
