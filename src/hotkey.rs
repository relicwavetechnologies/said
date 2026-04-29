// Uses IOKit HID Manager instead of CGEventTap.
// CGEventTap misses Caps Lock on modern macOS because the OS processes it at
// driver level before any event tap sees it. IOHIDManager receives raw hardware
// events before that processing happens.

use core_foundation::runloop::kCFRunLoopCommonModes;
use std::ffi::c_void;
use std::sync::Arc;
use std::time::Instant;

mod ffi {
    use std::ffi::c_void;

    pub type IOReturn = i32;
    pub type IOOptionBits = u32;

    pub type IOHIDValueCallback = unsafe extern "C" fn(
        context: *mut c_void,
        result: IOReturn,
        sender: *mut c_void,
        value: *mut c_void,
    );

    #[link(name = "IOKit", kind = "framework")]
    unsafe extern "C" {
        pub fn IOHIDManagerCreate(
            allocator: *const c_void,
            options: IOOptionBits,
        ) -> *mut c_void;

        pub fn IOHIDManagerSetDeviceMatching(
            manager: *mut c_void,
            matching: *const c_void,
        );

        pub fn IOHIDManagerRegisterInputValueCallback(
            manager: *mut c_void,
            callback: IOHIDValueCallback,
            context: *mut c_void,
        );

        pub fn IOHIDManagerScheduleWithRunLoop(
            manager: *mut c_void,
            run_loop: *mut c_void,
            run_loop_mode: *const c_void,
        );

        pub fn IOHIDManagerOpen(
            manager: *mut c_void,
            options: IOOptionBits,
        ) -> IOReturn;

        pub fn IOHIDValueGetElement(value: *mut c_void) -> *mut c_void;
        pub fn IOHIDValueGetIntegerValue(value: *mut c_void) -> i64;
        pub fn IOHIDElementGetUsagePage(element: *mut c_void) -> u32;
        pub fn IOHIDElementGetUsage(element: *mut c_void) -> u32;
    }

    // CoreFoundation run loop (already linked via core-foundation crate)
    unsafe extern "C" {
        pub fn CFRunLoopGetCurrent() -> *mut c_void;
        pub fn CFRunLoopRun();
    }
}

// HID Usage Table: Usage Page 0x07 = Keyboard/Keypad, Usage 0x39 = Caps Lock
const HID_USAGE_PAGE_KEYBOARD: u32 = 0x07;
const HID_USAGE_CAPS_LOCK: u32 = 0x39;
const DEBOUNCE_MS: u128 = 300;

struct HotkeyState {
    last_fire: Instant,
    callback: Arc<dyn Fn() + Send + Sync>,
}

static mut HOTKEY_STATE: Option<HotkeyState> = None;

unsafe extern "C" fn hid_callback(
    _context: *mut c_void,
    _result: ffi::IOReturn,
    _sender: *mut c_void,
    value: *mut c_void,
) {
    unsafe {
        let element = ffi::IOHIDValueGetElement(value);
        if element.is_null() {
            return;
        }

        let usage_page = ffi::IOHIDElementGetUsagePage(element);
        let usage = ffi::IOHIDElementGetUsage(element);
        let int_value = ffi::IOHIDValueGetIntegerValue(value);

        // Fire on physical Caps Lock press only (int_value==1 = key down, 0 = key up)
        if usage_page == HID_USAGE_PAGE_KEYBOARD
            && usage == HID_USAGE_CAPS_LOCK
            && int_value == 1
        {
            if let Some(ref mut state) = HOTKEY_STATE {
                if state.last_fire.elapsed().as_millis() > DEBOUNCE_MS {
                    state.last_fire = Instant::now();
                    println!("[hotkey] 🔥 Caps Lock pressed → toggling");
                    (state.callback)();
                }
            }
        }
    }
}

pub fn start_listener(callback: Arc<dyn Fn() + Send + Sync>) {
    std::thread::spawn(move || {
        unsafe {
            let past = Instant::now() - std::time::Duration::from_secs(10);
            HOTKEY_STATE = Some(HotkeyState {
                last_fire: past,
                callback,
            });

            let manager = ffi::IOHIDManagerCreate(std::ptr::null(), 0);
            if manager.is_null() {
                eprintln!("[hotkey] ✗ IOHIDManagerCreate failed");
                return;
            }

            // NULL matching = observe all HID devices; we filter by usage in the callback
            ffi::IOHIDManagerSetDeviceMatching(manager, std::ptr::null());

            ffi::IOHIDManagerRegisterInputValueCallback(
                manager,
                hid_callback,
                std::ptr::null_mut(),
            );

            let rl = ffi::CFRunLoopGetCurrent();
            ffi::IOHIDManagerScheduleWithRunLoop(
                manager,
                rl,
                kCFRunLoopCommonModes as *const _ as *const c_void,
            );

            let ret = ffi::IOHIDManagerOpen(manager, 0);
            if ret != 0 {
                eprintln!("[hotkey] ✗ IOHIDManagerOpen failed (code {ret})");
                eprintln!("         → grant Input Monitoring permission:");
                eprintln!("           System Settings → Privacy & Security → Input Monitoring");
                eprintln!("           Add VoicePolish, toggle ON, then restart.");
                return;
            }

            println!("[hotkey] ✓ IOKit HID: listening for Caps Lock");
            ffi::CFRunLoopRun();
        }
    });
}
