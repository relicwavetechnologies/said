#![allow(deprecated)]

mod api;
mod app;
mod config;
mod hotkey;
mod menubar;
mod paster;
mod recorder;

use std::fs::File;
use std::sync::{Arc, Mutex};

fn acquire_lock() -> Option<File> {
    let path = std::env::temp_dir().join("voice-polish.lock");
    let file = File::create(&path).ok()?;
    use std::os::unix::io::AsRawFd;
    let fd = file.as_raw_fd();
    let ret = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
    if ret != 0 {
        eprintln!("[voice] already running — only one instance allowed. Exiting.");
        std::process::exit(0);
    }
    Some(file)
}

fn preflight() {
    match recorder::AudioRecorder::preflight() {
        Ok(name) => println!("[preflight] mic: {name}"),
        Err(e) => {
            eprintln!("[preflight] ✗ {e}");
            std::process::exit(1);
        }
    }
}

fn main() {
    // 1. Exe dir — works for local dev (./target/release/voice-polish)
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    if let Some(dir) = &exe_dir {
        let _ = dotenvy::from_path(dir.join(".env"));
    }
    // 2. ~/VoicePolish/.env — works when binary lives inside .app bundle
    if std::env::var("GATEWAY_API_KEY").is_err() {
        if let Ok(home) = std::env::var("HOME") {
            let fallback = std::path::Path::new(&home).join("VoicePolish").join(".env");
            let _ = dotenvy::from_path(fallback);
        }
    }
    // 3. CWD fallback
    let _ = dotenvy::dotenv();

    let _lock = acquire_lock();
    config::validate();
    preflight();
    paster::request_permission(); // shows Accessibility prompt on first run

    let exe_path = std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());

    let ax_ok = paster::is_accessibility_granted();

    let mode = config::current_mode();
    println!("🎤  Voice Polish");
    println!("─────────────────────────────────────────────");
    println!("  binary   : {exe_path}");
    println!("  gateway  : {}", config::GATEWAY_BASE);
    println!("  mode     : {}  →  {}", mode.key, mode.model);
    println!("  hotkey   : Caps Lock  (start / stop recording)");
    println!("  paste    : {}", if ax_ok { "auto (Accessibility ✓)" } else { "clipboard only — grant Accessibility then restart" });
    println!("  menu bar : click ● to cycle mode");
    println!("─────────────────────────────────────────────");
    if !ax_ok {
        eprintln!("[startup] ⚠️  Accessibility NOT granted — paste will copy to clipboard only");
        eprintln!("[startup]    System Settings → Privacy & Security → Accessibility");
        eprintln!("[startup]    Add the binary shown above, toggle ON, then: vp stop && vp");
    }

    let core = Arc::new(Mutex::new(app::AppCore::new()));

    // Start hotkey listener
    let core_hotkey = Arc::clone(&core);
    hotkey::start_listener(Arc::new(move || {
        core_hotkey.lock().unwrap().toggle();
    }));

    // Run menu bar on main thread (required by macOS)
    menubar::run(core);
}
