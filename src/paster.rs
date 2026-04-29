use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

// macOS key codes
const KEY_V: u16 = 9;
const KEY_CMD: u16 = 55; // kVK_Command — left Command key

const K_CG_HID_EVENT_TAP: u32 = 0;
const K_CG_FLAG_COMMAND: u64 = 1 << 20;

// kCGEventSourceStateCombinedSessionState = 1 — matches what pynput uses
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

        // AXIsProcessTrusted checks whether this process is in the
        // Accessibility list in System Settings — the right question to ask
        // before calling CGEventPost.  CGPreflightPostEventAccess() checks a
        // *different* TCC service (kTCCServicePostEvent) and returns false
        // even when the app is trusted for Accessibility.
        pub fn AXIsProcessTrusted() -> bool;

        // Shows the "allow in System Settings" prompt when not trusted.
        pub fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
    }
}

/// Call once at startup — shows the Accessibility permission prompt if needed.
pub fn request_permission() {
    unsafe {
        if ffi::AXIsProcessTrusted() {
            println!("[paste] Accessibility permission: granted ✓");
        } else {
            eprintln!("[paste] requesting Accessibility permission…");
            eprintln!("[paste]   System Settings → Privacy & Security → Accessibility");
            eprintln!("[paste]   Add VoicePolish and toggle it ON, then restart the app.");
            // Pass NULL — we'll show the dialog via System Settings guidance.
            // Passing a prompt dict here requires ObjC bridge; the eprintln
            // above is sufficient since the install script already opens the page.
            ffi::AXIsProcessTrustedWithOptions(std::ptr::null());
        }
    }
}

fn post_key(source: *mut std::ffi::c_void, keycode: u16, key_down: bool, flags: u64) {
    unsafe {
        let event = ffi::CGEventCreateKeyboardEvent(source as *const _, keycode, key_down);
        if !event.is_null() {
            ffi::CGEventSetFlags(event, flags);
            ffi::CGEventPost(K_CG_HID_EVENT_TAP, event);
            ffi::CFRelease(event);
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

pub fn is_accessibility_granted() -> bool {
    unsafe { ffi::AXIsProcessTrusted() }
}

pub fn paste(text: &str) -> Result<(), String> {
    // Always copy to clipboard first — worst case the user can Cmd+V manually.
    pbcopy(text);

    unsafe {
        if !ffi::AXIsProcessTrusted() {
            eprintln!("[paste] ⚠️  Accessibility not granted — text copied to clipboard");
            eprintln!("[paste]    Press Cmd+V to paste manually, or grant Accessibility:");
            eprintln!("[paste]    System Settings → Privacy & Security → Accessibility");
            eprintln!("[paste]    Add VoicePolish, toggle ON, then: vp stop && vp");
            eprintln!("[paste]    Binary TCC must see: {}",
                std::env::current_exe()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "<unknown>".to_string())
            );
            // Return Ok so the workflow shows ✅ (clipboard) rather than ❌ (error).
            // Auto-paste will work once Accessibility is granted and app restarts.
            return Ok(());
        }
    }

    let original = pbpaste();
    // text already in clipboard from pbcopy above
    thread::sleep(Duration::from_millis(100));

    unsafe {
        let source = ffi::CGEventSourceCreate(K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION);

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
