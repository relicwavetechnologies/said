#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod backend;
mod desktop;
mod dg_stream; // P5: Deepgram WebSocket live streaming

use std::sync::{Arc, Mutex};

use tauri::{
    Emitter, Manager, State,
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    tray::TrayIconBuilder,
};

use backend::BackendEndpoint;
use desktop::DesktopApp;
use voice_polish_core::{AppSnapshot, ProcessSummary};
use voice_polish_paster as paster;

#[cfg(target_os = "macos")]
use voice_polish_hotkey as hotkey;

// ── Keystroke reconstruction (edit detection for AX-blind apps) ──────────────
//
// The existing CGEventTap in the hotkey crate is extended to also capture
// kCGEventKeyDown events into a rolling buffer.  watch_for_edit notes
// Instant::now() before watching, then replays all buffered keystrokes
// timestamped AFTER that instant against the known pasted text.
//
// This is the same technique Wispr Flow uses — no second CGEventTap needed.

/// Apply buffered keystrokes to reconstruct the final text in an AX-blind app.
///
/// `initial` is the text we pasted.  Events are filtered to those that arrived
/// after `since`.  Returns `None` only if reconstruction is truly unreliable
/// Show a macOS native notification.
///
/// Two-path notifier.
///
/// The macOS notification banner shows the icon of the *process that posted
/// it*, which is why we have two paths:
///
/// 1. **Production (.app bundle):** use `tauri-plugin-notification`. The
///    plugin posts via the app's own bundle, so the banner shows the Said
///    icon (icon.icns from tauri.conf.json's `bundle.icon`). This is the
///    only path that gets the brand on the banner.
///
/// 2. **Dev (raw debug binary):** the plugin silently no-ops because
///    `mac-notification-sys` requires a registered bundle identifier.
///    Fall back to `osascript`, which always shows a banner but uses the
///    Script Editor icon (we can't override it via AppleScript). At least
///    the user sees *that* something happened in dev.
///
/// We detect "running inside a .app" by inspecting the executable path
/// (`*.app/Contents/MacOS/*`). Cheap, no syscalls, no environment sniffing.
#[cfg(target_os = "macos")]
fn notify_macos(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;

    if is_bundled_app() {
        match app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
        {
            Ok(_) => {
                tracing::info!("[notify] plugin sent (Said icon): {title}");
                return;
            }
            Err(e) => {
                tracing::warn!("[notify] plugin failed: {e} — falling back to osascript");
            }
        }
    }
    osa_fallback(title, body);
}

#[cfg(target_os = "macos")]
fn is_bundled_app() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.to_string()))
        .map(|s| s.contains(".app/Contents/MacOS/"))
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn osa_fallback(title: &str, body: &str) {
    use std::process::{Command, Stdio};
    // AppleScript string literals: backslash-escape `\` and `"`.
    let title_esc = title.replace('\\', "\\\\").replace('"', "\\\"");
    let body_esc  = body .replace('\\', "\\\\").replace('"', "\\\"");
    let script = format!(
        r#"display notification "{body_esc}" with title "{title_esc}""#
    );
    match Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(_)  => tracing::info!("[notify] osa sent (no icon): {title}"),
        Err(e) => tracing::warn!("[notify] osascript spawn failed: {e}"),
    }
}

#[cfg(not(target_os = "macos"))]
fn notify_macos(_app: &tauri::AppHandle, _title: &str, _body: &str) {}

/// Translate a raw pipeline error string into one short human sentence.
///
/// We never want to surface diagnostic text like "Deepgram error 400:
/// invalid audio format" or "empty transcript — nothing spoken?" to the
/// user — those read like log lines.  This function maps the common cases
/// to plain English and falls back to a generic apology for everything
/// else.  Keep wording calm and blame-free; no "Error:" prefix, no codes,
/// no emoji.
fn humanize_error(raw: &str) -> String {
    let lower = raw.to_lowercase();

    // STT-side cases
    if lower.contains("empty transcript") || lower.contains("nothing spoken") {
        return "We couldn't hear anything in that recording. Try again.".to_string();
    }
    if lower.contains("deepgram")
        && (lower.contains("401") || lower.contains("403") || lower.contains("unauthorized"))
    {
        return "Speech recognition is offline because the API key isn't accepted. Check your Deepgram key in Settings.".to_string();
    }
    if lower.contains("deepgram") && (lower.contains("429") || lower.contains("rate")) {
        return "Speech recognition is rate-limited right now. Give it a few seconds and try again.".to_string();
    }
    if lower.contains("deepgram") || lower.contains("stt") {
        return "Speech recognition couldn't process that recording. Try again in a moment."
            .to_string();
    }

    // LLM-side cases
    if lower.contains("openai")
        && (lower.contains("not connected") || lower.contains("401") || lower.contains("403"))
    {
        return "OpenAI isn't connected. Open Settings to sign in again.".to_string();
    }
    if lower.contains("groq") && (lower.contains("401") || lower.contains("403")) {
        return "Groq isn't accepting the API key. Check it in Settings.".to_string();
    }
    if lower.contains("gemini") && (lower.contains("401") || lower.contains("403")) {
        return "Gemini isn't accepting the API key. Check it in Settings.".to_string();
    }
    if lower.contains("rate") || lower.contains("429") {
        return "The polish service is rate-limited right now. Try again in a moment.".to_string();
    }
    if lower.contains("preferences not found") {
        return "Settings aren't loaded yet. Wait a moment and try again.".to_string();
    }

    // Network / transport
    if lower.contains("timeout") || lower.contains("timed out") {
        return "The connection took too long. Check your internet and try again.".to_string();
    }
    if lower.contains("dns") || lower.contains("unreachable") || lower.contains("failed to connect")
    {
        return "Couldn't reach the network. Check your internet and try again.".to_string();
    }

    // Generic fallback — never expose raw error text.
    "Something went wrong with that recording. Please try again.".to_string()
}

/// (Cmd+Z, Cmd+X).  Mouse clicks are handled by trying every possible cursor
/// position and picking the candidate that preserves the most surrounding text
/// (i.e., the smallest local edit).
#[cfg(target_os = "macos")]
fn reconstruct_from_keystrokes(initial: &str, since: std::time::Instant) -> Option<String> {
    use hotkey::KeyEvt;

    let buf = hotkey::key_buffer();
    let guard = buf.lock().ok()?;

    let events: Vec<KeyEvt> = guard
        .iter()
        .filter(|t| t.when >= since)
        .map(|t| t.evt.clone())
        .collect();

    if events.is_empty() {
        return None;
    }

    let clicks = events
        .iter()
        .filter(|e| matches!(e, KeyEvt::MouseClick))
        .count();
    let chars = events
        .iter()
        .filter(|e| matches!(e, KeyEvt::Char(_)))
        .count();
    let bksp = events
        .iter()
        .filter(|e| matches!(e, KeyEvt::Backspace))
        .count();
    tracing::debug!(
        "[keystroke] replaying {} events ({chars} chars, {bksp} backspaces, {clicks} clicks) against {} char initial",
        events.len(),
        initial.len(),
    );

    let text: Vec<char> = initial.chars().collect();
    let cursor = text.len(); // cursor starts at end after paste

    replay_events(initial, &text, cursor, &events, false, 0)
}

/// Replay a sequence of key events starting from `text` with `cursor`.
///
/// When a `MouseClick` is encountered the cursor position becomes unknown.
/// We try every possible position (0..=text.len()), recurse on the remaining
/// events, and pick the candidate whose result is closest to `original` —
/// measured by longest preserved prefix + suffix (smallest contiguous edit).
///
/// `depth` limits recursion for multiple successive mouse clicks (cap at 3).
#[cfg(target_os = "macos")]
fn replay_events(
    original: &str,
    text: &[char],
    cursor: usize,
    events: &[hotkey::KeyEvt],
    all_sel: bool,
    depth: u8,
) -> Option<String> {
    use hotkey::KeyEvt;

    let mut text = text.to_vec();
    let mut cursor = cursor;
    let mut all_selected = all_sel;

    for (i, evt) in events.iter().enumerate() {
        match evt {
            KeyEvt::Char(c) => {
                if all_selected {
                    text.clear();
                    cursor = 0;
                    all_selected = false;
                }
                text.insert(cursor, *c);
                cursor += 1;
            }
            KeyEvt::Backspace => {
                all_selected = false;
                if cursor > 0 {
                    cursor -= 1;
                    text.remove(cursor);
                }
            }
            KeyEvt::Delete => {
                all_selected = false;
                if cursor < text.len() {
                    text.remove(cursor);
                }
            }
            KeyEvt::Left => {
                all_selected = false;
                if cursor > 0 {
                    cursor -= 1;
                }
            }
            KeyEvt::Right => {
                all_selected = false;
                if cursor < text.len() {
                    cursor += 1;
                }
            }
            KeyEvt::Home => {
                all_selected = false;
                cursor = 0;
            }
            KeyEvt::End => {
                all_selected = false;
                cursor = text.len();
            }
            // Option+arrows: word-granularity movement
            KeyEvt::WordLeft => {
                all_selected = false;
                cursor = word_start_before(&text, cursor);
            }
            KeyEvt::WordRight => {
                all_selected = false;
                cursor = word_end_after(&text, cursor);
            }
            // Cmd+arrows: line-granularity movement
            KeyEvt::LineStart => {
                all_selected = false;
                cursor = line_start_before(&text, cursor);
            }
            KeyEvt::LineEnd => {
                all_selected = false;
                cursor = line_end_after(&text, cursor);
            }
            // Option+Backspace / Cmd+Backspace: multi-char deletes
            KeyEvt::WordBackspace => {
                all_selected = false;
                delete_word_before(&mut text, &mut cursor);
            }
            KeyEvt::LineBackspace => {
                all_selected = false;
                delete_line_before(&mut text, &mut cursor);
            }
            KeyEvt::SelectAll => {
                all_selected = true;
            }
            KeyEvt::MouseClick => {
                if depth >= 3 {
                    return None;
                } // too many nested clicks — give up
                all_selected = false;

                let remaining = &events[i + 1..];
                let mut best: Option<String> = None;
                let mut best_score: usize = 0;

                for p in 0..=text.len() {
                    if let Some(candidate) =
                        replay_events(original, &text, p, remaining, false, depth + 1)
                    {
                        let score = preserved_text_score(original, &candidate);
                        if best.is_none() || score > best_score {
                            best_score = score;
                            best = Some(candidate);
                        }
                    }
                }

                if depth == 0 {
                    tracing::debug!(
                        "[keystroke] MouseClick at event {i}, tried {} positions, best score={best_score}",
                        text.len() + 1,
                    );
                }

                return best;
            }
            KeyEvt::Cut | KeyEvt::Undo => return None,
            KeyEvt::Other => {}
        }
    }

    Some(text.iter().collect())
}

// ── Text cursor movement helpers (approximate macOS semantics) ────────────────

/// Option+Left: move cursor to the start of the previous word.
/// A "word" is a maximal sequence of alphanumeric+apostrophe characters.
/// macOS also skips punctuation clusters first, matching `moveWordBackward:`.
#[cfg(target_os = "macos")]
fn word_start_before(text: &[char], pos: usize) -> usize {
    let mut i = pos;
    // 1. Skip non-word chars (spaces, punctuation) immediately before cursor
    while i > 0 && !text[i - 1].is_alphanumeric() && text[i - 1] != '\'' {
        i -= 1;
    }
    // 2. Skip the word chars
    while i > 0 && (text[i - 1].is_alphanumeric() || text[i - 1] == '\'') {
        i -= 1;
    }
    i
}

/// Option+Right: move cursor to the end of the next word.
#[cfg(target_os = "macos")]
fn word_end_after(text: &[char], pos: usize) -> usize {
    let mut i = pos;
    // 1. Skip non-word chars immediately after cursor
    while i < text.len() && !text[i].is_alphanumeric() && text[i] != '\'' {
        i += 1;
    }
    // 2. Skip the word chars
    while i < text.len() && (text[i].is_alphanumeric() || text[i] == '\'') {
        i += 1;
    }
    i
}

/// Cmd+Left / Home: move cursor to the start of the current line.
#[cfg(target_os = "macos")]
fn line_start_before(text: &[char], pos: usize) -> usize {
    let mut i = pos;
    while i > 0 && text[i - 1] != '\n' {
        i -= 1;
    }
    i
}

/// Cmd+Right / End: move cursor to the end of the current line.
#[cfg(target_os = "macos")]
fn line_end_after(text: &[char], pos: usize) -> usize {
    let mut i = pos;
    while i < text.len() && text[i] != '\n' {
        i += 1;
    }
    i
}

/// Option+Backspace: delete from cursor back to the start of the previous word.
#[cfg(target_os = "macos")]
fn delete_word_before(text: &mut Vec<char>, cursor: &mut usize) {
    let target = word_start_before(text, *cursor);
    text.drain(target..*cursor);
    *cursor = target;
}

/// Cmd+Backspace: delete from cursor back to the start of the current line.
#[cfg(target_os = "macos")]
fn delete_line_before(text: &mut Vec<char>, cursor: &mut usize) {
    let target = line_start_before(text, *cursor);
    text.drain(target..*cursor);
    *cursor = target;
}

/// How much of `original` is preserved in `candidate` at the start and end.
/// Higher = smaller contiguous edit = more likely the correct reconstruction.
#[cfg(target_os = "macos")]
fn preserved_text_score(original: &str, candidate: &str) -> usize {
    let orig: Vec<char> = original.chars().collect();
    let cand: Vec<char> = candidate.chars().collect();

    // Longest common prefix
    let prefix = orig
        .iter()
        .zip(cand.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // Longest common suffix (not overlapping with the prefix)
    let max_suffix = orig
        .len()
        .saturating_sub(prefix)
        .min(cand.len().saturating_sub(prefix));
    let suffix = orig
        .iter()
        .rev()
        .zip(cand.iter().rev())
        .take(max_suffix)
        .take_while(|(a, b)| a == b)
        .count();

    prefix + suffix
}

// ── Managed state ─────────────────────────────────────────────────────────────

/// Holds the local recording state machine.
struct SharedApp(Arc<Mutex<DesktopApp>>);

/// Holds the backend endpoint (url + secret). None until daemon starts.
struct BackendState(Arc<Mutex<Option<BackendEndpoint>>>);

/// Owns the BackendHandle (and its Child) for the lifetime of the app.
/// When Tauri drops managed state on exit, Drop fires → SIGTERM → SIGKILL.
struct BackendHandleState(Mutex<Option<backend::BackendHandle>>);

/// P5: Holds the oneshot receiver that delivers the pre-transcript from the
/// Deepgram WebSocket streaming task.  Replaced on every new recording.
struct StreamingState(Mutex<Option<tokio::sync::oneshot::Receiver<String>>>);

/// Stores the most-recently polished text. Populated after every voice/text polish;
/// cleared after it's pasted via Ctrl+Cmd+V or the `paste_latest` Tauri command.
struct LatestResult(std::sync::Arc<Mutex<Option<String>>>);

/// Hot-path cache: language setting + personal vocabulary keyterms.
///
/// Populated once when the backend becomes ready; refreshed in the background
/// whenever preferences change or new vocabulary is learned.  The WS recording
/// task reads from this cache — zero HTTP calls on the recording critical path.
struct HotPathCache(Arc<tokio::sync::RwLock<HotPathCacheInner>>);

#[derive(Default, Clone)]
struct HotPathCacheInner {
    /// User's STT language setting (e.g. "hi", "multi", "auto").
    language: String,
    /// Personal vocabulary terms sent to Deepgram as `keyterm=` biases.
    keyterms: Vec<String>,
}

/// Lightweight cache of tray-relevant prefs so `sync_tray` never needs async.
struct TrayCache(Mutex<TrayCacheInner>);
struct TrayCacheInner {
    custom_prompt: Option<String>,
    output_language: String, // "hinglish" | "english" | "hindi"
}
impl Default for TrayCacheInner {
    fn default() -> Self {
        Self {
            custom_prompt: None,
            output_language: "hinglish".into(),
        }
    }
}

// ── Tray helpers ──────────────────────────────────────────────────────────────

/// Short status text that appears next to the brand icon in the menu bar.
/// Empty when idle (icon alone).
fn tray_title(state: &str) -> &'static str {
    match state {
        "recording" => " ● REC",
        "processing" => " …",
        _ => "",
    }
}

/// Build the dynamic tray menu.
/// Re-run on every state change so recording label and language checkmarks stay in sync.
fn build_tray_menu(
    app: &tauri::AppHandle,
    snap: &AppSnapshot,
    custom_prompt: Option<&str>,
    output_language: &str,
) -> Result<Menu<tauri::Wry>, tauri::Error> {
    // ── 1. Toggle recording (state-aware label + enabled) ──────────────
    let toggle_label = match snap.state.as_str() {
        "recording" => "Stop recording",
        "processing" => "Processing…",
        _ => "Start recording",
    };
    let toggle_enabled = snap.state.as_str() != "processing";
    let toggle = MenuItem::with_id(
        app,
        "tray_toggle",
        toggle_label,
        toggle_enabled,
        None::<&str>,
    )?;

    // ── 2. Output language submenu ─────────────────────────────────────
    let mk_lang =
        |id: &str, label: &str, active: bool| -> Result<MenuItem<tauri::Wry>, tauri::Error> {
            let prefix = if active { "✓  " } else { "    " };
            MenuItem::with_id(app, id, format!("{prefix}{label}"), true, None::<&str>)
        };
    let lang_hinglish = mk_lang(
        "tray_lang_hinglish",
        "Hinglish → Hinglish",
        output_language == "hinglish",
    )?;
    let lang_english = mk_lang(
        "tray_lang_english",
        "Hindi → Polished English",
        output_language == "english",
    )?;
    let lang_hindi = mk_lang(
        "tray_lang_hindi",
        "Hindi → Hindi (Devanagari)",
        output_language == "hindi",
    )?;
    let lang_submenu = Submenu::with_items(
        app,
        "Output Language",
        true,
        &[
            &lang_hinglish as &dyn tauri::menu::IsMenuItem<tauri::Wry>,
            &lang_english,
            &lang_hindi,
        ],
    )?;

    // ── 4. "Polish my message" submenu ─────────────────────────────────
    // Shortcut hints: Option+1..5 (global hotkeys registered in setup).
    let p_prof = MenuItem::with_id(
        app,
        "tray_polish_professional",
        "Professional English  ⌥1",
        true,
        None::<&str>,
    )?;
    let p_casual = MenuItem::with_id(app, "tray_polish_casual", "Casual  ⌥2", true, None::<&str>)?;
    let p_concise = MenuItem::with_id(
        app,
        "tray_polish_concise",
        "Concise  ⌥3",
        true,
        None::<&str>,
    )?;
    let p_hinglish = MenuItem::with_id(
        app,
        "tray_polish_hinglish",
        "Hinglish  ⌥4",
        true,
        None::<&str>,
    )?;
    let p_assertive = MenuItem::with_id(
        app,
        "tray_polish_assertive",
        "Assertive",
        true,
        None::<&str>,
    )?;
    let p_neutral = MenuItem::with_id(app, "tray_polish_neutral", "Neutral", true, None::<&str>)?;

    let polish_refs: Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> = {
        let mut v: Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> = vec![
            Box::new(p_prof),
            Box::new(p_casual),
            Box::new(p_concise),
            Box::new(p_hinglish),
            Box::new(p_assertive),
            Box::new(p_neutral),
        ];
        // Add "Custom  ⌥5" only when the user has set a custom prompt in Settings
        if custom_prompt.map(|s| !s.trim().is_empty()).unwrap_or(false) {
            let p_custom =
                MenuItem::with_id(app, "tray_polish_custom", "Custom  ⌥5", true, None::<&str>)?;
            v.push(Box::new(p_custom));
        }
        v
    };
    let polish_item_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> =
        polish_refs.iter().map(|b| b.as_ref()).collect();
    let polish_submenu = Submenu::with_items(app, "Polish my message", true, &polish_item_refs)?;

    // ── 4. Window actions + quit ────────────────────────────────────────
    let show_item = MenuItem::with_id(app, "show", "Open Said", true, None::<&str>)?;
    let settings_item = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let reconnect_item =
        MenuItem::with_id(app, "reconnect", "Reconnect OpenAI…", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Said", true, None::<&str>)?;

    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let sep4 = PredefinedMenuItem::separator(app)?;

    Menu::with_items(
        app,
        &[
            &toggle as &dyn tauri::menu::IsMenuItem<tauri::Wry>,
            &sep1,
            &lang_submenu,
            &sep2,
            &polish_submenu,
            &sep3,
            &show_item,
            &settings_item,
            &reconnect_item,
            &sep4,
            &quit_item,
        ],
    )
}

/// Re-render the tray icon title + menu from the cached prefs (no async needed).
fn sync_tray(handle: &tauri::AppHandle, snap: &AppSnapshot) {
    if let Some(tray) = handle.tray_by_id("said") {
        let _ = tray.set_title(Some(tray_title(&snap.state)));

        // Read from in-process cache — never blocks on async or HTTP
        let cache = handle.state::<TrayCache>();
        let inner = cache.0.lock().unwrap_or_else(|p| p.into_inner());
        let custom = inner.custom_prompt.clone();
        let lang = inner.output_language.clone();
        drop(inner);

        if let Ok(menu) = build_tray_menu(handle, snap, custom.as_deref(), &lang) {
            let _ = tray.set_menu(Some(menu));
        }
    }
}

// ── Floating status bar ───────────────────────────────────────────────────────

/// Create the always-on-top floating status pill.
///
/// The window loads the same SPA with `#statusbar` in the hash so `main.tsx`
/// renders `<StatusBar />` instead of the full app.  It starts hidden; the
/// React component calls `appWindow.show()` / `hide()` as the pipeline runs.
fn create_status_bar(app: &tauri::AppHandle) {
    // Position: bottom-center, 90 px above the dock
    let (x, y) = if let Ok(Some(m)) = app.primary_monitor() {
        let sf = m.scale_factor();
        let sw = m.size().width  as f64 / sf;
        let sh = m.size().height as f64 / sf;
        let mx = m.position().x as f64 / sf;
        let my = m.position().y as f64 / sf;
        (mx + sw / 2.0 - 160.0, my + sh - 56.0 - 90.0)
    } else {
        (560.0, 860.0)
    };

    match tauri::WebviewWindowBuilder::new(
        app,
        "status-bar",
        tauri::WebviewUrl::App("index.html#statusbar".into()),
    )
    .title("Said")
    .inner_size(320.0, 56.0)
    .position(x, y)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .visible(false)
    .build()
    {
        Ok(_)  => tracing::info!("[status-bar] window created at ({x:.0},{y:.0})"),
        Err(e) => tracing::warn!("[status-bar] could not create window: {e}"),
    }
}

// ── Tray action helpers ───────────────────────────────────────────────────────

/// Trigger recording from a tray menu click.
/// Mirrors the `toggle_recording` Tauri command's logic.
fn tray_toggle_recording(app: &tauri::AppHandle) {
    let shared_state = app.state::<SharedApp>();
    let backend_state = app.state::<BackendState>();

    let current = match shared_state.0.lock() {
        Ok(g) => g.state,
        Err(_) => return,
    };

    match current {
        desktop::AppState::Idle => {
            do_start_recording(&shared_state.0, app);
        }
        desktop::AppState::Recording => {
            do_finish_recording(
                Arc::clone(&shared_state.0),
                app.clone(),
                Arc::clone(&backend_state.0),
            );
        }
        desktop::AppState::Processing => {} // ignore — already in flight
    }
}

/// Polish the currently selected text using the given tone preset.
///
/// Flow: read selection → POST /v1/text/polish (SSE) with tone_override → paste result.
fn tray_polish_message(app: &tauri::AppHandle, tone: &str) {
    let backend = app.state::<BackendState>();
    let ep_opt = backend.0.lock().ok().and_then(|g| g.clone());
    let ep = match ep_opt {
        Some(e) => e,
        None => {
            tracing::warn!("[tray_polish] backend not ready");
            return;
        }
    };

    // Read the selected text.  This is called from a spawned thread (not the
    // CGEventTap thread) so the Cmd+C fallback can work.
    tracing::info!("[tray_polish] reading selected text for tone={tone}...");
    let selected = paster::read_selected_text();
    let text = match selected {
        Some(t) if !t.trim().is_empty() => {
            tracing::info!("[tray_polish] got {} chars of selected text", t.len());
            t
        }
        _ => {
            tracing::warn!(
                "[tray_polish] no text selected — make sure text is highlighted before pressing Option+N"
            );
            return;
        }
    };

    let tone_owned = tone.to_string();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        tracing::info!(
            "[tray_polish] polishing {} chars with tone={}",
            text.len(),
            tone_owned
        );

        let result = api::stream_text_polish(
            &ep,
            text,
            None,
            Some(tone_owned),
            |_event| {}, // fire-and-forget on events; we paste the final result
        )
        .await;

        match result {
            Ok(done) if !done.polished.is_empty() => {
                tracing::info!(
                    "[tray_polish] done — {} words",
                    done.polished.split_whitespace().count()
                );
                // Emit tokens to the UI for live preview if the window is visible
                let _ = app_clone.emit("voice-done", &done);
                // Paste the polished text (replaces selection)
                if let Err(e) = paster::paste(&done.polished) {
                    tracing::warn!("[tray_polish] paste failed: {e}");
                }
            }
            Ok(_) => tracing::warn!("[tray_polish] empty result from backend"),
            Err(e) => tracing::warn!("[tray_polish] backend error: {e}"),
        }
    });
}

/// Show the main window and emit a hint to switch to the settings view.
fn tray_open_settings(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.set_focus();
    }
    let _ = app.emit("nav-settings", ());
}

/// Switch output language from the tray menu and persist to SQLite.
fn tray_set_output_language(app: &tauri::AppHandle, lang: &str) {
    // Update cache immediately so sync_tray shows the new checkmark
    if let Ok(mut cache) = app.state::<TrayCache>().0.lock() {
        cache.output_language = lang.to_string();
    }
    // Re-render tray with new checkmark
    let shared = app.state::<SharedApp>();
    if let Ok(d) = shared.0.lock() {
        let snap = d.snapshot();
        drop(d);
        sync_tray(app, &snap);
    }
    // Persist to backend (fire-and-forget)
    let backend = app.state::<BackendState>();
    let ep_opt = backend.0.lock().ok().and_then(|g| g.clone());
    let lang_own = lang.to_string();
    let app_h = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(ep) = ep_opt {
            let _ = api::patch_preferences(
                &ep,
                api::PrefsUpdate {
                    output_language: Some(lang_own),
                    ..Default::default()
                },
            )
            .await;
            // Tell the frontend to refresh its prefs so the settings page stays in sync
            let _ = app_h.emit("prefs-changed", ());
        }
    });
}

/// Initiate OpenAI OAuth from the tray menu — opens the system browser and
/// emits an event so the frontend can start polling for the connected state.
fn tray_reconnect_openai(app: &tauri::AppHandle) {
    let backend = app.state::<BackendState>();
    let ep_opt = backend.0.lock().ok().and_then(|g| g.clone());
    let ep = match ep_opt {
        Some(e) => e,
        None => {
            tracing::warn!("[tray_reconnect] backend not ready");
            return;
        }
    };
    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        match api::initiate_openai_oauth(&ep).await {
            Ok(result) => {
                if let Some(url) = result.get("auth_url").and_then(|v| v.as_str()) {
                    let _ = open::that(url);
                }
                // Tell the frontend to start polling — it will show the reconnect
                // state in the Settings view and update openAIConnected once done.
                let _ = app_clone.emit("openai-reconnect-initiated", ());
            }
            Err(e) => tracing::warn!("[tray_reconnect] failed to initiate OAuth: {e}"),
        }
    });
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
fn bootstrap(state: State<'_, SharedApp>, app: tauri::AppHandle) -> Result<AppSnapshot, String> {
    let snap = state.0.lock().map_err(|_| "lock failed")?.snapshot();
    sync_tray(&app, &snap);
    Ok(snap)
}

#[tauri::command]
fn get_snapshot(state: State<'_, SharedApp>) -> Result<AppSnapshot, String> {
    Ok(state.0.lock().map_err(|_| "lock failed")?.snapshot())
}

/// Return `{url, secret}` so the frontend can hit the backend directly.
#[tauri::command]
fn get_backend_endpoint(backend: State<'_, BackendState>) -> Result<serde_json::Value, String> {
    let lock = backend.0.lock().map_err(|_| "lock failed")?;
    let ep = lock.as_ref().ok_or("backend not yet started")?;
    Ok(serde_json::json!({ "url": ep.url, "secret": ep.secret }))
}

#[tauri::command]
async fn get_preferences(backend: State<'_, BackendState>) -> Result<api::Preferences, String> {
    let ep = get_endpoint(&backend)?;
    api::get_preferences(&ep).await
}

#[tauri::command]
async fn patch_preferences(
    backend: State<'_, BackendState>,
    tray_cache: State<'_, TrayCache>,
    hot_cache: State<'_, HotPathCache>,
    app: tauri::AppHandle,
    update: api::PrefsUpdate,
) -> Result<api::Preferences, String> {
    tracing::info!(
        "[patch_prefs] Tauri received: llm_provider={:?} selected_model={:?} tone={:?}",
        update.llm_provider,
        update.selected_model,
        update.tone_preset
    );
    let ep = get_endpoint(&backend)?;
    let result = api::patch_preferences(&ep, update).await;
    match &result {
        Ok(p) => {
            tracing::info!(
                "[patch_prefs] backend returned: llm_provider={:?}",
                p.llm_provider
            );
            // Keep tray cache in sync so sync_tray never needs async
            if let Ok(mut cache) = tray_cache.0.lock() {
                cache.custom_prompt = p.custom_prompt.clone();
                cache.output_language = p.output_language.clone();
            }
            // Re-render tray menu to show updated checkmark
            let shared = app.state::<SharedApp>();
            if let Ok(d) = shared.0.lock() {
                let snap = d.snapshot();
                drop(d);
                sync_tray(&app, &snap);
            }
            // Keep hot-path cache language in sync — no HTTP needed next recording.
            hot_cache.0.write().await.language = p.language.clone();
        }
        Err(e) => tracing::warn!("[patch_prefs] backend error: {e}"),
    }
    result
}

#[tauri::command]
async fn get_history(
    backend: State<'_, BackendState>,
    limit: Option<i64>,
) -> Result<Vec<api::Recording>, String> {
    let ep = get_endpoint(&backend)?;
    api::get_history(&ep, limit.unwrap_or(50)).await
}

#[tauri::command]
async fn submit_edit_feedback(
    backend: State<'_, BackendState>,
    hot_cache: State<'_, HotPathCache>,
    recording_id: String,
    user_kept: String,
    target_app: Option<String>,
) -> Result<(), String> {
    let ep = get_endpoint(&backend)?;
    let result = api::submit_feedback(&ep, &recording_id, &user_kept, target_app.as_deref()).await;
    // After feedback the backend may have learned new vocabulary — refresh
    // keyterms in the background so the next recording already has them.
    if result.is_ok() {
        let arc = Arc::clone(&hot_cache.0);
        let ep2 = ep.clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(terms) = api::get_vocabulary_terms(&ep2).await {
                tracing::debug!(
                    "[hot_cache] refreshed keyterms after feedback ({} terms)",
                    terms.len()
                );
                arc.write().await.keyterms = terms;
            }
        });
    }
    result
}

#[tauri::command]
fn set_mode(
    _key: String,
    state: State<'_, SharedApp>,
    app: tauri::AppHandle,
) -> Result<AppSnapshot, String> {
    // Model switching removed — always uses gpt-5.4-mini.
    let snap = state.0.lock().map_err(|_| "lock failed")?.snapshot();
    sync_tray(&app, &snap);
    Ok(snap)
}

#[tauri::command]
fn request_accessibility(state: State<'_, SharedApp>) -> Result<AppSnapshot, String> {
    paster::request_permission();
    Ok(state.0.lock().map_err(|_| "lock failed")?.snapshot())
}

#[tauri::command]
fn request_input_monitoring(state: State<'_, SharedApp>) -> Result<AppSnapshot, String> {
    paster::request_input_monitoring();
    Ok(state.0.lock().map_err(|_| "lock failed")?.snapshot())
}

/// Run the 5-method AX field reading diagnostic on whatever is focused right now.
/// The Tauri app already has Accessibility permission, so unlike a fresh standalone
/// binary, this can always reach the focused application.
///
/// `delay_secs` is how long to wait before sampling — gives the user time to
/// click into the target app before the diagnostic runs.
#[tauri::command]
async fn diagnose_ax(delay_secs: u64) -> Result<paster::AxDiagnostics, String> {
    let delay = delay_secs.clamp(0, 30);
    if delay > 0 {
        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
    }
    // Run the (synchronous, FFI-heavy) diagnostic on a blocking thread.
    let report = tokio::task::spawn_blocking(paster::diagnose_focused_field)
        .await
        .map_err(|e| format!("diagnostic task failed: {e}"))?;
    Ok(report)
}

/// UI button: start or stop recording depending on current state.
/// - idle      → start recording, return snapshot with state="recording"
/// - recording → stop recording, kick off async SSE pipeline, return state="processing"
/// - processing → no-op (return current snapshot)
#[tauri::command]
fn toggle_recording(
    state: State<'_, SharedApp>,
    backend: State<'_, BackendState>,
    app: tauri::AppHandle,
) -> Result<AppSnapshot, String> {
    let current_state = state.0.lock().map_err(|_| "lock failed")?.state;

    match current_state {
        desktop::AppState::Idle => {
            // Pre-unlock the focused app's AX tree before recording begins.
            #[cfg(target_os = "macos")]
            {
                let pid = paster::unlock_focused_app_now();
                tracing::debug!("[record] pre-unlocked AX for focused app pid={pid:?}");
            }
            // Start recording and return immediately
            let snap = state
                .0
                .lock()
                .map_err(|_| "lock failed")?
                .start_recording()?;
            sync_tray(&app, &snap);
            Ok(snap)
        }
        desktop::AppState::Recording => {
            // Extract wav bytes synchronously, then hand off the async SSE pipeline
            let wav = state
                .0
                .lock()
                .map_err(|_| "lock failed")?
                .stop_and_extract()?;
            let snap = state.0.lock().map_err(|_| "lock failed")?.snapshot();
            sync_tray(&app, &snap);

            // Kick off the SSE pipeline in the background (same as hotkey release)
            let shared2 = Arc::clone(&state.0);
            let app2 = app.clone();
            let back_arc2 = Arc::clone(&backend.0);
            // UI button path: no WS streaming pre-transcript (hotkey path handles it)
            let pre_tx_ui: Option<String> = None;
            tauri::async_runtime::spawn(async move {
                let result = run_voice_polish_sse(&back_arc2, wav, None, pre_tx_ui, &app2).await;

                // Spawn edit-watcher immediately after paste (non-blocking).
                // Capture watch_start NOW — before the spawn — so the ring
                // buffer timestamp filter doesn't miss early mouse clicks.
                let watch_start = std::time::Instant::now();
                if let Ok(ref done) = result {
                    let back3 = Arc::clone(&back_arc2);
                    tauri::async_runtime::spawn(watch_for_edit(
                        back3,
                        app2.clone(),
                        done.recording_id.clone(),
                        done.polished.clone(),
                        watch_start,
                    ));
                }

                let mut d = match shared2.lock() {
                    Ok(g) => g,
                    Err(_) => return,
                };
                let finished_snap = match result {
                    Ok(done) => d.finish_ok(voice_polish_core::ProcessSummary {
                        transcript: done.polished.clone(),
                        polished: done.polished,
                        model: done.model_used,
                        confidence: done.confidence.unwrap_or(0.0),
                        transcribe_ms: done.latency_ms.transcribe as u64,
                        polish_ms: done.latency_ms.polish as u64,
                    }),
                    Err(e) => d.finish_err(e),
                };
                sync_tray(&app2, &finished_snap);
                let _ = app2.emit("app-state", &finished_snap);
            });

            Ok(snap) // Return "processing" snapshot to the UI immediately
        }
        desktop::AppState::Processing => {
            // Already in flight — return current snapshot, don't do anything
            Ok(state.0.lock().map_err(|_| "lock failed")?.snapshot())
        }
    }
}

// ── Recording flow ────────────────────────────────────────────────────────────

/// Start recording. Called when user presses Caps Lock (or taps the button).
fn do_start_recording(shared: &Arc<Mutex<DesktopApp>>, app: &tauri::AppHandle) {
    // Pre-unlock the focused app's AX tree BEFORE recording begins.
    // Chrome / Electron need ~150-200 ms to build their accessibility cache after
    // AXEnhancedUserInterface / AXManualAccessibility is set.  By unlocking here
    // we give the browser the full dictation window (typically 2-10 s) to get
    // ready, so that post-paste edit detection can read AXValue reliably.
    #[cfg(target_os = "macos")]
    {
        let pid = paster::unlock_focused_app_now();
        tracing::debug!("[record] pre-unlocked AX for focused app pid={pid:?}");
    }

    let snap = shared
        .lock()
        .ok()
        .and_then(|mut d| d.start_recording().ok());
    if let Some(snap) = snap {
        sync_tray(app, &snap);
        let _ = app.emit("app-state", &snap);
    }

    // ── P5: Start Deepgram WS streaming immediately ────────────────────────────
    // Take the chunk receiver from the recorder and open a WebSocket to Deepgram.
    // The transcript will be ready (or close to it) by the time Caps Lock is released.
    let chunk_recv = shared.lock().ok().and_then(|mut d| d.take_chunk_receiver());
    if let Some(chunk_recv) = chunk_recv {
        let streaming_state = app.state::<StreamingState>();
        let (transcript_tx, transcript_rx) = tokio::sync::oneshot::channel::<String>();
        // Use ok() — a poisoned mutex (from a previous panic) must not cascade
        if let Some(mut g) = streaming_state.0.lock().ok() {
            *g = Some(transcript_rx);
        }

        let hot_cache_arc = Arc::clone(&app.state::<HotPathCache>().0);
        let backend_for_pe = Arc::clone(&app.state::<BackendState>().0);

        tauri::async_runtime::spawn(async move {
            // ── P5 hot path: zero HTTP calls before WS connect ─────────────────
            //
            // API key: read from env (instant).
            // Language + keyterms: read from HotPathCache (in-memory RwLock,
            // populated at startup and kept fresh by patch_preferences /
            // submit_edit_feedback).  No network round-trips on this path.
            let deepgram_key = std::env::var("DEEPGRAM_API_KEY").unwrap_or_default();
            if deepgram_key.is_empty() {
                tracing::warn!("[dg_stream] DEEPGRAM_API_KEY not set — WS streaming disabled");
                let _ = transcript_tx.send(String::new());
                return;
            }

            let (language, keyterms) = {
                let c = hot_cache_arc.read().await;
                (c.language.clone(), c.keyterms.clone())
            };
            if !keyterms.is_empty() {
                tracing::info!(
                    "[dg_stream] biasing WS with {} cached term(s)",
                    keyterms.len()
                );
            }

            // Build pre-embed URL from backend endpoint (fire-and-forget on CloseStream)
            let pre_embed_info: Option<(String, String)> =
                backend_for_pe.lock().ok().and_then(|g| {
                    g.as_ref()
                        .map(|ep| (format!("{}/v1/pre-embed", ep.url), ep.secret.clone()))
                });

            let pre_embed_ref = pre_embed_info
                .as_ref()
                .map(|(url, secret)| (url.as_str(), secret.as_str()));

            let transcript = dg_stream::stream_to_deepgram(
                chunk_recv,
                &deepgram_key,
                &language,
                &keyterms,
                pre_embed_ref,
            )
            .await;
            tracing::info!(
                "[dg_stream] pre-transcript result: {}",
                transcript.as_deref().unwrap_or("<none>")
            );
            let _ = transcript_tx.send(transcript.unwrap_or_default());
        });
    } else {
        tracing::debug!("[dg_stream] no chunk receiver — WS streaming not started");
    }
}

/// Stop recording, ship WAV to backend via SSE, paste the result.
fn do_finish_recording(
    shared: Arc<Mutex<DesktopApp>>,
    app: tauri::AppHandle,
    back_arc: Arc<Mutex<Option<BackendEndpoint>>>,
) {
    // Extract wav bytes synchronously (near-instant, no I/O).
    // This also drops the recorder's chunk_tx, signalling the WS task to close.
    let wav = {
        let mut d = match shared.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        if let Some(t) = app.tray_by_id("said") {
            let _ = t.set_title(Some("[  …  ]  Said"));
        }
        match d.stop_and_extract() {
            Ok(w) => w,
            Err(e) => {
                let snap = d.finish_err(e);
                let _ = app.emit("app-state", &snap);
                return;
            }
        }
    };

    // ── P5: Take the transcript receiver before spawning the async task ────────
    // Use ok() so a poisoned mutex from a previous panic doesn't cascade-crash.
    let transcript_rx = app
        .state::<StreamingState>()
        .0
        .lock()
        .ok()
        .and_then(|mut g| g.take());

    // Do the async SSE pipeline in a tokio task
    let shared2 = Arc::clone(&shared);
    let app2 = app.clone();
    let back_arc2 = Arc::clone(&back_arc);

    tauri::async_runtime::spawn(async move {
        // ── P5: Wait up to 2 s for the Deepgram WS transcript ─────────────────
        // stop_and_extract() dropped chunk_tx, which closes the audio channel and
        // triggers CloseStream inside the WS task.  Deepgram usually finalises in
        // 100–200 ms, so the transcript should arrive quickly.
        // Wait up to 4 s for the Deepgram WS transcript.
        // In practice the WS path takes ~1.5-2 s from Caps Lock release to final
        // transcript (CloseStream + Deepgram finalize + 300ms endpointing window).
        // If it still doesn't arrive, we fall through to the normal HTTP STT path.
        // Estimate recording duration from WAV size:
        // 16kHz × 16-bit × mono = 32,000 bytes/sec, plus 44 byte WAV header
        let wav_duration_s = (wav.len().saturating_sub(44)) as f64 / 32_000.0;

        let pre_transcript: Option<String> = if let Some(rx) = transcript_rx {
            match tokio::time::timeout(std::time::Duration::from_secs(4), rx).await {
                Ok(Ok(t)) if !t.is_empty() => {
                    // Quality gate: reject suspiciously short transcripts.
                    // Typical Hindi/English speech: ~2 words/second.
                    // If we get fewer than 1 word per 2 seconds of audio,
                    // the WS likely returned a partial — fall back to HTTP STT.
                    let word_count = t.split_whitespace().count();
                    let expected_min_words = (wav_duration_s / 2.0).max(1.0) as usize;
                    if word_count < expected_min_words && wav_duration_s > 3.0 {
                        tracing::warn!(
                            "[finish] WS transcript too short: {} words for {:.1}s recording (expected ≥{}) — falling back to HTTP STT. transcript={t:?}",
                            word_count,
                            wav_duration_s,
                            expected_min_words
                        );
                        None
                    } else {
                        tracing::info!(
                            "[finish] ✓ WS pre-transcript ready ({} chars, {} words, {:.1}s audio): {t:?}",
                            t.len(),
                            word_count,
                            wav_duration_s
                        );
                        Some(t)
                    }
                }
                Ok(_) => {
                    tracing::info!("[finish] WS transcript empty — falling back to HTTP STT");
                    None
                }
                Err(_) => {
                    tracing::warn!(
                        "[finish] WS transcript timed out after 4 s — falling back to HTTP STT"
                    );
                    None
                }
            }
        } else {
            None
        };

        let result = run_voice_polish_sse(&back_arc2, wav, None, pre_transcript, &app2).await;

        // Spawn edit-watcher immediately after paste (non-blocking).
        // Capture watch_start NOW — before the spawn — so the ring
        // buffer timestamp filter doesn't miss early mouse clicks.
        let watch_start = std::time::Instant::now();
        if let Ok(ref done) = result {
            let back3 = Arc::clone(&back_arc2);
            tauri::async_runtime::spawn(watch_for_edit(
                back3,
                app2.clone(),
                done.recording_id.clone(),
                done.polished.clone(),
                watch_start,
            ));
        }

        let mut d = match shared2.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let snap = match result {
            Ok(done) => d.finish_ok(ProcessSummary {
                transcript: done.polished.clone(),
                polished: done.polished,
                model: done.model_used,
                confidence: done.confidence.unwrap_or(0.0),
                transcribe_ms: done.latency_ms.transcribe as u64,
                polish_ms: done.latency_ms.polish as u64,
            }),
            Err(e) => d.finish_err(e),
        };
        sync_tray(&app2, &snap);
        let _ = app2.emit("app-state", &snap);
    });
}

/// Async SSE consumer: streams tokens from backend, types them word-by-word,
/// and stores the result for Ctrl+Cmd+V re-paste.
async fn run_voice_polish_sse(
    back_arc: &Arc<Mutex<Option<BackendEndpoint>>>,
    wav: Vec<u8>,
    target_app: Option<String>,
    pre_transcript: Option<String>,
    app: &tauri::AppHandle,
) -> Result<api::PolishDone, String> {
    let ep = {
        let lock = back_arc.lock().map_err(|_| "backend lock failed")?;
        lock.clone().ok_or("backend not started")?
    };

    let app_clone = app.clone();

    // Track whether word-by-word AX typing succeeded
    let typed_any = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let typed_any2 = typed_any.clone();
    let token_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let token_count2 = token_count.clone();
    let fail_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let fail_count2 = fail_count.clone();

    tracing::info!(
        "[pipeline] → sending to backend: wav={}KB pre_transcript={}",
        wav.len() / 1024,
        pre_transcript
            .as_ref()
            .map(|t| {
                let truncated: String = t.chars().take(80).collect();
                if truncated.len() < t.len() {
                    format!("\"{truncated}…\"")
                } else {
                    format!("\"{t}\"")
                }
            })
            .unwrap_or_else(|| "none (will use HTTP STT)".into()),
    );

    let done = api::stream_voice_polish(&ep, wav, target_app, pre_transcript, move |event| {
        match &event {
            api::PolishEvent::Token { token } => {
                // Emit to UI for live preview
                let _ = app_clone.emit("voice-token", serde_json::json!({ "token": token }));
                // Type word-by-word directly into focused app via AX
                match paster::type_text(token) {
                    Ok(true) => {
                        let prev = typed_any2.swap(true, std::sync::atomic::Ordering::Relaxed);
                        let n = token_count2.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        if !prev {
                            tracing::info!("[main] GAP-2: word-by-word typing started — first token {:?}", token);
                        }
                        let _ = n;
                    }
                    Ok(false) => {
                        fail_count2.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                    Err(e) => {
                        fail_count2.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        tracing::warn!("[main] type_text error: {e}");
                    }
                }
            }
            api::PolishEvent::Status { phase, transcript } => {
                tracing::info!("[pipeline] status: phase={phase} transcript={transcript:?}");
                let _ = app_clone.emit("voice-status", serde_json::json!({ "phase": phase, "transcript": transcript }));
            }
            api::PolishEvent::Done(done) => {
                tracing::info!(
                    "[pipeline] ✓ done: {} chars, model={}, latency: stt={}ms embed={}ms polish={}ms total={}ms",
                    done.polished.len(),
                    done.model_used,
                    done.latency_ms.transcribe,
                    done.latency_ms.embed,
                    done.latency_ms.polish,
                    done.latency_ms.total,
                );
                let _ = app_clone.emit("voice-done", done);
            }
            api::PolishEvent::Error { message, audio_id } => {
                let human = humanize_error(&message);
                let _ = app_clone.emit("voice-error", serde_json::json!({
                    "message":  human.clone(),
                    "audio_id": audio_id,
                }));
                // Native macOS banner — informational only.  In dev mode the
                // osascript path can't attach action buttons (those require a
                // bundled .app with a registered UNNotificationCategory), so
                // the banner simply tells the user something went wrong and
                // they switch to Said when ready.  The in-app toast carries
                // the actual History / Retry buttons.
                //
                // No auto-focus, no dock bounce — per user preference.  The
                // banner is the only attention signal; the user controls when
                // to bring Said forward.
                notify_macos(
                    &app_clone,
                    "Said couldn't finish that recording",
                    &format!("{human}  Open Said to retry or view history."),
                );
            }
        }
    })
    .await?;

    let n_typed = token_count.load(std::sync::atomic::Ordering::Relaxed);
    let n_failed = fail_count.load(std::sync::atomic::Ordering::Relaxed);
    if typed_any.load(std::sync::atomic::Ordering::Relaxed) {
        if n_failed > 0 {
            // Some tokens typed, some failed — AX partially worked (user switched app?).
            // Do a full clipboard paste to ensure completeness.
            tracing::warn!(
                "[main] word-by-word partial: {n_typed} ok, {n_failed} failed — clipboard paste for safety"
            );
            if !done.polished.is_empty() {
                // Select-all and replace to avoid duplicating the partial text
                if let Err(e) = paster::paste(&done.polished) {
                    tracing::warn!("[main] safety paste failed: {e}");
                }
            }
        } else {
            tracing::info!("[main] word-by-word complete — {n_typed} token(s) typed directly");
        }
    } else {
        // AX not available at all — fall back to clipboard paste
        tracing::info!(
            "[main] AX not granted — falling back to clipboard paste ({} chars)",
            done.polished.len()
        );
        if !done.polished.is_empty() {
            if let Err(e) = paster::paste(&done.polished) {
                tracing::warn!("[main] paste fallback failed: {e}");
            }
        }
    }

    // Always store latest result so Ctrl+Cmd+V can re-paste it any time
    if !done.polished.is_empty() {
        if let Ok(mut g) = app.state::<LatestResult>().0.lock() {
            *g = Some(done.polished.clone());
        }
        tracing::info!(
            "[main] result stored ({} chars) — Ctrl+Cmd+V to paste again",
            done.polished.len()
        );
    }

    Ok(done)
}

/// Paste the most-recently stored polished result into the focused app.
/// Invoked by the Ctrl+Cmd+V hotkey and by the UI's "Paste latest" button.
#[tauri::command]
fn paste_latest(latest: State<'_, LatestResult>) -> Result<bool, String> {
    let text = {
        let g = latest.0.lock().map_err(|_| "lock failed")?;
        g.clone()
    };
    match text {
        None => {
            tracing::info!("[paste_latest] nothing stored yet");
            Ok(false)
        }
        Some(t) => {
            tracing::info!("[paste_latest] pasting {} chars", t.len());
            paster::paste(&t).map_err(|e| format!("paste failed: {e}"))?;
            Ok(true)
        }
    }
}

/// Delete a recording from the backend (SQLite + WAV file).
#[tauri::command]
async fn delete_recording(backend: State<'_, BackendState>, id: String) -> Result<(), String> {
    let ep = get_endpoint(&backend)?;
    api::delete_recording(&ep, &id).await
}

/// Return the bearer-authed URL to stream a recording's WAV audio.
/// The frontend fetches this URL with the Authorization header to get a blob.
#[tauri::command]
fn get_recording_audio_url(
    backend: State<'_, BackendState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let ep = get_endpoint(&backend)?;
    let url = api::recording_audio_url(&ep, &id);
    let secret = ep.secret.clone();
    Ok(serde_json::json!({ "url": url, "secret": secret }))
}

/// Return WAV bytes for in-app playback without requiring the webview to make
/// an authenticated localhost fetch.
#[tauri::command]
async fn get_recording_audio_bytes(
    backend: State<'_, BackendState>,
    id: String,
) -> Result<Vec<u8>, String> {
    let ep = get_endpoint(&backend)?;
    api::recording_audio_bytes(&ep, &id).await
}

fn safe_download_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "said-recording.wav".to_string()
    } else if trimmed.to_lowercase().ends_with(".wav") {
        trimmed
    } else {
        format!("{trimmed}.wav")
    }
}

fn unique_download_path(dir: &std::path::Path, filename: &str) -> std::path::PathBuf {
    let initial = dir.join(filename);
    if !initial.exists() {
        return initial;
    }

    let path = std::path::Path::new(filename);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("said-recording");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("wav");
    for n in 2..1000 {
        let candidate = dir.join(format!("{stem}-{n}.{ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem}-copy.{ext}"))
}

/// Save a recording WAV to ~/Downloads via native filesystem IO. This avoids
/// WKWebView blob-anchor download behavior, which is unreliable in packaged
/// desktop apps.
#[tauri::command]
async fn download_recording_audio(
    backend: State<'_, BackendState>,
    id: String,
    filename: String,
) -> Result<String, String> {
    let ep = get_endpoint(&backend)?;
    let bytes = api::recording_audio_bytes(&ep, &id).await?;
    let dir = dirs::download_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "Downloads folder not found".to_string())?;

    std::fs::create_dir_all(&dir).map_err(|e| format!("couldn't create Downloads folder: {e}"))?;

    let filename = safe_download_filename(&filename);
    let path = unique_download_path(&dir, &filename);
    std::fs::write(&path, bytes).map_err(|e| format!("couldn't save audio: {e}"))?;

    Ok(path.display().to_string())
}

/// Retry a failed recording by re-submitting its saved WAV file.
/// `audio_id` is the UUID that the backend included in the `voice-error` event.
#[tauri::command]
fn retry_recording(
    audio_id: String,
    state: State<'_, SharedApp>,
    backend: State<'_, BackendState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Read WAV from the saved file
    let audio_dir = {
        let base = dirs::data_local_dir()
            .or_else(|| dirs::home_dir().map(|h| h.join(".local/share")))
            .unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        base.join("VoicePolish").join("audio")
    };
    let wav_path = audio_dir.join(format!("{audio_id}.wav"));
    let wav = std::fs::read(&wav_path).map_err(|e| format!("saved audio not found: {e}"))?;

    // Mark as processing so the UI shows a spinner
    {
        let mut d = state.0.lock().map_err(|_| "lock failed")?;
        if d.state != desktop::AppState::Idle {
            return Err("busy — wait for current operation to finish".into());
        }
        d.state = desktop::AppState::Processing;
    }

    let shared2 = Arc::clone(&state.0);
    let app2 = app.clone();
    let back_arc2 = Arc::clone(&backend.0);

    tauri::async_runtime::spawn(async move {
        let result = run_voice_polish_sse(&back_arc2, wav, None, None, &app2).await;

        let watch_start = std::time::Instant::now();
        if let Ok(ref done) = result {
            let back3 = Arc::clone(&back_arc2);
            tauri::async_runtime::spawn(watch_for_edit(
                back3,
                app2.clone(),
                done.recording_id.clone(),
                done.polished.clone(),
                watch_start,
            ));
        }

        let mut d = match shared2.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let snap = match result {
            Ok(done) => d.finish_ok(ProcessSummary {
                transcript: done.polished.clone(),
                polished: done.polished,
                model: done.model_used,
                confidence: done.confidence.unwrap_or(0.0),
                transcribe_ms: done.latency_ms.transcribe as u64,
                polish_ms: done.latency_ms.polish as u64,
            }),
            Err(e) => d.finish_err(e),
        };
        sync_tray(&app2, &snap);
        let _ = app2.emit("app-state", &snap);
    });

    Ok(())
}

// ── Pending-edit review commands ──────────────────────────────────────────────

#[tauri::command]
async fn get_pending_edits(
    backend: State<'_, BackendState>,
) -> Result<api::PendingEditsResponse, String> {
    let ep = get_endpoint(&backend)?;
    api::get_pending_edits(&ep).await
}

#[tauri::command]
async fn resolve_pending_edit(
    backend: State<'_, BackendState>,
    hot_cache: State<'_, HotPathCache>,
    id: String,
    action: String,
) -> Result<(), String> {
    let ep = get_endpoint(&backend)?;
    let result = api::resolve_pending_edit(&ep, &id, &action).await;
    // "approve" promotes a term into vocabulary — refresh cache immediately.
    if result.is_ok() && action == "approve" {
        let arc = Arc::clone(&hot_cache.0);
        let ep2 = ep.clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(terms) = api::get_vocabulary_terms(&ep2).await {
                tracing::info!(
                    "[hot_cache] refreshed after pending-edit approval ({} terms)",
                    terms.len()
                );
                arc.write().await.keyterms = terms;
            }
        });
    }
    result
}

// ── Vocabulary management commands ────────────────────────────────────────────

#[tauri::command]
async fn list_vocabulary(
    backend: State<'_, BackendState>,
) -> Result<api::VocabListResponse, String> {
    let ep = get_endpoint(&backend)?;
    api::list_vocabulary(&ep).await
}

#[tauri::command]
async fn add_vocabulary_term(
    app: tauri::AppHandle,
    backend: State<'_, BackendState>,
    term: String,
) -> Result<(), String> {
    let ep = get_endpoint(&backend)?;
    api::add_vocabulary_term(&ep, &term).await?;
    let _ = app.emit("vocabulary-changed", ());

    // In-app toast (matches the website's design language) — primary surface.
    let _ = app.emit(
        "vocab-toast",
        serde_json::json!({
            "kind": "added", "term": term, "source": "manual",
        }),
    );

    // OS-level fallback for when the Said window isn't focused.
    notify_macos(
        &app,
        "Added to vocabulary",
        &format!("Said will recognise \"{term}\" on your next recording."),
    );
    Ok(())
}

#[tauri::command]
async fn delete_vocabulary_term(
    app: tauri::AppHandle,
    backend: State<'_, BackendState>,
    term: String,
) -> Result<(), String> {
    let ep = get_endpoint(&backend)?;
    api::delete_vocabulary_term(&ep, &term).await?;
    let _ = app.emit("vocabulary-changed", ());
    let _ = app.emit(
        "vocab-toast",
        serde_json::json!({
            "kind": "removed", "term": term,
        }),
    );
    Ok(())
}

#[tauri::command]
async fn star_vocabulary_term(
    app: tauri::AppHandle,
    backend: State<'_, BackendState>,
    term: String,
) -> Result<bool, String> {
    let ep = get_endpoint(&backend)?;
    let starred = api::star_vocabulary_term(&ep, &term).await?;
    let _ = app.emit("vocabulary-changed", ());

    // Lightweight confirmation toast for star/unstar — only on STAR (positive
    // affirmation), not on unstar (silent).
    if starred {
        let _ = app.emit(
            "vocab-toast",
            serde_json::json!({
                "kind": "starred", "term": term,
            }),
        );
        notify_macos(
            &app,
            "Pinned to vocabulary",
            &format!("Said will keep \"{term}\" even if you stop using it."),
        );
    }
    Ok(starred)
}

// ── Invite-a-friend ───────────────────────────────────────────────────────────

/// Outcome of an invite send attempt — lets the frontend either celebrate
/// the server-side send or seamlessly fall back to opening the user's mail app.
#[derive(serde::Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum InviteOutcome {
    /// Email was sent server-side via Resend.
    Sent,
    /// Server has no email provider configured (RESEND_API_KEY missing).
    /// Frontend should open `mailto:` so the user can still send via their
    /// own mail client.
    FallbackMailto,
}

#[tauri::command]
async fn send_invite_email(
    backend: State<'_, BackendState>,
    to: String,
) -> Result<InviteOutcome, String> {
    let ep = get_endpoint(&backend)?;
    match api::send_invite_email(&ep, &to).await {
        Ok(_) => Ok(InviteOutcome::Sent),
        Err(e) if e == "email_not_configured" => Ok(InviteOutcome::FallbackMailto),
        Err(e) => Err(e),
    }
}

// ── External URL opener ───────────────────────────────────────────────────────

/// Open a URL (https://, mailto:, etc.) in the user's default app.
///
/// Tauri's webview blocks `window.open("mailto:…")` silently — calls fall
/// through to the browser's noop handler, so the user sees nothing happen.
/// This command shells out to the OS opener instead.
#[tauri::command]
fn open_external(url: String) -> Result<(), String> {
    use std::process::Command;

    // Defence in depth: only allow safe schemes. We never pass arbitrary
    // shell to `open` (it's argv-based) but reject schemes that don't make
    // sense for a "click a link" handler.
    let lower = url.to_ascii_lowercase();
    let ok = lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("mailto:");
    if !ok {
        return Err(format!("refusing to open scheme: {url}"));
    }

    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(&url).spawn();

    #[cfg(target_os = "windows")]
    let result = Command::new("cmd").args(["/C", "start", "", &url]).spawn();

    #[cfg(all(unix, not(target_os = "macos")))]
    let result = Command::new("xdg-open").arg(&url).spawn();

    result
        .map(|_| ())
        .map_err(|e| format!("failed to open: {e}"))
}

// ── Cloud auth commands ───────────────────────────────────────────────────────

/// Cloud URL — read from env, default to the hosted service.
fn cloud_url() -> String {
    std::env::var("CLOUD_API_URL").unwrap_or_else(|_| "https://cloud.voicepolish.app".into())
}

#[tauri::command]
async fn cloud_signup(
    email: String,
    password: String,
    backend: State<'_, BackendState>,
) -> Result<api::CloudAuthResponse, String> {
    let resp = api::cloud_signup(&cloud_url(), &email, &password).await?;
    // Persist token in local backend SQLite
    if let Ok(ep) = get_endpoint(&backend) {
        let _ = api::store_cloud_token(&ep, &resp.token, &resp.account.license_tier).await;
    }
    Ok(resp)
}

#[tauri::command]
async fn cloud_login(
    email: String,
    password: String,
    backend: State<'_, BackendState>,
) -> Result<api::CloudAuthResponse, String> {
    let resp = api::cloud_login(&cloud_url(), &email, &password).await?;
    if let Ok(ep) = get_endpoint(&backend) {
        let _ = api::store_cloud_token(&ep, &resp.token, &resp.account.license_tier).await;
    }
    Ok(resp)
}

#[tauri::command]
async fn cloud_logout(backend: State<'_, BackendState>) -> Result<(), String> {
    let ep = get_endpoint(&backend)?;
    api::clear_cloud_token(&ep).await
}

#[tauri::command]
async fn get_cloud_status(backend: State<'_, BackendState>) -> Result<api::CloudStatus, String> {
    let ep = get_endpoint(&backend)?;
    api::get_cloud_status(&ep).await
}

// ── OpenAI OAuth commands ─────────────────────────────────────────────────────

#[tauri::command]
async fn get_openai_status(backend: State<'_, BackendState>) -> Result<serde_json::Value, String> {
    let ep = get_endpoint(&backend)?;
    api::get_openai_status(&ep).await
}

#[tauri::command]
async fn initiate_openai_oauth(
    backend: State<'_, BackendState>,
) -> Result<serde_json::Value, String> {
    let ep = get_endpoint(&backend)?;
    let result = api::initiate_openai_oauth(&ep).await?;
    // Open the auth URL in the user's default browser
    if let Some(url) = result.get("auth_url").and_then(|v| v.as_str()) {
        let _ = open::that(url);
    }
    Ok(result)
}

#[tauri::command]
async fn disconnect_openai(backend: State<'_, BackendState>) -> Result<(), String> {
    let ep = get_endpoint(&backend)?;
    api::disconnect_openai(&ep).await
}

/// On launch, refresh license from cloud if a token is stored.
/// Returns the cached tier on network failure (graceful degradation).
#[tauri::command]
async fn refresh_license(backend: State<'_, BackendState>) -> Result<serde_json::Value, String> {
    let ep = get_endpoint(&backend)?;
    let status = api::get_cloud_status(&ep).await?;
    if !status.connected {
        return Ok(serde_json::json!({ "tier": "free", "source": "local" }));
    }
    // We don't store the raw token in Tauri state, but the backend has it.
    // We can get it back via the status endpoint... but the backend doesn't
    // expose the raw token over HTTP for security. So for license refresh,
    // Tauri asks the backend to re-check — the backend can do this if needed.
    // For now, return the locally-stored tier.
    Ok(serde_json::json!({
        "tier":      status.license_tier,
        "connected": status.connected,
        "source":    "local",
    }))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn get_endpoint(backend: &State<'_, BackendState>) -> Result<BackendEndpoint, String> {
    let lock = backend.0.lock().map_err(|_| "lock failed")?;
    lock.clone().ok_or_else(|| "backend not started".into())
}

// ── Edit watcher ──────────────────────────────────────────────────────────────

/// After pasting, poll the focused text element for up to 2 minutes.
/// When the user stops typing for 8 s (or switches apps), emit "edit-detected"
/// so the frontend can ask "Save this preference?" before writing to SQLite.
async fn watch_for_edit(
    back_arc: Arc<Mutex<Option<BackendEndpoint>>>,
    app: tauri::AppHandle,
    recording_id: String,
    polished: String,                // the AI-generated text we pasted
    watch_start: std::time::Instant, // captured at the call site, right after paste
) {
    use std::time::{Duration, Instant};

    // Let the paste animation settle and focus move into the text field.
    tokio::time::sleep(Duration::from_millis(700)).await;

    // Snapshot the PID right after paste.
    let initial_pid = paster::focused_pid();

    // Attempt to get the initial field value.  Chrome / Electron may still be
    // building their AX cache even after the pre-unlock at recording-start, so
    // we retry a few times with increasing delays before declaring "AX blind".
    let post_paste = {
        let mut val = paster::read_focused_value().unwrap_or_default();
        if val.is_empty() {
            // 2nd attempt after 300 ms
            tokio::time::sleep(Duration::from_millis(300)).await;
            val = paster::read_focused_value().unwrap_or_default();
        }
        if val.is_empty() {
            // 3rd attempt after another 500 ms — AX tree should be ready by now
            tokio::time::sleep(Duration::from_millis(500)).await;
            val = paster::read_focused_value().unwrap_or_default();
        }
        val
    };

    let mut last_val = post_paste.clone();
    // best_candidate = last field value that still shared words with polished text.
    // Needed because apps like Slack clear the input after Send, making last_val
    // a UI placeholder ("Type / for commands") that replaces the actual edit.
    let mut best_candidate = post_paste.clone();
    let mut idle_at = Instant::now();
    let started     = Instant::now();
    // Capture-error metadata, hoisted so we can ship it to the backend's
    // CAPTURE_ERROR pre-filter alongside the edit text.
    let mut app_switched_during_capture: bool = false;

    tracing::info!(
        "[edit-watch] watching {recording_id} — initial field readable: {} (len={})",
        !post_paste.is_empty(),
        post_paste.len(),
    );

    // Poll loop: 30 ms cadence.  No mid-loop side effects — we only read AX
    // and watch for app switches.  Clipboard verification (Cmd+A+C) is
    // strictly an end-of-loop, same-app operation; doing it during the loop
    // disrupts the user's typing in AX-blind apps like Claude input.
    loop {
        tokio::time::sleep(Duration::from_millis(30)).await;

        // Check PID FIRST — if the user switched apps, break immediately WITHOUT
        // reading the new app's field value.  If we read first, last_val gets
        // overwritten with the new app's (empty) text, corrupting the diff.
        let now_pid = paster::focused_pid();
        let pid_switched = matches!(
            (initial_pid, now_pid),
            (Some(a), Some(b)) if a != b
        );
        if pid_switched {
            app_switched_during_capture = true;
            break;
        }

        // Still in the same app — read the current field value.
        let now_val = paster::read_focused_value().unwrap_or_default();
        if now_val != last_val {
            idle_at = Instant::now();
            // Only promote to best_candidate if the value still shares words
            // with the polished text (guards against Send-cleared placeholders).
            if shares_word_overlap(&now_val, &polished) {
                best_candidate = now_val.clone();
            }
            last_val = now_val;
        }

        let done = idle_at.elapsed() > Duration::from_secs(8)  // 8s idle — user stopped typing
            || started.elapsed() > Duration::from_secs(120); // 2-min cap

        if done {
            break;
        }
    }

    // If the final field value lost all overlap with our polished text (e.g. the
    // user sent the message and the input reverted to a placeholder), use the last
    // meaningful intermediate value instead.
    let effective_val = if shares_word_overlap(&last_val, &polished) {
        last_val.clone()
    } else if best_candidate != post_paste {
        tracing::info!(
            "[edit-watch] last_val lost overlap with polished (sent message?); using best_candidate"
        );
        best_candidate.clone()
    } else {
        last_val.clone()
    };

    let final_pid = paster::focused_pid();
    tracing::info!(
        "[edit-watch] done watching {recording_id} — field changed: {}, same app: {}",
        effective_val != post_paste,
        matches!((initial_pid, final_pid), (Some(a), Some(b)) if a == b),
    );

    // ── Determine user_kept + capture_method ───────────────────────────────────
    //
    // The capture_method is propagated to the backend so auto-promotion thresholds
    // can scale with capture confidence:
    //   • "ax"                 → AX API read directly (high confidence, ground truth)
    //   • "keystroke_verified" → keystroke replay AGREES with clipboard read (high)
    //   • "clipboard"          → clipboard read; keystroke unavailable or disagreed (medium)
    //   • "keystroke_only"     → keystroke replay; clipboard unreachable (LOW — pending only)

    let user_kept: String;
    let capture_method: &'static str;

    if !post_paste.is_empty() {
        // ── AX was readable — compare values directly ──────────────────────────
        if effective_val == post_paste {
            tracing::info!("[edit-watch] no edits detected for {recording_id}");
            return;
        }
        user_kept = extract_kept(&polished, &post_paste, &effective_val);
        capture_method = "ax";
        tracing::info!(
            "[edit-watch] edit captured (AX) for {recording_id}: {:?} → {:?}",
            polished.chars().take(60).collect::<String>(),
            user_kept.chars().take(60).collect::<String>(),
        );
    } else {
        // ── AX blind (Lark, Chrome contenteditable, WebView) ─────────────────
        //
        // Foundational rule: keystroke replay alone is STRUCTURALLY UNRELIABLE.
        // CGEventTap doesn't see selection events (mouse drag-select, some
        // app-level Cmd+A flavours), so a "select MAAR + type EMIAC" edit
        // looks identical to "type EMIAC at cursor" — we'd reconstruct
        // "EMIACMAAR" instead of the actual "EMIAC".
        //
        // Fix: ALWAYS cross-verify with clipboard. Clipboard reads the actual
        // final state of the field; it's the ground truth.
        //   • keystroke + clipboard agree           → high confidence
        //   • keystroke + clipboard disagree         → trust clipboard
        //   • clipboard available, keystroke None    → use clipboard
        //   • clipboard unavailable                  → use keystroke but mark LOW
        //   • neither available                      → skip

        #[cfg(target_os = "macos")]
        {
            let same_app = matches!(
                (initial_pid, final_pid),
                (Some(a), Some(b)) if a == b
            );

            // 1. Run keystroke replay (no side effects).
            let ks_result = reconstruct_from_keystrokes(&polished, watch_start);

            // 2. End-of-loop clipboard read — only meaningful if still in the
            //    same app.  Doing this DURING the loop would disrupt typing
            //    (Cmd+A selects the user's content), so we only do it once at
            //    the end.
            //
            //    AX-blind + app-switched scenarios cannot be captured cleanly
            //    from a third-party tool; we accept that as a structural limit
            //    rather than disrupt the user mid-edit.
            let polished_trimmed = polished.trim();
            let cb_kept: Option<String> = if same_app {
                let cb_result =
                    tokio::task::spawn_blocking(paster::capture_focused_text_via_selection)
                        .await
                        .unwrap_or(None);
                cb_result.and_then(|raw| {
                    let captured = raw.trim().to_string();
                    if !captured.contains(polished_trimmed) {
                        return None;
                    }
                    let edited = extract_kept(polished_trimmed, polished_trimmed, &captured);
                    if edited == polished_trimmed {
                        None
                    } else {
                        Some(edited)
                    }
                })
            } else {
                tracing::warn!(
                    "[edit-watch] AX-blind + app switched — clipboard unreachable for {recording_id}"
                );
                None
            };

            // 4. Reconcile keystroke vs clipboard.
            match (ks_result, cb_kept) {
                (Some(ks_text), Some(cb_text)) => {
                    // Both available — clipboard is ground truth, keystroke is
                    // confirmation.  If they agree (modulo trim) → high
                    // confidence.  If they disagree → trust clipboard.
                    if ks_text.trim() == cb_text.trim() {
                        user_kept = cb_text;
                        capture_method = "keystroke_verified";
                        tracing::info!(
                            "[edit-watch] edit captured (keystroke ⊕ clipboard agreed) for {recording_id}: {:?} → {:?}",
                            polished.chars().take(60).collect::<String>(),
                            user_kept.chars().take(60).collect::<String>(),
                        );
                    } else {
                        tracing::warn!(
                            "[edit-watch] keystroke ≠ clipboard — trusting clipboard. ks={:?} cb={:?}",
                            ks_text.chars().take(60).collect::<String>(),
                            cb_text.chars().take(60).collect::<String>(),
                        );
                        user_kept = cb_text;
                        capture_method = "clipboard";
                    }
                }
                (None, Some(cb_text)) => {
                    user_kept = cb_text;
                    capture_method = "clipboard";
                    tracing::info!(
                        "[edit-watch] edit captured (clipboard, keystroke uncertain) for {recording_id}: {:?} → {:?}",
                        polished.chars().take(60).collect::<String>(),
                        user_kept.chars().take(60).collect::<String>(),
                    );
                }
                (Some(ks_text), None) => {
                    // Clipboard unreachable. Use keystroke but mark LOW —
                    // backend will store as pending instead of auto-promoting.
                    if ks_text.trim() == polished_trimmed {
                        tracing::info!(
                            "[edit-watch] keystroke replay: no change from polished — skipping {recording_id}"
                        );
                        return;
                    }
                    user_kept = ks_text;
                    capture_method = "keystroke_only";
                    tracing::warn!(
                        "[edit-watch] edit captured (keystroke ONLY, clipboard unreachable) for {recording_id}: {:?} → {:?} (low-confidence — will store as pending only)",
                        polished.chars().take(60).collect::<String>(),
                        user_kept.chars().take(60).collect::<String>(),
                    );
                }
                (None, None) => {
                    tracing::info!(
                        "[edit-watch] both keystroke and clipboard unavailable — skipping {recording_id}"
                    );
                    return;
                }
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            tracing::info!("[edit-watch] AX blind — skipping (non-macOS) for {recording_id}");
            return;
        }
    }

    // ── Pre-flight gates (cheap, no API call) ─────────────────────────────────

    if user_kept.is_empty() || user_kept.trim() == polished.trim() {
        tracing::info!("[edit-watch] no diff for {recording_id} — skipping");
        return;
    }

    // Garbage check: if user_kept shares zero words with polished it's likely
    // a UI placeholder (e.g. Slack's "Type / for commands") that leaked through.
    //
    // Exception: format transformations like "abhishek at the rate gmail dot com"
    // → "abhishek@gmail.com" produce no word overlap but ARE valid corrections.
    // Detect these by checking if user_kept looks like an email, URL, handle, or
    // other compact identifier format — let those through to the classifier.
    if !shares_word_overlap(&user_kept, &polished) && !is_format_transformation(&user_kept) {
        tracing::info!(
            "[edit-watch] user_kept has no word overlap with polished — garbage, skipping. kept={:?}",
            user_kept.chars().take(40).collect::<String>()
        );
        return;
    }

    // Whitespace / punctuation / AX-jitter filter (no API call needed).
    if !is_meaningful_edit(&polished, &user_kept) {
        tracing::info!("[edit-watch] edit not meaningful for {recording_id} — skipping");
        return;
    }

    // ── Three-way classifier (Groq LLM call) ────────────────────────────────
    // Sends (recording_id, ai_output, user_kept) to the backend which looks up
    // the original transcript and asks Groq: "Is this an AI mistake correction
    // that we should learn from, or just user rephrasing / adding context?"
    tracing::info!(
        "[edit-watch] classifying edit for {recording_id}: polished={:?} → kept={:?}",
        polished.chars().take(50).collect::<String>(),
        user_kept.chars().take(50).collect::<String>(),
    );

    let ep_opt = back_arc.lock().ok().and_then(|g| g.clone());
    if let Some(ref ep) = ep_opt {
        let capture_meta = api::CaptureMeta {
            time_since_paste_ms: watch_start.elapsed().as_millis() as u64,
            app_switched:        app_switched_during_capture,
            // matches_clipboard is left false for now — wiring it requires
            // careful sequencing with the end-of-loop Cmd+A+C path; deferred
            // to a follow-up so this PR stays focused on the four foundational
            // accuracy fixes.
            matches_clipboard:   false,
        };
        match api::classify_edit(ep, &recording_id, &polished, &user_kept, capture_method, capture_meta).await {
            Ok(resp) => {
                tracing::info!(
                    "[edit-watch] classifier: class={} promoted={} repeat={} learned={} notify={} reason={:?} pending={:?}",
                    resp.class,
                    resp.promoted_count,
                    resp.is_repeat,
                    resp.learned,
                    resp.notify,
                    resp.reason,
                    resp.pending_id
                );

                if resp.notify {
                    // In-app toast per learnable term (matches website UI,
                    // includes Undo affordance).  We surface the first term
                    // — most edits produce one; on rare multi-promote edits
                    // the others land silently in vocab.
                    let first_term = resp.promoted_terms.first().cloned();
                    if let Some(ref term) = first_term {
                        if !term.trim().is_empty() {
                            let _ = app.emit(
                                "vocab-toast",
                                serde_json::json!({
                                    "kind":   "added",
                                    "term":   term,
                                    "source": "auto",
                                }),
                            );
                        }
                    }
                    // OS-level fallback for when the Said window isn't focused.
                    // Plain human copy — never the LLM's raw reason string,
                    // never internal class names, never punctuation flair.
                    let (title, body) = match resp.class.as_str() {
                        "STT_ERROR" => (
                            "Said learned a new word",
                            match &first_term {
                                Some(t) => {
                                    format!("Said will recognise \"{t}\" on your next recording.")
                                }
                                None => "Said remembered your correction.".to_string(),
                            },
                        ),
                        "POLISH_ERROR" => (
                            "Said updated a writing preference",
                            "Said will use your wording next time.".to_string(),
                        ),
                        _ => (
                            "Said learned from your edit",
                            "Said remembered your correction.".to_string(),
                        ),
                    };
                    notify_macos(&app, title, &body);
                }

                // Surface queued (k-event sighting recorded but not yet promoted)
                // so the user knows the system noticed their correction. Without
                // this the silent k-event behavior looks broken — the classifier
                // ran, found a real STT error, and "did nothing" from the user's
                // POV. Soft toast only; no OS banner.
                if let Some(term) = resp.queued_terms.first() {
                    if !term.trim().is_empty() {
                        let _ = app.emit(
                            "vocab-toast",
                            serde_json::json!({
                                "kind":   "queued",
                                "term":   term,
                                "source": "auto",
                            }),
                        );
                    }
                }

                if resp.learned || resp.pending_id.is_some() {
                    let _ = app.emit("pending-edits-changed", ());
                }

                // If vocabulary was updated, refresh the hot-path cache now so
                // the very next recording already uses the newly learned terms.
                if resp.learned && resp.promoted_count > 0 {
                    let hot_arc = Arc::clone(&app.state::<HotPathCache>().0);
                    let ep2 = ep.clone();
                    tokio::spawn(async move {
                        if let Ok(terms) = api::get_vocabulary_terms(&ep2).await {
                            tracing::info!(
                                "[hot_cache] refreshed after learning — {} term(s) now cached",
                                terms.len()
                            );
                            hot_arc.write().await.keyterms = terms;
                        }
                    });
                }
            }
            Err(e) => {
                tracing::warn!("[edit-watch] classify_edit call failed: {e}");
                // Classifier unavailable — fail open (don't store, don't notify).
            }
        }
    }
    let _ = back_arc; // keep arc alive until end of scope
}

/// Returns true if `candidate` shares at least one significant word (>3 chars,
/// case-insensitive ASCII) with `reference`.  Used to detect when the app has
/// cleared its text field (e.g. Slack post-send shows "Type / for commands").
/// Returns true if `text` looks like a format-transformed value — an email,
/// URL, handle, phone number, or similar compact identifier.  These are valid
/// corrections that the word-overlap garbage gate would otherwise discard,
/// because "abhishek@gmail.com" shares no whitespace-delimited tokens with
/// "Abhishek at the rate gmail dot com."
fn is_format_transformation(text: &str) -> bool {
    let t = text.trim();
    // Email address: something@something.tld
    if t.contains('@') && t.contains('.') && !t.contains(' ') {
        return true;
    }
    // URL: starts with http/https/www or contains ://
    if t.starts_with("http://")
        || t.starts_with("https://")
        || t.starts_with("www.")
        || t.contains("://")
    {
        return true;
    }
    // Handle / username: starts with @ or contains _ with no spaces
    if t.starts_with('@') || (t.contains('_') && !t.contains(' ') && t.len() < 40) {
        return true;
    }
    // Phone number: mostly digits, spaces/dashes/dots/parens, 7+ chars
    let digits: usize = t.chars().filter(|c| c.is_ascii_digit()).count();
    if digits >= 7
        && t.chars()
            .all(|c| c.is_ascii_digit() || " -.+()\u{00A0}".contains(c))
    {
        return true;
    }
    false
}

fn shares_word_overlap(candidate: &str, reference: &str) -> bool {
    let ref_words: std::collections::HashSet<String> = reference
        .split_whitespace()
        .filter(|w| w.chars().count() > 3)
        .map(|w| w.to_lowercase())
        .collect();
    if ref_words.is_empty() {
        return !candidate.is_empty();
    }
    candidate
        .split_whitespace()
        .any(|w| ref_words.contains(&w.to_lowercase()))
}

/// Given what we pasted (`polished`), where the field was right after paste
/// (`post_paste`), and the final field value (`last_val`), extract the user's
/// edited version of our text.
fn extract_kept(polished: &str, post_paste: &str, last_val: &str) -> String {
    // Find where our polished text starts in the field.
    let Some(offset) = post_paste.find(polished.trim()) else {
        // Can't locate it precisely — return the full field value.
        return last_val.to_string();
    };

    let prefix = &post_paste[..offset];
    let after_end = offset + polished.trim().len();
    let suffix = &post_paste[after_end..];

    // In last_val, strip the same prefix and suffix to get the edited middle.
    if let Some(lv_after_prefix) = last_val.strip_prefix(prefix) {
        if let Some(edited) = lv_after_prefix.strip_suffix(suffix) {
            return edited.trim().to_string();
        }
        // Suffix changed too — return everything after the prefix.
        return lv_after_prefix.trim().to_string();
    }

    // Prefix changed — return full field value as a fallback.
    last_val.to_string()
}

/// Returns true only if `user_kept` is *meaningfully* different from `polished`.
///
/// Filters out false positives caused by:
/// - Whitespace-only changes (trailing newline, extra space)
/// - Case-only changes (auto-capitalize)
/// - Smart-punctuation substitutions (smart quotes, em-dashes, ellipsis)
/// - AX read jitter (< 3 character differences) — **except** when a jargon-
///   like token (digits + letters mixed, e.g. n8n, k8s, v2.0) is involved,
///   which is exactly the case where small char diffs ARE meaningful.
fn is_meaningful_edit(polished: &str, user_kept: &str) -> bool {
    let p = normalize_for_diff(polished);
    let k = normalize_for_diff(user_kept);

    if p == k {
        tracing::info!("[edit-gate] normalized texts identical — not meaningful");
        return false;
    }

    // Word-level check: at least 1 alphanumeric word must actually differ.
    // Compute this first so the char-distance gate can be context-aware.
    let p_words: Vec<&str> = p.split_whitespace().collect();
    let k_words: Vec<&str> = k.split_whitespace().collect();
    let max_len = p_words.len().max(k_words.len());
    let mut word_diffs = 0usize;
    let mut jargon_diff = false;
    for i in 0..max_len {
        let pw = p_words.get(i).copied().unwrap_or("");
        let kw = k_words.get(i).copied().unwrap_or("");
        if pw != kw
            && (pw.chars().any(|c| c.is_alphanumeric()) || kw.chars().any(|c| c.is_alphanumeric()))
        {
            word_diffs += 1;
            // Jargon signal: if EITHER side of the diff has digits, the edit
            // is almost certainly a meaningful jargon correction (n8n, k8s,
            // v2.0, IP0 → IPO, etc.) regardless of how few chars differ.
            if pw.chars().any(|c| c.is_ascii_digit()) || kw.chars().any(|c| c.is_ascii_digit()) {
                jargon_diff = true;
            }
        }
    }

    if word_diffs == 0 {
        tracing::info!(
            "[edit-gate] no alphanumeric word diffs — punctuation/formatting only, not meaningful"
        );
        return false;
    }

    // Character-level distance gate.  Threshold = 1 for jargon edits (any
    // diff matters), 3 for plain prose (filter AX jitter / autocorrect).
    let char_diff = simple_char_distance(&p, &k);
    let min_char_diff = if jargon_diff { 1 } else { 3 };
    if char_diff < min_char_diff {
        tracing::info!(
            "[edit-gate] char distance {char_diff} < {min_char_diff} — AX jitter, not meaningful"
        );
        return false;
    }

    tracing::info!(
        "[edit-gate] {word_diffs} word(s) changed, char_diff={char_diff}, jargon={jargon_diff} — meaningful edit"
    );
    true
}

/// Normalize text for edit comparison: collapse whitespace, lowercase,
/// replace common Unicode punctuation variants with ASCII equivalents.
fn normalize_for_diff(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
        .replace('\u{201c}', "\"") // left double smart quote
        .replace('\u{201d}', "\"") // right double smart quote
        .replace('\u{2018}', "'") // left single smart quote
        .replace('\u{2019}', "'") // right single smart quote / apostrophe
        .replace('\u{2014}', "-") // em-dash
        .replace('\u{2013}', "-") // en-dash
        .replace('\u{2026}', "...") // ellipsis
        .replace('\u{00a0}', " ") // non-breaking space
}

/// Simple positional character distance (diff chars at same index + length diff).
fn simple_char_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let min_len = a_chars.len().min(b_chars.len());
    let mut diff = a_chars.len().abs_diff(b_chars.len());
    for i in 0..min_len {
        if a_chars[i] != b_chars[i] {
            diff += 1;
        }
    }
    diff
}

// ── Entry point ───────────────────────────────────────────────────────────────

fn main() {
    // 1. Load env vars from .env files
    voice_polish_core::load_env();

    // 2. Tracing — write to ~/Library/Logs/Said/said.log so logs survive in bundled app
    let log_dir = format!(
        "{}/Library/Logs/Said",
        std::env::var("HOME").unwrap_or_else(|_| ".".into())
    );
    std::fs::create_dir_all(&log_dir).ok();
    let log_path = format!("{log_dir}/said.log");
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .expect("cannot open said.log");
    // Two tracing layers: log file (always) + stderr (for `cargo run` visibility).
    {
        use tracing_subscriber::fmt;
        use tracing_subscriber::prelude::*;

        let filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "info,voice_polish_hotkey=debug,voice_polish_paster=debug".into());

        let file_layer = fmt::layer()
            .with_ansi(false)
            .with_writer(std::sync::Mutex::new(log_file));

        let stderr_layer = fmt::layer().with_ansi(true).with_writer(std::io::stderr);

        tracing_subscriber::registry()
            .with(filter)
            .with(file_layer)
            .with(stderr_layer)
            .init();
    }
    tracing::info!("[main] said desktop starting — log file: {log_path}");

    // 3. Shared state
    let shared_app = Arc::new(Mutex::new(DesktopApp::new()));
    let backend_arc = Arc::new(Mutex::new(None::<BackendEndpoint>));

    tauri::Builder::default()
        .setup({
            let shared   = Arc::clone(&shared_app);
            let back_arc = Arc::clone(&backend_arc);
            move |app| {
                // ── Spawn backend daemon ──────────────────────────────────────
                // ── Permission status at launch (visible in ~/Library/Logs/Said/said.log) ──
                let ax_ok = paster::is_accessibility_granted();
                let im_ok = hotkey::is_input_monitoring_granted();
                tracing::info!("[perm] Accessibility={ax_ok} InputMonitoring={im_ok}");
                if !ax_ok {
                    tracing::warn!("[perm] Accessibility NOT granted — paste will fail. Grant in System Settings → Privacy → Accessibility");
                }
                if !im_ok {
                    tracing::warn!("[perm] Input Monitoring NOT granted — hotkeys (Caps Lock, Option+1-5, Ctrl+Cmd+V) will not work. Grant in System Settings → Privacy → Input Monitoring");
                }

                match backend::spawn() {
                    Ok(handle) => {
                        // Extract all endpoint clones BEFORE storing (move) the handle.
                        let ep  = handle.endpoint();
                        let ep2 = handle.endpoint();
                        *back_arc.lock().unwrap() = Some(ep.clone());
                        // Store the full handle so Drop kills the child on app exit.
                        // Without this the child outlives the app (zombie leak).
                        if let Ok(mut h) = app.state::<BackendHandleState>().0.lock() {
                            *h = Some(handle);
                        }
                        tracing::info!("[main] backend daemon ready");
                        // Seed the tray cache with real prefs so the first tray
                        // menu already shows the correct model checkmark.
                        let app_h = app.handle().clone();
                        tauri::async_runtime::spawn(async move {
                            // Fetch prefs + vocab in parallel — both needed at startup.
                            let (prefs_res, vocab_res) = tokio::join!(
                                api::get_preferences(&ep),
                                api::get_vocabulary_terms(&ep),
                            );
                            if let Ok(prefs) = &prefs_res {
                                if let Ok(mut cache) = app_h.state::<TrayCache>().0.lock() {
                                    cache.custom_prompt   = prefs.custom_prompt.clone();
                                    cache.output_language = prefs.output_language.clone();
                                }
                                // Re-render now that we have real data
                                let shared = app_h.state::<SharedApp>();
                                if let Ok(d) = shared.0.lock() {
                                    let snap = d.snapshot();
                                    drop(d);
                                    sync_tray(&app_h, &snap);
                                }
                            }
                            // Seed hot-path cache so the first recording needs zero HTTP.
                            let language = prefs_res.as_ref().ok()
                                .map(|p| p.language.clone())
                                .unwrap_or_default();
                            let keyterms = vocab_res.unwrap_or_default();
                            if !keyterms.is_empty() {
                                tracing::info!("[hot_cache] seeded with {} vocab term(s)", keyterms.len());
                            }
                            let hot = app_h.state::<HotPathCache>();
                            let mut c = hot.0.write().await;
                            c.language = language;
                            c.keyterms = keyterms;
                        });

                        // ── Periodic cache refresh every 5 minutes ────────────
                        // Belt-and-suspenders: even if an event-driven refresh
                        // fails (network blip, task panic), the cache catches up
                        // within 5 minutes.
                        let app_h = app.handle().clone();
                        tauri::async_runtime::spawn(async move {
                            let mut interval = tokio::time::interval(
                                std::time::Duration::from_secs(300)
                            );
                            interval.tick().await; // skip the immediate first tick
                            loop {
                                interval.tick().await;
                                match api::get_vocabulary_terms(&ep2).await {
                                    Ok(terms) => {
                                        tracing::debug!(
                                            "[hot_cache] periodic refresh — {} term(s)", terms.len()
                                        );
                                        app_h.state::<HotPathCache>().0.write().await.keyterms = terms;
                                    }
                                    Err(e) => {
                                        tracing::warn!("[hot_cache] periodic refresh failed: {e}");
                                    }
                                }
                            }
                        });
                    }
                    Err(e) => {
                        tracing::error!("[main] failed to spawn backend: {e}");
                        // App continues without backend; commands return errors.
                    }
                }

                // ── System tray ───────────────────────────────────────────────
                // Build the initial menu from a fresh snapshot. It will be
                // rebuilt by `sync_tray()` on every state change.
                let initial_snap = shared.lock().ok().map(|d| d.snapshot());
                // Initial menu uses defaults (model=smart, no custom prompt) —
                // sync_tray() will refresh it with real prefs once the backend is ready.
                let initial_menu = match &initial_snap {
                    Some(snap) => build_tray_menu(app.handle(), snap, None, "hinglish")?,
                    None => Menu::with_items(app, &[
                        &MenuItem::with_id(app, "show", "Open Said", true, None::<&str>)?,
                        &PredefinedMenuItem::separator(app)?,
                        &MenuItem::with_id(app, "quit", "Quit Said", true, None::<&str>)?,
                    ])?,
                };

                // Brand mark — embedded retina PNG, marked as template so
                // macOS auto-tints to match menu bar appearance.
                let tray_icon = tauri::image::Image::from_bytes(
                    include_bytes!("../icons/tray@2x.png")
                ).ok();

                let mut tray_builder = TrayIconBuilder::with_id("said")
                    .tooltip("Said — Voice Polish Studio")
                    .menu(&initial_menu)
                    .show_menu_on_left_click(true);   // ← left-click opens menu

                if let Some(icon) = tray_icon {
                    tray_builder = tray_builder.icon(icon).icon_as_template(true);
                }

                tray_builder
                    .on_menu_event(|app, event| {
                        let id = event.id.as_ref();
                        match id {
                            "tray_toggle" => tray_toggle_recording(app),
                            "show" => {
                                if let Some(w) = app.get_webview_window("main") {
                                    let _ = w.show();
                                    let _ = w.set_focus();
                                }
                            }
                            "settings"  => tray_open_settings(app),
                            "reconnect" => tray_reconnect_openai(app),
                            "quit"      => app.exit(0),
                            // Output language switch
                            _ if id.starts_with("tray_lang_") => {
                                let lang = &id["tray_lang_".len()..];
                                tray_set_output_language(app, lang);
                            }
                            // Polish my message — tone preset suffix
                            _ if id.starts_with("tray_polish_") => {
                                let tone = &id["tray_polish_".len()..];
                                tray_polish_message(app, tone);
                            }
                            _ => {}
                        }
                    })
                    .build(app)?;

                // ── Close window → hide (keep running in menu bar) ────────────
                if let Some(window) = app.get_webview_window("main") {
                    let win = window.clone();
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();
                            let _ = win.hide();
                        }
                    });
                }

                // ── Floating status bar ────────────────────────────────────────
                create_status_bar(app.handle());

                // ── Caps Lock hold-to-record (macOS only) ─────────────────────
                #[cfg(target_os = "macos")]
                {
                    let h_press   = app.handle().clone();
                    let a_press   = Arc::clone(&shared);
                    let h_release = app.handle().clone();
                    let a_release = Arc::clone(&shared);
                    let b_release = Arc::clone(&back_arc);

                    hotkey::start_hold_listener(
                        Arc::new(move || {
                            let a = Arc::clone(&a_press);
                            let h = h_press.clone();
                            std::thread::spawn(move || do_start_recording(&a, &h));
                        }),
                        Arc::new(move || {
                            let a = Arc::clone(&a_release);
                            let h = h_release.clone();
                            let b = Arc::clone(&b_release);
                            std::thread::spawn(move || do_finish_recording(a, h, b));
                        }),
                    );

                    // ── Option+1..5 tone shortcuts ─────────────────────────────
                    // Select text in any app, press Option+N to polish with a preset tone.
                    //
                    // IMPORTANT: the callback runs on the CGEventTap's CFRunLoop thread.
                    // We MUST NOT call read_selected_text() on that thread — its Cmd+C
                    // fallback posts synthetic key events that queue behind the running
                    // callback and never reach the target app.  Spawning a new thread
                    // lets the tap callback return immediately so the run-loop is unblocked.
                    let app_shortcut = app.handle().clone();
                    hotkey::register_shortcut_callback(Arc::new(move |n: u8| {
                        let tone: &str = match n {
                            1 => "professional",
                            2 => "casual",
                            3 => "concise",
                            4 => "hinglish",
                            5 => "custom",
                            _ => return,
                        };
                        let app_clone = app_shortcut.clone();
                        let tone_owned = tone.to_string();
                        std::thread::spawn(move || {
                            // Small delay to let the tap callback return and the
                            // CFRunLoop process queued events before we try Cmd+C.
                            std::thread::sleep(std::time::Duration::from_millis(50));
                            tray_polish_message(&app_clone, &tone_owned);
                        });
                    }));

                    // ── Ctrl+Cmd+V — paste latest stored result ─────────────────
                    let latest_arc = std::sync::Arc::clone(
                        &app.state::<LatestResult>().inner().0
                    );
                    hotkey::register_paste_callback(Arc::new(move || {
                        let text = {
                            let Ok(g) = latest_arc.lock() else { return };
                            g.clone()
                        };
                        if let Some(t) = text {
                            tracing::info!("[paste_hotkey] Ctrl+Cmd+V → pasting {} chars", t.len());
                            std::thread::spawn(move || {
                                if let Err(e) = paster::paste(&t) {
                                    tracing::warn!("[paste_hotkey] paste failed: {e}");
                                }
                            });
                        } else {
                            tracing::info!("[paste_hotkey] Ctrl+Cmd+V pressed but nothing stored yet");
                        }
                    }));
                }

                Ok(())
            }
        })
        .plugin(tauri_plugin_notification::init())
        .manage(SharedApp(shared_app))
        .manage(BackendState(backend_arc))
        .manage(BackendHandleState(Mutex::new(None)))
        .manage(StreamingState(Mutex::new(None)))
        .manage(TrayCache(Mutex::new(TrayCacheInner::default())))
        .manage(LatestResult(std::sync::Arc::new(Mutex::new(None))))
        .manage(HotPathCache(Arc::new(tokio::sync::RwLock::new(HotPathCacheInner::default()))))
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            get_snapshot,
            get_backend_endpoint,
            get_preferences,
            patch_preferences,
            get_history,
            submit_edit_feedback,
            toggle_recording,
            set_mode,
            request_accessibility,
            request_input_monitoring,
            diagnose_ax,
            // Cloud auth
            cloud_signup,
            cloud_login,
            cloud_logout,
            get_cloud_status,
            refresh_license,
            // OpenAI OAuth
            get_openai_status,
            initiate_openai_oauth,
            disconnect_openai,
            // Paste latest
            paste_latest,
            // Retry
            retry_recording,
            // Recording management
            delete_recording,
            get_recording_audio_url,
            get_recording_audio_bytes,
            download_recording_audio,
            // Pending-edit review
            get_pending_edits,
            resolve_pending_edit,
            // Vocabulary management
            list_vocabulary,
            add_vocabulary_term,
            delete_vocabulary_term,
            star_vocabulary_term,
            // Invite a friend
            send_invite_email,
            // External URL opener (mailto:, https://) — Tauri webview blocks window.open
            open_external,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Voice Polish desktop")
        .run(|_handle, event| {
            // Only prevent exit when the last window is closed (so closing the
            // window hides it rather than quitting). An explicit app.exit(0)
            // from the tray "Quit Said" item bypasses this and terminates.
            if let tauri::RunEvent::ExitRequested { code, api, .. } = event {
                if code.is_none() {
                    // Window closed — hide instead of quit
                    api.prevent_exit();
                }
                // code.is_some() means app.exit(N) was called — let it through
            }
        });
}

// ── Tests for the meaningful-edit gate ────────────────────────────────────────

#[cfg(test)]
mod meaningful_edit_tests {
    use super::is_meaningful_edit;

    #[test]
    fn rejects_identical_after_normalize() {
        assert!(!is_meaningful_edit("Hello", "  hello  "));
    }

    #[test]
    fn rejects_punctuation_only_change() {
        assert!(!is_meaningful_edit("Hello world.", "Hello world!"));
    }

    #[test]
    fn rejects_short_non_jargon_typo() {
        // Plain prose typo within 2 chars — likely AX jitter.
        assert!(!is_meaningful_edit(
            "the meeting was good",
            "the meeting was god",
        ));
    }

    #[test]
    fn accepts_real_word_swap_at_threshold() {
        assert!(is_meaningful_edit(
            "the meeting was good",
            "the meeting was great",
        ));
    }

    #[test]
    fn accepts_short_jargon_edit_with_digits() {
        // The exact production case from logs:
        //   "Kal N10 ka IB0 nikalne wala hai." → "Kal n8n ka IB0 nikalne wala hai."
        // char_diff after normalize = 2.  Old gate rejected as "AX jitter".
        // New jargon-aware gate accepts because the changed token has digits.
        assert!(is_meaningful_edit(
            "Kal N10 ka IB0 nikalne wala hai.",
            "Kal n8n ka IB0 nikalne wala hai.",
        ));
    }

    #[test]
    fn accepts_n8n_corrections_universally() {
        assert!(is_meaningful_edit("I use 10 daily", "I use n8n daily"));
        assert!(is_meaningful_edit("I use written daily", "I use n8n daily"));
        assert!(is_meaningful_edit("I use k9s", "I use k8s")); // 1-char digit fix
        assert!(is_meaningful_edit("v2.1 release", "v2.0 release"));
    }

    #[test]
    fn rejects_zero_alphanumeric_word_changes() {
        assert!(!is_meaningful_edit("hello world", "hello   world"));
    }
}
