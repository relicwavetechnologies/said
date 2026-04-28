use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

// macOS key codes
const KEY_V: u16 = 9;
const KEY_CMD: u16 = 55; // kVK_Command — left Command key

const K_CG_HID_EVENT_TAP: u32 = 0;
const K_CG_FLAG_COMMAND: u64 = 1 << 20;

// kCGEventSourceStateCombinedSessionState = 1
// This is what pynput uses — matches the current combined HID/app state.
// Using null source instead produces events the system may silently drop.
const K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION: u32 = 1;

mod ffi {
    use std::ffi::c_void;
    unsafe extern "C" {
        pub fn CGEventSourceCreate(state_id: u32) -> *mut c_void;
        pub fn CGEventCreateKeyboardEvent(
            source: *const c_void,
            keycode: u16,
            key_down: bool,
        ) -> *mut c_void;
        pub fn CGEventSetFlags(event: *mut c_void, flags: u64);
        pub fn CGEventPost(tap: u32, event: *mut c_void);
        pub fn CFRelease(cf: *mut c_void);
        pub fn CGPreflightPostEventAccess() -> bool;
        pub fn CGRequestPostEventAccess() -> bool;
    }
}

fn post_key(source: *mut std::ffi::c_void, keycode: u16, key_down: bool, flags: u64) {
    unsafe {
        let event =
            ffi::CGEventCreateKeyboardEvent(source as *const _, keycode, key_down);
        if !event.is_null() {
            ffi::CGEventSetFlags(event, flags);
            ffi::CGEventPost(K_CG_HID_EVENT_TAP, event);
            ffi::CFRelease(event);
        }
    }
}

/// Call once at startup — triggers the macOS Accessibility permission dialog.
pub fn request_permission() {
    unsafe {
        if !ffi::CGPreflightPostEventAccess() {
            eprintln!("[paste] requesting Accessibility permission…");
            eprintln!("[paste]   If no dialog appears, open:");
            eprintln!("[paste]   System Settings → Privacy & Security → Accessibility");
            eprintln!("[paste]   and add voice-polish manually.");
            ffi::CGRequestPostEventAccess();
        } else {
            println!("[paste] Accessibility permission: granted ✓");
        }
    }
}

fn pbcopy(text: &str) {
    if let Ok(mut child) = Command::new("pbcopy")
        .stdin(std::process::Stdio::piped())
        .spawn()
    {
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(text.as_bytes());
        }
        let _ = child.wait();
    }
}

fn pbpaste() -> String {
    Command::new("pbpaste")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default()
}

pub fn paste(text: &str) -> Result<(), String> {
    unsafe {
        if !ffi::CGPreflightPostEventAccess() {
            return Err(
                "Accessibility permission not granted — open System Settings → Privacy & Security → Accessibility and add voice-polish".into()
            );
        }
    }

    let original = pbpaste();
    pbcopy(text);
    thread::sleep(Duration::from_millis(100));

    unsafe {
        // Create an event source matching the combined HID+session state —
        // exactly what pynput does.  A null source produces events the OS
        // may silently discard on macOS 12+.
        let source = ffi::CGEventSourceCreate(K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION);

        // Exactly what pynput does:
        // press Cmd, press V (with Cmd flag), release V, release Cmd
        post_key(source, KEY_CMD, true, 0);
        thread::sleep(Duration::from_millis(10));

        post_key(source, KEY_V, true, K_CG_FLAG_COMMAND);
        thread::sleep(Duration::from_millis(10));

        post_key(source, KEY_V, false, K_CG_FLAG_COMMAND);
        thread::sleep(Duration::from_millis(10));

        post_key(source, KEY_CMD, false, 0);

        if !source.is_null() {
            ffi::CFRelease(source);
        }
    }

    thread::sleep(Duration::from_millis(500));
    pbcopy(&original);
    Ok(())
}
