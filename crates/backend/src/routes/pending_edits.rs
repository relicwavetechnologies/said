//! Pending-edit review endpoints.
//!
//! POST /v1/pending-edits             — store a detected edit for later approval
//! GET  /v1/pending-edits             — list unresolved pending edits + count
//! POST /v1/pending-edits/:id/resolve — approve (→ learning corpus) or skip

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    store::{history, pending_edits, vectors},
    AppState,
};

// ── Create ────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateBody {
    pub recording_id: Option<String>,
    pub ai_output:    String,
    pub user_kept:    String,
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<CreateBody>,
) -> (StatusCode, Json<serde_json::Value>) {
    match pending_edits::insert(
        &state.pool,
        &state.default_user_id,
        body.recording_id.as_deref(),
        &body.ai_output,
        &body.user_kept,
    ) {
        Some(id) => {
            info!("[pending-edits] stored {id}");
            (StatusCode::CREATED, Json(serde_json::json!({ "id": id })))
        }
        None => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "insert failed" })),
        ),
    }
}

// ── List ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ListResponse {
    pub edits: Vec<pending_edits::PendingEdit>,
    pub total: i64,
}

pub async fn list(State(state): State<AppState>) -> Json<ListResponse> {
    let edits = pending_edits::list_pending(&state.pool, &state.default_user_id);
    let total = pending_edits::count_pending(&state.pool, &state.default_user_id);
    Json(ListResponse { edits, total })
}

// ── Resolve ───────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ResolveBody {
    pub action: String, // "approve" | "skip"
}

pub async fn resolve(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ResolveBody>,
) -> StatusCode {
    let action_code: i32 = if body.action == "approve" { 1 } else { 2 };

    // When approved, write to the learning corpus the same way the live
    // feedback route does (insert edit_event → fire-and-forget embed).
    if body.action == "approve" {
        if let Some(pe) = pending_edits::get(&state.pool, &id) {
            if let Some(rec_id) = &pe.recording_id {
                if let Some(rec) = history::get_recording(&state.pool, rec_id) {
                    let event_id = vectors::insert_edit_event(
                        &state.pool,
                        &state.default_user_id,
                        Some(rec_id),
                        &rec.transcript,
                        &pe.ai_output,
                        &pe.user_kept,
                        None,
                    );
                    history::apply_edit_feedback(&state.pool, rec_id, &pe.user_kept);
                    info!("[pending-edits] approved {id} → edit_event {:?}", event_id);
                } else {
                    warn!("[pending-edits] recording {rec_id} not found for approval of {id}");
                }
            }
        }
    }

    if pending_edits::resolve(&state.pool, &id, action_code) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}
