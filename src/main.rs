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
    // Load .env from the directory the binary is in, or current dir
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    if let Some(dir) = &exe_dir {
        let _ = dotenvy::from_path(dir.join(".env"));
    }
    let _ = dotenvy::dotenv(); // also check cwd

    let _lock = acquire_lock();
    config::validate();
    preflight();

    let mode = config::current_mode();
    println!("🎤  Voice Polish");
    println!("─────────────────────────────────────────────");
    println!("  gateway  : {}", config::GATEWAY_BASE);
    println!("  mode     : {}  →  {}", mode.key, mode.model);
    println!("  hotkey   : fn + Shift  (start / stop recording)");
    println!("  menu bar : click ● to cycle mode");
    println!("─────────────────────────────────────────────");

    let core = Arc::new(Mutex::new(app::AppCore::new()));

    // Start hotkey listener
    let core_hotkey = Arc::clone(&core);
    hotkey::start_listener(Arc::new(move || {
        core_hotkey.lock().unwrap().toggle();
    }));

    // Run menu bar on main thread (required by macOS)
    menubar::run(core);
}
