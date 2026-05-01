//! Local desktop state machine.
//!
//! Owns the AudioRecorder and tracks whether we are idle/recording/processing.
//! All API calls (gateway, backend) have been moved to `api.rs`; this module
//! is deliberately thin so the Mutex lock time stays near-zero.

use std::time::Instant;

use voice_polish_core::{all_modes, current_mode, set_mode as core_set_mode, AppSnapshot, ProcessSummary};
use voice_polish_paster::is_accessibility_granted;
use voice_polish_recorder::{AudioRecorder, ChunkReceiver};

#[cfg(target_os = "macos")]
use voice_polish_hotkey::is_input_monitoring_granted;

// ── State machine ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum AppState { Idle, Recording, Processing }

impl AppState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Idle       => "idle",
            Self::Recording  => "recording",
            Self::Processing => "processing",
        }
    }
}

// ── DesktopApp ────────────────────────────────────────────────────────────────

pub struct DesktopApp {
    pub state:             AppState,
    pub recorder:          AudioRecorder,
    pub last_result:       Option<ProcessSummary>,
    pub last_error:        Option<String>,
    pub recording_started: Option<Instant>,
}

impl DesktopApp {
    pub fn new() -> Self {
        Self {
            state:             AppState::Idle,
            recorder:          AudioRecorder::new(),
            last_result:       None,
            last_error:        None,
            recording_started: None,
        }
    }

    /// Build a snapshot for the frontend.
    /// History, total_words, daily_streak now come from the backend SQLite;
    /// this snapshot only carries ephemeral UI state.
    pub fn snapshot(&self) -> AppSnapshot {
        let current = current_mode();
        AppSnapshot {
            state:                 self.state.as_str().to_string(),
            platform:              std::env::consts::OS.to_string(),
            current_mode:          current.key,
            current_mode_label:    current.label,
            current_model:         current.model,
            auto_paste_supported:     cfg!(target_os = "macos"),
            accessibility_granted:    is_accessibility_granted(),
            #[cfg(target_os = "macos")]
            input_monitoring_granted: is_input_monitoring_granted(),
            #[cfg(not(target_os = "macos"))]
            input_monitoring_granted: false,
            modes:                 all_modes().to_vec(),
            last_result:           self.last_result.clone(),
            last_error:            self.last_error.clone(),
            // These fields are populated from backend in Phase E
            history:       vec![],
            total_words:   0,
            daily_streak:  0,
            avg_wpm:       0,
        }
    }

    /// Take the live audio chunk receiver for Deepgram WS streaming (P5).
    /// Returns `None` if the recorder hasn't started yet or the receiver was already taken.
    pub fn take_chunk_receiver(&mut self) -> Option<ChunkReceiver> {
        self.recorder.take_chunk_receiver()
    }

    pub fn set_mode(&mut self, key: &str) -> Result<AppSnapshot, String> {
        core_set_mode(key)?;
        self.last_error = None;
        Ok(self.snapshot())
    }

    /// Begin recording. Returns the snapshot for the UI.
    pub fn start_recording(&mut self) -> Result<AppSnapshot, String> {
        if self.state == AppState::Processing {
            return Err("still processing previous recording".into());
        }
        self.recorder.start()?;
        self.state             = AppState::Recording;
        self.last_error        = None;
        self.recording_started = Some(Instant::now());
        Ok(self.snapshot())
    }

    /// Stop the recorder and return the raw WAV bytes.
    /// Sets state → Processing so the UI shows a spinner.
    /// The caller is responsible for the async API call and then
    /// calling `finish_ok` or `finish_err`.
    pub fn stop_and_extract(&mut self) -> Result<Vec<u8>, String> {
        if self.state != AppState::Recording {
            return Err("not recording".into());
        }
        self.state = AppState::Processing;
        self.recorder
            .stop()
            .ok_or_else(|| "no audio captured".to_string())
    }

    pub fn finish_ok(&mut self, result: ProcessSummary) -> AppSnapshot {
        self.state       = AppState::Idle;
        self.last_result = Some(result);
        self.last_error  = None;
        self.snapshot()
    }

    pub fn finish_err(&mut self, err: String) -> AppSnapshot {
        self.state      = AppState::Idle;
        self.last_error = Some(err);
        self.snapshot()
    }
}
