//! POST /v1/classify-edit
//!
//! Hands a (transcript, polished, user_kept) triple to the 4-way classifier and
//! routes the result into the correct learning store *automatically*:
//!
//!   • STT_ERROR     → vocabulary (STT-layer bias) + stt_replacements (post-STT swap)
//!   • POLISH_ERROR  → word_corrections (LLM polish substitution)
//!   • USER_REPHRASE → no learning artifact written
//!   • USER_REWRITE  → no learning artifact written
//!
//! Auto-promotion (no manual approval): STT_ERROR with a clear jargon-like
//! candidate is promoted on the FIRST sighting.  Why: a single occurrence is
//! enough — the candidate is by definition a rare/specialized term, and the
//! cost of a wrong promotion is bounded (we demote on revert).
//!
//! For POLISH_ERROR we keep the original "promote on repeat" behavior, because
//! single-shot promotions there can corrupt common words.

use axum::{extract::State, http::StatusCode, Json};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    llm::{classifier, classifier::EditClass, phonetics},
    store::{corrections, history, pending_edits, prefs::get_prefs, stt_replacements, vocabulary},
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
    pub class:            String,
    pub reason:           String,
    pub candidates:       Vec<classifier::Candidate>,
    /// True if any artifact was written (vocab / replacement / correction).
    pub learned:          bool,
    /// True if the desktop should show a notification.
    pub notify:           bool,
    /// Set when a pending_edits row was inserted (POLISH_ERROR first-sighting).
    pub pending_id:       Option<String>,
    /// Number of vocabulary or replacement terms promoted on this edit.
    pub promoted_count:   usize,
    /// Whether at least one of the candidates matched a previously-seen rule.
    pub is_repeat:        bool,
}

pub async fn classify(
    State(state): State<AppState>,
    Json(body): Json<ClassifyBody>,
) -> (StatusCode, Json<ClassifyResponse>) {
    // 1. Look up the transcript from the recording.
    let transcript = match history::get_recording(&state.pool, &body.recording_id) {
        Some(rec) => rec.transcript,
        None => {
            warn!("[classify] recording {} not found", body.recording_id);
            return (
                StatusCode::NOT_FOUND,
                Json(empty_response("USER_REPHRASE", "recording not found")),
            );
        }
    };

    // 2. Resolve Groq key.
    let groq_key = get_prefs(&state.pool, &state.default_user_id)
        .and_then(|p| p.groq_api_key)
        .or_else(|| std::env::var("GROQ_API_KEY").ok())
        .unwrap_or_default();

    // 3. Call the 4-way classifier.
    let http = Client::new();
    let result = match classifier::classify_edit(
        &http, &groq_key, &transcript, &body.ai_output, &body.user_kept,
    ).await {
        Some(r) => r,
        None => {
            info!("[classify] classifier unavailable — skipping for {}", body.recording_id);
            return (
                StatusCode::OK,
                Json(empty_response("USER_REPHRASE", "classifier unavailable")),
            );
        }
    };

    // 3b. Negative-signal demotion: if a previously-promoted vocabulary term
    //     appears in the polish but is removed/replaced in user_kept, decrement
    //     its weight.  Pure additive — runs before class-specific routing so
    //     even a USER_REPHRASE can demote a noisy past promotion.
    let demoted = run_demotion_pass(&state, &body.ai_output, &body.user_kept);
    if demoted > 0 {
        info!("[classify] demoted {demoted} vocabulary term(s) on this edit");
    }

    // 4. Route to the correct store(s) based on class.
    let mut promoted_count = 0_usize;
    let mut is_repeat      = false;
    let mut pending_id     = None;
    let mut learned        = false;

    match result.class {
        EditClass::SttError => {
            for cand in &result.candidates {
                let correct = cand.correct_form.trim();
                if correct.is_empty() { continue; }

                // Heuristic guard: only auto-promote if the candidate looks
                // jargon-like OR the classifier is highly confident.  This stops
                // the LLM from labeling a casual rephrase as STT_ERROR.
                let jargon  = phonetics::jargon_score(correct);
                let confident = result.confidence >= 0.7;
                if jargon < 0.3 && !confident {
                    info!(
                        "[classify] STT_ERROR candidate {correct:?} score={jargon:.2} \
                         conf={:.2} — below auto-promote threshold; skipping",
                        result.confidence
                    );
                    continue;
                }

                // Already in vocab? bump weight; record repeat.
                if vocabulary::upsert(&state.pool, &state.default_user_id, correct, 1.0, "auto") {
                    learned = true;
                    promoted_count += 1;
                }

                // Add (transcript_form → correct_form) replacement when STT
                // produced a distinct wrong form.
                let from = cand.transcript_form.trim();
                if !from.is_empty()
                    && !from.eq_ignore_ascii_case(correct)
                    && stt_replacements::upsert(&state.pool, &state.default_user_id, from, correct, 1.0)
                {
                    promoted_count += 1;
                }

                // Repeat detection: if vocab use_count > 1, it had been seen.
                let counts = vocabulary::top_terms(&state.pool, &state.default_user_id, 200);
                if counts.iter().any(|t| t.term.eq_ignore_ascii_case(correct) && t.use_count > 1) {
                    is_repeat = true;
                }
            }
        }

        EditClass::PolishError => {
            // Polish errors are riskier (common-word collisions) — we keep the
            // pending_edits → manual approval flow for first sightings, but the
            // backend now promotes automatically when we've seen the same wrong
            // word before in word_corrections.
            for cand in &result.candidates {
                let wrong   = cand.polish_form.trim().to_ascii_lowercase();
                let correct = cand.correct_form.trim();
                if wrong.is_empty() || correct.is_empty() || wrong == correct.to_ascii_lowercase() {
                    continue;
                }
                if correction_exists(&state, &wrong) {
                    is_repeat = true;
                    corrections::upsert(
                        &state.pool,
                        &state.default_user_id,
                        &[(wrong.clone(), correct.to_ascii_lowercase())],
                    );
                    learned = true;
                    promoted_count += 1;
                }
            }

            // Always store as pending so the user can review even single-shot
            // POLISH_ERRORs in the UI.  This is the safety valve for ambiguous
            // common-word substitutions.
            pending_id = pending_edits::insert(
                &state.pool,
                &state.default_user_id,
                Some(&body.recording_id),
                &body.ai_output,
                &body.user_kept,
            );
        }

        EditClass::UserRephrase | EditClass::UserRewrite => {
            // No learning artifact.
        }
    }

    // 5. Notification policy: notify on STT_ERROR auto-promotion (something
    //    visible just changed), or on a repeat POLISH_ERROR promotion.
    let notify = match result.class {
        EditClass::SttError    => promoted_count > 0,
        EditClass::PolishError => learned && is_repeat,
        _                      => false,
    };

    info!(
        "[classify] {} class={} promoted={} repeat={} notify={} learned={} pending={:?}",
        body.recording_id, result.class.as_str(), promoted_count, is_repeat,
        notify, learned, pending_id,
    );

    (
        StatusCode::OK,
        Json(ClassifyResponse {
            class:          result.class.as_str().to_string(),
            reason:         result.reason,
            candidates:     result.candidates,
            learned,
            notify,
            pending_id,
            promoted_count,
            is_repeat,
        }),
    )
}

/// Demote vocabulary terms that appear in the polish but are removed/replaced
/// by the user.  Returns how many terms were demoted.  We intentionally only
/// look at terms with `source != 'starred'` so user-pinned vocab is safe.
fn run_demotion_pass(state: &AppState, polish: &str, user_kept: &str) -> usize {
    let polish_lower = polish.to_ascii_lowercase();
    let kept_lower   = user_kept.to_ascii_lowercase();
    let vocab        = vocabulary::top_terms(&state.pool, &state.default_user_id, 200);

    let mut demoted = 0_usize;
    for v in vocab {
        if v.source == "starred" { continue; }
        let term_lower = v.term.to_ascii_lowercase();
        // present in polish, absent in user_kept → user removed it
        if polish_lower.contains(&term_lower)
            && !kept_lower.contains(&term_lower)
            && vocabulary::demote(&state.pool, &state.default_user_id, &v.term, 0.5)
        {
            demoted += 1;
        }
    }
    demoted
}

/// Has the user previously corrected this exact polish form?
fn correction_exists(state: &AppState, wrong: &str) -> bool {
    let conn = match state.pool.get() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM word_corrections
         WHERE user_id = ?1 AND wrong_text = ?2",
        rusqlite::params![state.default_user_id.as_str(), wrong],
        |row| row.get(0),
    ).unwrap_or(0);
    count > 0
}

fn empty_response(class: &str, reason: &str) -> ClassifyResponse {
    ClassifyResponse {
        class:          class.to_string(),
        reason:         reason.to_string(),
        candidates:     vec![],
        learned:        false,
        notify:         false,
        pending_id:     None,
        promoted_count: 0,
        is_repeat:      false,
    }
}
