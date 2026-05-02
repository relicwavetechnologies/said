//! POST /v1/classify-edit
//!
//! Four-stage learning pipeline:
//!
//!   1. **Pre-filter** (cheap, no LLM): drop no-ops, USER_REWRITE shapes
//!      (huge length deltas, polish-kept-verbatim-with-prefix), and script
//!      mismatches. Most edits exit here without an API call.
//!
//!   2. **Diff** (deterministic): compute structural hunks from polish vs
//!      user_kept. Each hunk carries `(transcript_window, polish_window,
//!      kept_window)` taken directly from the texts. The classifier in stage
//!      3 will *label* these — it can never *invent* candidates.
//!
//!   3. **Classify** (LLM as labeler): hand the hunks to Groq, which assigns
//!      one class label per hunk + an overall class for the edit. Strict
//!      JSON schema, missing labels default to USER_REPHRASE.
//!
//!   4. **Promotion gates** (data-driven, defense-in-depth): for each STT_ERROR
//!      / POLISH_ERROR labelled hunk, verify the proposed correct_form
//!      actually appears in user_kept, has the right script for the user's
//!      output language, and is plausibly an STT mishearing (phonetic or
//!      jargon evidence) before promoting to the vocabulary / replacement
//!      stores.
//!
//! The architectural invariant: a learning artifact is written ONLY when a
//! diff-derived hunk passes all four stages. The LLM cannot bypass this by
//! claiming a correction that doesn't exist in the actual edit text.

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{
    llm::{
        classifier, classifier::{EditClass, LabelledHunk},
        edit_diff, phonetics, pre_filter, promotion_gate,
    },
    store::{
        corrections, history, pending_edits, prefs::get_prefs,
        stt_replacements, vocabulary,
    },
    AppState,
};

#[derive(Deserialize)]
pub struct ClassifyBody {
    pub recording_id:   String,
    pub ai_output:      String,
    pub user_kept:      String,
    /// How the desktop captured the edit text.  Drives auto-promotion gating:
    ///   "ax" | "keystroke_verified" → high confidence, may auto-promote
    ///   "clipboard"                 → medium, may auto-promote with strict gates
    ///   "keystroke_only"            → LOW, store as pending only
    /// Missing/unknown values are treated as `"ax"` for backward compatibility.
    #[serde(default = "default_capture_method")]
    pub capture_method: String,
}

fn default_capture_method() -> String { "ax".to_string() }

/// Capture-confidence policy.  Maps the wire-level `capture_method` string
/// into a single bool: "is this capture trustworthy enough to auto-promote?"
fn capture_allows_auto_promote(capture_method: &str) -> bool {
    matches!(capture_method, "ax" | "keystroke_verified" | "clipboard")
}

#[derive(Serialize)]
pub struct ClassifyResponse {
    pub class:          String,
    pub reason:         String,
    pub candidates:     Vec<LabelledHunk>,
    pub learned:        bool,
    pub notify:         bool,
    pub pending_id:     Option<String>,
    pub promoted_count: usize,
    pub is_repeat:      bool,
    /// Flat list of correct_form values that survived all gates and were
    /// promoted to vocabulary.  The desktop uses this for the in-app toast
    /// so it doesn't have to deserialize the full LabelledHunk schema.
    #[serde(default)]
    pub promoted_terms: Vec<String>,
}

pub async fn classify(
    State(state): State<AppState>,
    Json(body): Json<ClassifyBody>,
) -> (StatusCode, Json<ClassifyResponse>) {
    // 1. Look up the transcript and the user's output language preference.
    let rec = match history::get_recording(&state.pool, &body.recording_id) {
        Some(r) => r,
        None => {
            warn!("[classify] recording {} not found", body.recording_id);
            return (
                StatusCode::NOT_FOUND,
                Json(empty_response("USER_REPHRASE", "recording not found")),
            );
        }
    };
    let transcript = rec.transcript;
    let prefs = get_prefs(&state.pool, &state.default_user_id);
    let output_language = prefs
        .as_ref()
        .map(|p| p.output_language.clone())
        .unwrap_or_else(|| "hinglish".into());

    // 2. Run negative-signal demotion (independent of class).  If a previously
    //    promoted vocab term appears in polish but is removed in user_kept,
    //    decrement its weight.  Always runs — even on REWRITE/REPHRASE.
    let demoted = run_demotion_pass(&state, &body.ai_output, &body.user_kept);
    if demoted > 0 {
        info!("[classify] demoted {demoted} vocabulary term(s) on this edit");
    }

    // 3. STAGE 1 — Pre-filter.
    match pre_filter::run(&body.ai_output, &body.user_kept, &output_language) {
        pre_filter::PreFilter::Drop => {
            info!("[classify] pre-filter: drop (no real edit) for {}", body.recording_id);
            return (
                StatusCode::OK,
                Json(empty_response("USER_REPHRASE", "no learnable change")),
            );
        }
        pre_filter::PreFilter::EarlyClass(d) => {
            info!(
                "[classify] pre-filter: early-class={} reason={:?} (skipping LLM)",
                d.class, d.reason
            );
            return (
                StatusCode::OK,
                Json(empty_response(d.class, d.reason)),
            );
        }
        pre_filter::PreFilter::Pass => {} // continue
    }

    // 4. STAGE 2 — Compute diff hunks.
    let hunks = edit_diff::diff(&transcript, &body.ai_output, &body.user_kept);
    if hunks.is_empty() {
        // Pre-filter said pass but diff found no structural change — vacuous edit.
        info!("[classify] diff produced no hunks for {}", body.recording_id);
        return (
            StatusCode::OK,
            Json(empty_response("USER_REPHRASE", "no structural diff hunks")),
        );
    }
    info!("[classify] diff produced {} hunk(s) for {}", hunks.len(), body.recording_id);

    // 5. STAGE 3 — LLM labeler.
    let groq_key = prefs
        .as_ref()
        .and_then(|p| p.groq_api_key.clone())
        .or_else(|| std::env::var("GROQ_API_KEY").ok())
        .unwrap_or_default();
    let http   = state.http_client.clone();
    let result = match classifier::classify_edit(
        &http, &groq_key, &transcript, &body.ai_output, &body.user_kept,
        &hunks, &output_language,
    ).await {
        Some(r) => r,
        None    => {
            info!("[classify] classifier unavailable — skipping for {}", body.recording_id);
            return (
                StatusCode::OK,
                Json(empty_response("USER_REPHRASE", "classifier unavailable")),
            );
        }
    };

    // 6. STAGE 4 — Promotion gates + write artifacts.
    let mut promoted_count = 0_usize;
    let mut is_repeat      = false;
    let mut pending_id     = None;
    let mut learned        = false;
    let mut promoted_terms: Vec<String> = Vec::new();

    // Capture-confidence master switch.  When false, no auto-promotion regardless
    // of class — we store as pending and let the user review.  This is the
    // foundational guard against unreliable AX-blind keystroke reconstruction
    // (where CGEventTap can't see selection events, so a "select X + type Y"
    // edit looks identical to "type Y at cursor", producing concatenations).
    let auto_promote_allowed = capture_allows_auto_promote(&body.capture_method);
    if !auto_promote_allowed {
        info!(
            "[classify] capture_method={:?} → low-confidence capture, no auto-promotion (will store as pending if learnable)",
            body.capture_method
        );
    }

    for cand in &result.candidates {
        let correct = cand.correct_form().trim();
        if correct.is_empty() { continue; }

        match cand.class {
            EditClass::SttError => {
                if !auto_promote_allowed { continue; }
                if !stt_promotion_allowed(cand, correct, &body.user_kept, &output_language) {
                    continue;
                }
                if vocabulary::upsert(&state.pool, &state.default_user_id, correct, 1.0, "auto") {
                    learned = true;
                    promoted_count += 1;
                    promoted_terms.push(correct.to_string());
                }
                let from = cand.transcript_form().trim();
                if !from.is_empty()
                    && !from.eq_ignore_ascii_case(correct)
                    && stt_replacements::upsert(&state.pool, &state.default_user_id, from, correct, 1.0)
                {
                    promoted_count += 1;
                }
                if vocabulary::top_terms(&state.pool, &state.default_user_id, 200)
                    .iter()
                    .any(|t| t.term.eq_ignore_ascii_case(correct) && t.use_count > 1)
                {
                    is_repeat = true;
                }
            }
            EditClass::PolishError => {
                if !auto_promote_allowed { continue; }
                if !polish_promotion_allowed(cand, correct, &body.user_kept, &output_language) {
                    continue;
                }
                let wrong = cand.polish_form().trim().to_ascii_lowercase();
                if wrong.is_empty() || wrong == correct.to_ascii_lowercase() {
                    continue;
                }
                if correction_exists(&state, &wrong) {
                    is_repeat = true;
                    corrections::upsert(
                        &state.pool, &state.default_user_id,
                        &[(wrong, correct.to_ascii_lowercase())],
                    );
                    learned = true;
                    promoted_count += 1;
                    promoted_terms.push(correct.to_string());
                }
            }
            EditClass::UserRephrase | EditClass::UserRewrite => {
                // No-op; safe defaults already kept by the labeler when uncertain.
            }
        }
    }

    // Invalidate lexicon cache if any corrections or stt_replacements were written
    if learned {
        crate::invalidate_lexicon_cache(&state.lexicon_cache).await;
    }

    // Pending-edit safety valve.  Two reasons to write a pending row:
    //   1. POLISH_ERROR with no auto-promotion fired (single-shot, user reviews).
    //   2. ANY learnable class (STT_ERROR | POLISH_ERROR) when the capture was
    //      low-confidence (auto_promote_allowed=false).  Without this, low-
    //      confidence captures would silently disappear.
    let learnable = result.class.is_learnable();
    let needs_pending = (result.class == EditClass::PolishError && !learned)
        || (learnable && !auto_promote_allowed);
    if needs_pending {
        pending_id = pending_edits::insert(
            &state.pool, &state.default_user_id,
            Some(&body.recording_id), &body.ai_output, &body.user_kept,
        );
    }

    let notify = match result.class {
        EditClass::SttError    => promoted_count > 0,
        EditClass::PolishError => learned && is_repeat,
        _                      => false,
    };

    info!(
        "[classify] {} overall={} hunks={} promoted={} repeat={} notify={} learned={} pending={:?}",
        body.recording_id, result.class.as_str(), result.candidates.len(),
        promoted_count, is_repeat, notify, learned, pending_id,
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
            promoted_terms,
        }),
    )
}

/// Defense-in-depth gate for STT_ERROR auto-promotion.
fn stt_promotion_allowed(
    cand:            &LabelledHunk,
    correct:         &str,
    user_kept:       &str,
    output_language: &str,
) -> bool {
    if !promotion_gate::appears_in_user_kept(correct, user_kept) {
        warn!(
            "[classify] STT_ERROR rejected — correct_form {correct:?} not in user_kept (LLM hallucination?)"
        );
        return false;
    }
    if !promotion_gate::script_matches(correct, output_language) {
        warn!(
            "[classify] STT_ERROR rejected — script of {correct:?} doesn't match output_language={output_language:?}"
        );
        return false;
    }

    // Concatenation guard: when correct_form contains polish_form as a
    // substring (e.g. polish="MAAR" + kept="EMIACMAAR"), the user almost
    // certainly intended a *replacement* but cursor positioning produced an
    // *insertion*.  Refuse to promote — it's ambiguous and produces noisy vocab.
    //
    // Exception: if extracted_term is present and points at a sub-token (which
    // by parser invariant must be a whole-word substring of the hunk windows),
    // we trust the extraction over the raw concatenation pattern.
    if cand.extracted_term.is_none()
        && promotion_gate::is_concatenation_pattern(cand.polish_form(), correct)
    {
        warn!(
            "[classify] STT_ERROR rejected — concatenation pattern: polish_form {:?} ⊂ correct_form {:?} (likely insertion-without-deletion)",
            cand.polish_form(), correct,
        );
        return false;
    }

    // Plausibility: the candidate must look STT-error-like.
    // Either phonetic similarity to the wrong form, or independent jargon-ness.
    let phon_sim = phonetics::similarity(cand.transcript_form(), correct)
        .max(phonetics::similarity(cand.polish_form(), correct));
    let jargon   = phonetics::jargon_score(correct);
    let confident = cand.confidence >= 0.7;

    if phon_sim < 0.5 && jargon < 0.4 && !confident {
        info!(
            "[classify] STT_ERROR rejected — weak signal (phon={phon_sim:.2}, jargon={jargon:.2}, conf={:.2}) for {correct:?}",
            cand.confidence,
        );
        return false;
    }
    true
}

/// Gate for POLISH_ERROR auto-promotion.
fn polish_promotion_allowed(
    _cand:           &LabelledHunk,
    correct:         &str,
    user_kept:       &str,
    output_language: &str,
) -> bool {
    promotion_gate::appears_in_user_kept(correct, user_kept)
        && promotion_gate::script_matches(correct, output_language)
}

/// Demote vocabulary terms that appear in polish but are removed in user_kept.
fn run_demotion_pass(state: &AppState, polish: &str, user_kept: &str) -> usize {
    let polish_lower = polish.to_ascii_lowercase();
    let kept_lower   = user_kept.to_ascii_lowercase();
    let vocab        = vocabulary::top_terms(&state.pool, &state.default_user_id, 200);

    let mut demoted = 0_usize;
    for v in vocab {
        if v.source == "starred" { continue; }
        let term_lower = v.term.to_ascii_lowercase();
        if polish_lower.contains(&term_lower)
            && !kept_lower.contains(&term_lower)
            && vocabulary::demote(&state.pool, &state.default_user_id, &v.term, 0.5)
        {
            demoted += 1;
        }
    }
    demoted
}

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
        promoted_terms: vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_method_policy_table() {
        assert!(capture_allows_auto_promote("ax"));
        assert!(capture_allows_auto_promote("keystroke_verified"));
        assert!(capture_allows_auto_promote("clipboard"));
        // The bug case: keystroke alone is NOT allowed to auto-promote.
        assert!(!capture_allows_auto_promote("keystroke_only"));
        // Unknown values default to refused (safe fallback).
        assert!(!capture_allows_auto_promote(""));
        assert!(!capture_allows_auto_promote("anything_else"));
    }
}
