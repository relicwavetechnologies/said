use std::sync::atomic::{AtomicUsize, Ordering};

pub const GATEWAY_BASE: &str = "https://gateway-v21w.onrender.com";
pub const VOICE_URL: &str = "https://gateway-v21w.onrender.com/v1/voice/polish";
pub const SAMPLE_RATE: u32 = 16_000;
pub const CHANNELS: u16 = 1;
pub const MIN_DURATION_S: f32 = 0.5;

pub struct Mode {
    pub key: &'static str,
    pub label: &'static str,
    pub model: &'static str,
    pub icon: &'static str,
}

pub const MODES: &[Mode] = &[
    Mode { key: "fast",   label: "⚡  Fast   (gpt-5.4-mini)",            model: "gpt-5.4-mini",                  icon: "⚡" },
    Mode { key: "smart",  label: "🧠  Smart  (gpt-5.4)",                model: "gpt-5.4",                       icon: "🧠" },
    Mode { key: "claude", label: "🤖  Claude (claude-sonnet)",           model: "claude-sonnet-4-6",             icon: "🤖" },
    Mode { key: "gemini", label: "✨  Gemini (gemini-3.1-flash)",        model: "gemini-3.1-flash-lite-preview", icon: "✨" },
];

static MODE_INDEX: AtomicUsize = AtomicUsize::new(0);

pub fn current_mode() -> &'static Mode {
    &MODES[MODE_INDEX.load(Ordering::Relaxed)]
}

pub fn cycle_mode() -> &'static Mode {
    let next = (MODE_INDEX.load(Ordering::Relaxed) + 1) % MODES.len();
    MODE_INDEX.store(next, Ordering::Relaxed);
    &MODES[next]
}

pub fn mode_label() -> &'static str {
    current_mode().label
}

pub fn api_key() -> String {
    std::env::var("GATEWAY_API_KEY").unwrap_or_default()
}

pub fn validate() {
    let key = api_key();
    if key.is_empty() {
        eprintln!("[config] ✗ GATEWAY_API_KEY not set in .env");
        std::process::exit(1);
    }
}
