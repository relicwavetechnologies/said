//! Vocabulary HTTP routes.
//!
//! GET    /v1/vocabulary/terms      — light: top-N term strings (STT-bias hot path)
//! GET    /v1/vocabulary            — full: rows with weight/source/use_count
//! POST   /v1/vocabulary            — manual add (source = 'manual')
//! DELETE /v1/vocabulary/:term      — hard remove a single term
//! POST   /v1/vocabulary/:term/star — toggle starred status (immune to demotion)

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{store::{
    vocabulary, vocab_embeddings, vocab_fts,
    pending_promotions, stt_replacements,
    prefs::get_prefs, now_ms,
}, AppState};

// ── GET /v1/vocabulary/terms (hot path) ──────────────────────────────────────

#[derive(Serialize)]
pub struct TermsResponse {
    pub terms: Vec<String>,
}

#[derive(Deserialize, Default)]
pub struct TermsQuery {
    /// Optional output_language filter.  When supplied, returns only terms
    /// learned in this language (plus legacy language=NULL terms).  Without
    /// this, returns the language-agnostic top-N (legacy behaviour).
    pub language: Option<String>,
}

pub async fn list_terms(
    State(state): State<AppState>,
    Query(q):     Query<TermsQuery>,
) -> Json<TermsResponse> {
    // Resolve effective language: explicit query parameter wins, else fall
    // back to the user's stored output_language preference.  This means the
    // desktop hot path automatically gets language-bucketed keyterms with
    // no client-side change required.
    let lang = q.language
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            get_prefs(&state.pool, &state.default_user_id)
                .map(|p| p.output_language)
                .filter(|s| !s.trim().is_empty())
        });

    let terms = match lang.as_deref() {
        Some(lang) => vocabulary::top_term_strings_for_language(
            &state.pool, &state.default_user_id, lang, 100,
        ),
        None => vocabulary::top_term_strings(&state.pool, &state.default_user_id, 100),
    };
    Json(TermsResponse { terms })
}

// ── GET /v1/vocabulary (management UI) ───────────────────────────────────────

#[derive(Serialize)]
pub struct VocabListResponse {
    pub terms: Vec<vocabulary::VocabTerm>,
    pub total: i64,
}

pub async fn list(State(state): State<AppState>) -> Json<VocabListResponse> {
    let terms = vocabulary::top_terms(&state.pool, &state.default_user_id, 500);
    let total = vocabulary::count(&state.pool, &state.default_user_id);
    Json(VocabListResponse { terms, total })
}

// ── POST /v1/vocabulary (manual add) ─────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateBody {
    pub term: String,
}

pub async fn create(
    State(state): State<AppState>,
    Json(body):   Json<CreateBody>,
) -> (StatusCode, Json<serde_json::Value>) {
    let trimmed = body.term.trim();
    if trimmed.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "term cannot be empty" })),
        );
    }
    if trimmed.chars().count() > 64 {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "term too long (max 64 chars)" })),
        );
    }
    // Manual entries land at weight=1.5 (slightly above auto-default 1.0) so
    // user-curated terms outrank a fresh auto-promoted one.
    let ok = vocabulary::upsert(&state.pool, &state.default_user_id, trimmed, 1.5, "manual");
    if ok {
        // Sync FTS index for BM25 retrieval. Manual adds usually have no
        // example_context yet — the FTS row is keyed on the term alone
        // until a future sighting fills in context.
        vocab_fts::upsert(&state.pool, &state.default_user_id, trimmed, None);
        info!("[vocab] manual add: {trimmed:?}");
        (StatusCode::CREATED, Json(serde_json::json!({ "term": trimmed })))
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "insert failed" })),
        )
    }
}

// ── DELETE /v1/vocabulary/:term ──────────────────────────────────────────────

pub async fn delete(
    State(state): State<AppState>,
    Path(term):   Path<String>,
) -> StatusCode {
    let trimmed = term.trim();
    if trimmed.is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    let conn = match state.pool.get() {
        Ok(c) => c,
        Err(e) => { warn!("[vocab] delete pool error: {e}"); return StatusCode::INTERNAL_SERVER_ERROR; }
    };
    let n = conn.execute(
        "DELETE FROM vocabulary WHERE user_id = ?1 AND term = ?2",
        params![state.default_user_id.as_str(), trimmed],
    ).unwrap_or(0);
    drop(conn);

    // Cascade-clean every per-term side table. None of these have FK cascades
    // back to `vocabulary`, so each must be cleared explicitly. Skipping any
    // one of them leaves a different ghost behaviour:
    //   • vocab_embeddings    — stale centroid surfaces in dense retrieval
    //   • vocab_fts           — stale BM25 hit surfaces in lexical gate
    //   • vocab_embedding_examples — zombie ring resurfaces if term re-added
    //   • stt_replacements    — pre-polish layer keeps rewriting → canonical
    //   • pending_promotions  — queued K-event resurrects the deleted term
    vocab_embeddings::delete(&state.pool, &state.default_user_id, trimmed);
    vocab_fts::delete(&state.pool, &state.default_user_id, trimmed);
    let stt_n  = stt_replacements::delete_by_correct_form(
        &state.pool, &state.default_user_id, trimmed,
    );
    let pend_n = pending_promotions::delete_all_for_term(
        &state.pool, &state.default_user_id, trimmed,
    );
    info!(
        "[vocab] delete term={trimmed:?} vocab_rows={n} stt_aliases={stt_n} pending={pend_n}",
    );
    if n > 0 { StatusCode::NO_CONTENT } else { StatusCode::NOT_FOUND }
}

// ── POST /v1/vocabulary/:term/star ───────────────────────────────────────────

#[derive(Serialize)]
pub struct StarResponse {
    pub starred: bool,
}

pub async fn toggle_star(
    State(state): State<AppState>,
    Path(term):   Path<String>,
) -> (StatusCode, Json<StarResponse>) {
    let trimmed = term.trim();
    if trimmed.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(StarResponse { starred: false }));
    }
    let conn = match state.pool.get() {
        Ok(c) => c,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(StarResponse { starred: false })),
    };
    let current_source: Option<String> = conn.query_row(
        "SELECT source FROM vocabulary WHERE user_id = ?1 AND term = ?2",
        params![state.default_user_id.as_str(), trimmed],
        |r| r.get(0),
    ).ok();

    let Some(source) = current_source else {
        return (StatusCode::NOT_FOUND, Json(StarResponse { starred: false }));
    };

    let (new_source, new_weight, starred) = if source == "starred" {
        // Un-star → revert to manual (preserve weight, just lift the demotion shield).
        ("manual", 1.5_f64, false)
    } else {
        // Star → bump weight high so it sorts to top + becomes demotion-immune.
        ("starred", 3.0_f64, true)
    };

    let n = conn.execute(
        "UPDATE vocabulary SET source = ?1, weight = ?2, last_used = ?3
           WHERE user_id = ?4 AND term = ?5",
        params![new_source, new_weight, now_ms(), state.default_user_id.as_str(), trimmed],
    ).unwrap_or(0);
    info!("[vocab] toggle_star term={trimmed:?} → source={new_source} starred={starred} rows={n}");
    (StatusCode::OK, Json(StarResponse { starred }))
}
