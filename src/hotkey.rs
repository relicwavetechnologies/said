use core_foundation::runloop::kCFRunLoopCommonModes;
use std::sync::Arc;
use std::time::Instant;

mod ffi {
    use std::ffi::c_void;

    pub type CGEventRef = *mut c_void;
    pub type CGEventTapProxy = *mut c_void;
    pub type CFMachPortRef = *mut c_void;

    pub type CGEventTapCallBack = unsafe extern "C" fn(
        proxy: CGEventTapProxy,
        event_type: u32,
        event: CGEventRef,
        user_info: *mut c_void,
    ) -> CGEventRef;

    pub const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
    pub const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;

    unsafe extern "C" {
        pub fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: CGEventTapCallBack,
            user_info: *mut c_void,
        ) -> CFMachPortRef;

        pub fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            port: CFMachPortRef,
            order: i64,
        ) -> *mut c_void;

        pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
        pub fn CGEventTapIsEnabled(tap: CFMachPortRef) -> bool;

        pub fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
        pub fn CFRunLoopGetCurrent() -> *mut c_void;
        pub fn CFRunLoopRun();
    }
}

const CAPS_LOCK_KEYCODE: i64 = 57;
const DEBOUNCE_MS: u128 = 300;

struct CallbackState {
    last_fire: Instant,
    callback: Arc<dyn Fn() + Send + Sync>,
}

static mut CALLBACK_STATE: Option<CallbackState> = None;

unsafe extern "C" fn tap_callback(
    _proxy: ffi::CGEventTapProxy,
    event_type: u32,
    event: ffi::CGEventRef,
    _user_info: *mut std::ffi::c_void,
) -> ffi::CGEventRef {
    unsafe {
        if event_type != ffi::K_CG_EVENT_FLAGS_CHANGED {
            return event;
        }

        let keycode =
            ffi::CGEventGetIntegerValueField(event, ffi::K_CG_KEYBOARD_EVENT_KEYCODE);

        if keycode == CAPS_LOCK_KEYCODE {
            if let Some(ref mut state) = CALLBACK_STATE {
                if state.last_fire.elapsed().as_millis() > DEBOUNCE_MS {
                    state.last_fire = Instant::now();
                    println!("[hotkey] 🔥 Caps Lock → toggling");
                    (state.callback)();
                }
            }
        }
    }
    event
}

pub fn start_listener(callback: Arc<dyn Fn() + Send + Sync>) {
    std::thread::spawn(move || {
        unsafe {
            let past = Instant::now() - std::time::Duration::from_secs(10);
            CALLBACK_STATE = Some(CallbackState {
                last_fire: past,
                callback,
            });

            let mask: u64 = 1u64 << ffi::K_CG_EVENT_FLAGS_CHANGED;

            let tap = ffi::CGEventTapCreate(
                0, 0, 1,
                mask,
                tap_callback,
                std::ptr::null_mut(),
            );

            if tap.is_null() {
                eprintln!("[hotkey] ✗ failed to create event tap");
                eprintln!("         → grant Input Monitoring permission:");
                eprintln!("           System Settings → Privacy & Security → Input Monitoring");
                eprintln!("           Add voice-polish and toggle it ON, then restart.");
                return;
            }

            let source = ffi::CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            if source.is_null() {
                eprintln!("[hotkey] ✗ failed to create run loop source");
                return;
            }

            let rl = ffi::CFRunLoopGetCurrent();
            ffi::CFRunLoopAddSource(
                rl,
                source,
                kCFRunLoopCommonModes as *const _ as *const std::ffi::c_void,
            );

            if !ffi::CGEventTapIsEnabled(tap) {
                eprintln!("[hotkey] ✗ event tap created but DISABLED — no Input Monitoring permission");
                eprintln!("         → System Settings → Privacy & Security → Input Monitoring");
                eprintln!("           Add voice-polish and toggle it ON, then restart.");
                return;
            }

            println!("[hotkey] ✓ listening for Caps Lock");
            ffi::CFRunLoopRun();
        }
    });
}
