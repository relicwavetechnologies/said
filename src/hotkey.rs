use core_foundation::runloop::kCFRunLoopCommonModes;
use core_graphics::event::CGEventFlags;
use std::sync::Arc;
use std::time::Instant;

#[allow(non_upper_case_globals)]
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

    // CGEventField for keycode
    pub const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

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

        pub fn CGEventGetFlags(event: CGEventRef) -> u64;
        pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;

        pub fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
        pub fn CFRunLoopGetCurrent() -> *mut c_void;
        pub fn CFRunLoopRun();
    }

    pub const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;
}

const FN_KEYCODE: i64 = 63; // fn/Globe key
const SHIFT_FLAG: u64 = CGEventFlags::CGEventFlagShift.bits();
const DEBOUNCE_MS: u128 = 400;

struct CallbackState {
    fn_down: bool,
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

        let keycode = ffi::CGEventGetIntegerValueField(event, ffi::K_CG_KEYBOARD_EVENT_KEYCODE);
        let flags = ffi::CGEventGetFlags(event);
        let has_shift = (flags & SHIFT_FLAG) != 0;

        if let Some(ref mut state) = CALLBACK_STATE {
            if keycode == FN_KEYCODE {
                let fn_flag_now = (flags & CGEventFlags::CGEventFlagSecondaryFn.bits()) != 0;

                if fn_flag_now {
                    // fn pressed
                    state.fn_down = true;
                } else if state.fn_down {
                    // fn released
                    state.fn_down = false;
                    if has_shift && state.last_fire.elapsed().as_millis() > DEBOUNCE_MS {
                        state.last_fire = Instant::now();
                        println!("[hotkey] 🔥 fn+Shift → toggling");
                        (state.callback)();
                    }
                }
            }
        }
    }
    event
}

pub fn start_listener(callback: Arc<dyn Fn() + Send + Sync>) {
    std::thread::spawn(move || {
        unsafe {
            CALLBACK_STATE = Some(CallbackState {
                fn_down: false,
                last_fire: Instant::now() - std::time::Duration::from_secs(10),
                callback,
            });

            let mask: u64 = 1u64 << ffi::K_CG_EVENT_FLAGS_CHANGED;

            let tap = ffi::CGEventTapCreate(
                0, // kCGHIDEventTap
                0, // kCGHeadInsertEventTap
                1, // kCGEventTapOptionListenOnly
                mask,
                tap_callback,
                std::ptr::null_mut(),
            );

            if tap.is_null() {
                eprintln!(
                    "[hotkey] ✗ failed to create event tap — grant Accessibility permission"
                );
                eprintln!("         System Settings → Privacy & Security → Accessibility");
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

            println!("[hotkey] listening for fn + Shift");
            ffi::CFRunLoopRun();
        }
    });
}
