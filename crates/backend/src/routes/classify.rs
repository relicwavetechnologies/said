//! POST /v1/classify-edit
//!
//! Three-way edit classifier: given (recording_id, ai_output, user_kept),
//! looks up the original transcript, calls the Groq classifier to determine
//! if the edit is a learnable AI correction, and auto-stores a pending edit
//! if so.

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
    pub should_learn: bool,
    pub reason:       String,
    pub corrections:  Vec<classifier::Correction>,
    /// Set only when should_learn=true and the pending edit was stored.
    pub pending_id:   Option<String>,
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
                }),
            );
        }
    };

    // 4. If should_learn, auto-store as pending edit
    let pending_id = if result.should_learn {
        let id = pending_edits::insert(
            &state.pool,
            &state.default_user_id,
            Some(&body.recording_id),
            &body.ai_output,
            &body.user_kept,
        );
        if let Some(ref pid) = id {
            info!("[classify] stored pending edit {pid} for recording {}", body.recording_id);
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
            should_learn: result.should_learn,
            reason:       result.reason,
            corrections:  result.corrections,
            pending_id,
        }),
    )
}
