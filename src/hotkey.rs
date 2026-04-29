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
    // macOS sends these pseudo-types when it auto-disables an active tap
    pub const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFFFFFE;
    pub const K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT: u32 = 0xFFFFFFFF;

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
        pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);

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
// Stored so the re-enable handler inside the callback can reference it
static mut EVENT_TAP: ffi::CFMachPortRef = std::ptr::null_mut();

unsafe extern "C" fn tap_callback(
    _proxy: ffi::CGEventTapProxy,
    event_type: u32,
    event: ffi::CGEventRef,
    _user_info: *mut std::ffi::c_void,
) -> ffi::CGEventRef {
    unsafe {
        // macOS auto-disables active taps that process events too slowly.
        // Re-enable immediately so the hotkey keeps working.
        if event_type == ffi::K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT
            || event_type == ffi::K_CG_EVENT_TAP_DISABLED_BY_USER_INPUT
        {
            if !EVENT_TAP.is_null() {
                ffi::CGEventTapEnable(EVENT_TAP, true);
                eprintln!("[hotkey] ⚡ tap auto-disabled by macOS → re-enabled");
            }
            return event;
        }

        if event_type != ffi::K_CG_EVENT_FLAGS_CHANGED {
            return event;
        }

        let keycode = ffi::CGEventGetIntegerValueField(event, ffi::K_CG_KEYBOARD_EVENT_KEYCODE);

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
    event // always pass the event through — don't suppress Caps Lock
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

            // options=0 → kCGEventTapOptionDefault (active intercepting tap).
            // Active taps are more reliably delivered for Caps Lock than passive (1).
            // Requires Input Monitoring permission in System Settings.
            let tap = ffi::CGEventTapCreate(
                0, 0, 0,
                mask,
                tap_callback,
                std::ptr::null_mut(),
            );

            if tap.is_null() {
                eprintln!("[hotkey] ✗ CGEventTapCreate failed — null tap returned");
                eprintln!("         Input Monitoring not granted. Fix:");
                eprintln!("         1. System Settings → Privacy & Security → Input Monitoring");
                eprintln!("         2. Click + and add: {}", current_exe_str());
                eprintln!("         3. Toggle it ON, then run: vp stop && vp");
                return;
            }

            EVENT_TAP = tap;

            if !ffi::CGEventTapIsEnabled(tap) {
                eprintln!("[hotkey] ✗ tap created but DISABLED — Input Monitoring not granted");
                eprintln!("         Fix:");
                eprintln!("         1. System Settings → Privacy & Security → Input Monitoring");
                eprintln!("         2. Find VoicePolish (or add it), toggle ON");
                eprintln!("         3. Run: vp stop && vp");
                eprintln!("         Binary TCC must see: {}", current_exe_str());
                // Don't return — keep the run loop alive so the menu bar stays up.
                // Hotkey won't fire until the user restarts after granting permission.
            } else {
                println!("[hotkey] ✓ CGEventTap active — listening for Caps Lock");
            }

            let source = ffi::CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            if source.is_null() {
                eprintln!("[hotkey] ✗ CFMachPortCreateRunLoopSource failed");
                return;
            }

            let rl = ffi::CFRunLoopGetCurrent();
            ffi::CFRunLoopAddSource(
                rl,
                source,
                kCFRunLoopCommonModes as *const _ as *const std::ffi::c_void,
            );

            ffi::CFRunLoopRun();
        }
    });
}

fn current_exe_str() -> String {
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string())
}
