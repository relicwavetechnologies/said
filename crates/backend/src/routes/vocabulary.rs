//! Vocabulary HTTP routes.
//!
//! GET  /v1/vocabulary/terms  — light: returns just the term strings, top-N
//!                              by weight × recency.  Used by the Tauri WS
//!                              client to bias Deepgram in real time.
//!
//! GET  /v1/vocabulary        — full: returns each row with weight, use_count,
//!                              source, last_used.  Used by the (forthcoming)
//!                              vocabulary management UI.

use axum::{extract::State, Json};
use serde::Serialize;

use crate::{store::vocabulary, AppState};

#[derive(Serialize)]
pub struct TermsResponse {
    pub terms: Vec<String>,
}

/// GET /v1/vocabulary/terms — top 100 personal vocab terms by weight.
pub async fn list_terms(State(state): State<AppState>) -> Json<TermsResponse> {
    let terms = vocabulary::top_term_strings(&state.pool, &state.default_user_id, 100);
    Json(TermsResponse { terms })
}

#[derive(Serialize)]
pub struct VocabListResponse {
    pub terms: Vec<vocabulary::VocabTerm>,
    pub total: i64,
}

/// GET /v1/vocabulary — full rows with metadata for management UI.
pub async fn list(State(state): State<AppState>) -> Json<VocabListResponse> {
    let terms = vocabulary::top_terms(&state.pool, &state.default_user_id, 200);
    let total = vocabulary::count(&state.pool, &state.default_user_id);
    Json(VocabListResponse { terms, total })
}
