//! POST /v1/classify-edit
//!
//! Three-way edit classifier: given (recording_id, ai_output, user_kept),
//! looks up the original transcript, calls the Groq classifier to determine
//! if the edit is a learnable AI correction, and auto-stores a pending edit
//! if so.
//!
//! **Confidence-based notification tiers:**
//! - `correction_count >= 2`  → high confidence (notify user)
//! - `is_repeat == true`      → high confidence (notify user)
//! - `correction_count == 1 && !is_repeat` → silent learn (store, no notification)
//!
//! The Tauri side reads `notify` to decide whether to show a macOS notification.

use axum::{extract::State, http::StatusCode, Json};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    llm::classifier,
    store::{history, pending_edits, prefs::get_prefs},
    AppState,
};

#[derive(Deserialize)]
pub struct ClassifyBody {
    pub recording_id: String,
    pub ai_output:    String,
    pub user_kept:    String,
}

#[derive(Serialize)]
pub struct ClassifyResponse {
    pub should_learn:     bool,
    pub reason:           String,
    pub corrections:      Vec<classifier::Correction>,
    /// Set only when should_learn=true and the pending edit was stored.
    pub pending_id:       Option<String>,
    /// Number of word/phrase corrections the classifier found in this edit.
    pub correction_count: usize,
    /// True if any correction in this edit has been seen before in past edit_events.
    pub is_repeat:        bool,
    /// True if the Tauri side should show a notification to the user.
    /// Based on: correction_count >= 2 OR is_repeat.
    /// Single first-time corrections are stored silently.
    pub notify:           bool,
}

pub async fn classify(
    State(state): State<AppState>,
    Json(body): Json<ClassifyBody>,
) -> (StatusCode, Json<ClassifyResponse>) {
    // 1. Look up the transcript from the recording
    let transcript = match history::get_recording(&state.pool, &body.recording_id) {
        Some(rec) => rec.transcript,
        None => {
            warn!("[classify] recording {} not found", body.recording_id);
            return (
                StatusCode::NOT_FOUND,
                Json(ClassifyResponse {
                    should_learn: false,
                    reason: "recording not found".into(),
                    corrections: vec![],
                    pending_id: None,
                    correction_count: 0,
                    is_repeat: false,
                    notify: false,
                }),
            );
        }
    };

    // 2. Get Groq API key from prefs or env
    let groq_key = get_prefs(&state.pool, &state.default_user_id)
        .and_then(|p| p.groq_api_key)
        .or_else(|| std::env::var("GROQ_API_KEY").ok())
        .unwrap_or_default();

    // 3. Call the classifier
    let http = Client::new();
    let result = classifier::classify_edit(
        &http,
        &groq_key,
        &transcript,
        &body.ai_output,
        &body.user_kept,
    )
    .await;

    let result = match result {
        Some(r) => r,
        None => {
            // Classifier failed (no API key, network error, parse error).
            // Don't block the user — skip learning silently.
            info!("[classify] classifier unavailable — skipping learning for {}", body.recording_id);
            return (
                StatusCode::OK,
                Json(ClassifyResponse {
                    should_learn: false,
                    reason: "classifier unavailable".into(),
                    corrections: vec![],
                    pending_id: None,
                    correction_count: 0,
                    is_repeat: false,
                    notify: false,
                }),
            );
        }
    };

    let correction_count = result.corrections.len();

    // 4. Check for repeat corrections — has the user corrected any of these
    //    same words before? If so, this is high-confidence.
    let is_repeat = if result.should_learn && !result.corrections.is_empty() {
        check_repeat_corrections(&state.pool, &state.default_user_id, &result.corrections)
    } else {
        false
    };

    // 5. Decide notification tier:
    //    - 2+ corrections in one edit → clearly deliberate, notify
    //    - Repeat correction (same word corrected before) → pattern confirmed, notify
    //    - Single first-time correction → could be noise, store silently
    let notify = result.should_learn && (correction_count >= 2 || is_repeat);

    // 6. If should_learn, auto-store as pending edit
    let pending_id = if result.should_learn {
        let id = pending_edits::insert(
            &state.pool,
            &state.default_user_id,
            Some(&body.recording_id),
            &body.ai_output,
            &body.user_kept,
        );
        if let Some(ref pid) = id {
            info!(
                "[classify] stored pending edit {pid} for recording {} (corrections={}, repeat={}, notify={})",
                body.recording_id, correction_count, is_repeat, notify
            );
        }
        id
    } else {
        info!(
            "[classify] not learning for {} — reason: {}",
            body.recording_id, result.reason
        );
        None
    };

    (
        StatusCode::OK,
        Json(ClassifyResponse {
            should_learn:     result.should_learn,
            reason:           result.reason,
            corrections:      result.corrections,
            pending_id,
            correction_count,
            is_repeat,
            notify,
        }),
    )
}

/// Check if any of the classifier's corrections match a past correction the user
/// has already made.  Looks at `word_corrections` table (which accumulates all
/// historic word-level diffs) for matching `wrong_text` values.
fn check_repeat_corrections(
    pool:    &crate::store::DbPool,
    user_id: &str,
    corrections: &[classifier::Correction],
) -> bool {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return false,
    };

    for c in corrections {
        // Normalize to lowercase for matching
        let ai_word = c.ai_said.trim().to_lowercase();
        if ai_word.is_empty() { continue; }

        // Check if we've seen this exact wrong word corrected before
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM word_corrections
             WHERE user_id = ?1 AND wrong_text = ?2",
            rusqlite::params![user_id, ai_word],
            |row| row.get(0),
        ).unwrap_or(0);

        if count > 0 {
            info!(
                "[classify] repeat correction detected: '{}' was corrected before (count={})",
                ai_word, count
            );
            return true;
        }
    }
    false
}
