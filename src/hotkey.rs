use core_foundation::runloop::kCFRunLoopCommonModes;
use core_graphics::event::CGEventFlags;
use std::sync::Arc;

// Raw Core Graphics FFI for event taps
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

    unsafe extern "C" {
        pub fn CGEventTapCreate(
            tap: u32,        // kCGHIDEventTap = 0
            place: u32,      // kCGHeadInsertEventTap = 0
            options: u32,    // kCGEventTapOptionListenOnly = 1
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

        pub fn CFRunLoopAddSource(
            rl: *mut c_void,
            source: *mut c_void,
            mode: *const c_void,
        );

        pub fn CFRunLoopGetCurrent() -> *mut c_void;
        pub fn CFRunLoopRun();
    }

    // Event type masks
    pub const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;
    pub const K_CG_EVENT_KEY_DOWN: u32 = 10;
    pub const K_CG_EVENT_KEY_UP: u32 = 11;
}

const FN_FLAG: u64 = CGEventFlags::CGEventFlagSecondaryFn.bits();
const SHIFT_FLAG: u64 = CGEventFlags::CGEventFlagShift.bits();

struct CallbackState {
    prev_fn: bool,
    callback: Arc<dyn Fn() + Send + Sync>,
}

static mut CALLBACK_STATE: Option<CallbackState> = None;

unsafe extern "C" fn tap_callback(
    _proxy: ffi::CGEventTapProxy,
    _event_type: u32,
    event: ffi::CGEventRef,
    _user_info: *mut std::ffi::c_void,
) -> ffi::CGEventRef {
    unsafe {
        let flags = ffi::CGEventGetFlags(event);
        let has_fn = (flags & FN_FLAG) != 0;
        let has_shift = (flags & SHIFT_FLAG) != 0;

        if let Some(ref mut state) = CALLBACK_STATE {
            if state.prev_fn && !has_fn && has_shift {
                println!("[hotkey] 🔥 fn+Shift → toggling");
                (state.callback)();
            }
            state.prev_fn = has_fn;
        }
    }
    event // pass through
}

pub fn start_listener(callback: Arc<dyn Fn() + Send + Sync>) {
    std::thread::spawn(move || {
        unsafe {
            CALLBACK_STATE = Some(CallbackState {
                prev_fn: false,
                callback,
            });

            // Event mask: FlagsChanged | KeyDown | KeyUp
            let mask: u64 = (1u64 << ffi::K_CG_EVENT_FLAGS_CHANGED)
                | (1u64 << ffi::K_CG_EVENT_KEY_DOWN)
                | (1u64 << ffi::K_CG_EVENT_KEY_UP);

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
