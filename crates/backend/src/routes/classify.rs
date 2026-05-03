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
        edit_diff, edit_diff::Hunk,
        phonetic_triage, phonetics, pre_filter, promotion_gate,
    },
    store::{
        corrections, history, pending_edits, pending_promotions,
        prefs::get_prefs, stt_replacements, vocabulary,
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
    /// Milliseconds elapsed between paste-completed and the captured edit.
    /// Used by the CAPTURE_ERROR pre-filter to reject edits that arrived too
    /// long after the paste (likely an unrelated edit, not a correction).
    /// Missing → 0 (treated as "no signal", does not trigger the gate).
    #[serde(default)]
    pub time_since_paste_ms: u64,
    /// True if the active app/window changed between paste and capture.
    /// Almost always means the user moved on; the captured text rarely
    /// belongs to our paste.
    #[serde(default)]
    pub app_switched: bool,
    /// True if the captured `user_kept` matches the contents of the user's
    /// clipboard at capture time.  Strong signal that what we read was the
    /// user pasting more text on top of our paste — not a typed edit.
    #[serde(default)]
    pub matches_clipboard: bool,
}

fn default_capture_method() -> String { "ax".to_string() }

/// Maximum elapsed-since-paste before we treat the edit as unrelated to
/// our paste.  30 seconds is generous (covers slow human typing, longer
/// thinking pauses) without being unbounded.
const CAPTURE_STALE_MS: u64 = 30_000;

/// Capture-confidence policy.  Maps the wire-level `capture_method` string
/// into a single bool: "is this capture trustworthy enough to auto-promote?"
fn capture_allows_auto_promote(capture_method: &str) -> bool {
    matches!(capture_method, "ax" | "keystroke_verified" | "clipboard")
}

/// Stricter subset of `capture_allows_auto_promote`: captures whose source
/// is an *atomic* read of a specific text element.  An AX read returning a
/// value means it came from the targeted element at that moment; a focus
/// change after the read doesn't invalidate it.  Keystroke / clipboard
/// captures, by contrast, can include events from a window the user already
/// switched to, so they're treated as low-confidence under app-switch.
fn is_high_confidence_capture(capture_method: &str) -> bool {
    matches!(capture_method, "ax" | "keystroke_verified")
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
    /// Terms recorded into the pending-promotions queue but not yet promoted
    /// (k-threshold not met).  Surface this to the user as a soft "noticed —
    /// once more and I'll remember" toast so the system never feels silent.
    #[serde(default)]
    pub queued_terms:   Vec<String>,
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

    // 2. CAPTURE_ERROR gate.  Each check rejects an *obviously* bad signal
    //    so the LLM doesn't burn budget on it.  Critical: we never reject a
    //    high-confidence AX or keystroke_verified capture just because of an
    //    out-of-band condition like app_switch — those reads are atomic
    //    snapshots of a specific element at a specific moment, so a later
    //    focus change doesn't invalidate them.
    //
    //      • matches_clipboard — kept text equals the user's clipboard at
    //                            capture time.  They pasted on top of our
    //                            paste; the diff is meaningless regardless of
    //                            capture method.
    //      • app_switched      — focus left the original window mid-watch.
    //                            ONLY rejected for keystroke-only / clipboard
    //                            captures (those signals can be polluted by
    //                            the new window).  AX captures are kept —
    //                            an AX read returning text means it came from
    //                            the targeted element atomically, before we
    //                            noticed the switch.
    //      • too late          — > 30 s after paste.  Universal reject; a
    //                            very late edit is rarely tied to our paste.
    if body.matches_clipboard {
        info!("[classify] capture_error: kept text matches clipboard for {}", body.recording_id);
        return (
            StatusCode::OK,
            Json(empty_response("USER_REPHRASE", "capture_error: kept matches clipboard (user pasted)")),
        );
    }
    if body.app_switched && !is_high_confidence_capture(&body.capture_method) {
        info!(
            "[classify] capture_error: app_switched + low-confidence capture ({:?}) for {}",
            body.capture_method, body.recording_id,
        );
        return (
            StatusCode::OK,
            Json(empty_response("USER_REPHRASE", "capture_error: app changed during low-confidence capture")),
        );
    }
    if body.time_since_paste_ms > CAPTURE_STALE_MS {
        info!(
            "[classify] capture_error: stale capture ({}ms after paste) for {}",
            body.time_since_paste_ms, body.recording_id,
        );
        return (
            StatusCode::OK,
            Json(empty_response("USER_REPHRASE", "capture_error: edit arrived > 30 s after paste")),
        );
    }

    // 3. Run negative-signal demotion (independent of class).  If a previously
    //    promoted vocab term appears in polish but is removed in user_kept,
    //    decrement its weight.  Always runs — even on REWRITE/REPHRASE.
    let demoted = run_demotion_pass(&state, &body.ai_output, &body.user_kept);
    if demoted > 0 {
        info!("[classify] demoted {demoted} vocabulary term(s) on this edit");
    }

    // 4. STAGE 1 — Pre-filter.
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

    // 5. STAGE 2.5 — Phonetic triage.  For each hunk, decide cheaply whether
    //    its class is obvious (clear typo/case-fix → STT_ERROR; clear synonym
    //    swap → USER_REPHRASE).  Hunks that resolve here skip the LLM entirely.
    //    Ambiguous hunks fall through.
    let triage = phonetic_triage::triage(&hunks);
    let resolved_count = triage
        .iter()
        .filter(|d| matches!(d, phonetic_triage::TriageDecision::Resolved(_)))
        .count();
    if resolved_count > 0 {
        info!(
            "[classify] triage resolved {}/{} hunk(s) without LLM",
            resolved_count, triage.len(),
        );
    }
    let ambiguous_hunks: Vec<Hunk> = triage
        .iter()
        .zip(hunks.iter())
        .filter(|(d, _)| matches!(d, phonetic_triage::TriageDecision::Ambiguous))
        .map(|(_, h)| h.clone())
        .collect();

    // 6. STAGE 3 — LLM labeler (only for hunks the triage couldn't resolve).
    //    If everything was resolved, we skip the API call entirely.
    let llm_result = if ambiguous_hunks.is_empty() {
        info!("[classify] all hunks resolved by triage — no LLM call");
        None
    } else {
        let groq_key = prefs
            .as_ref()
            .and_then(|p| p.groq_api_key.clone())
            .or_else(|| std::env::var("GROQ_API_KEY").ok())
            .unwrap_or_default();
        let http = state.http_client.clone();
        match classifier::classify_edit(
            &http, &groq_key, &transcript, &body.ai_output, &body.user_kept,
            &ambiguous_hunks, &output_language,
        ).await {
            Some(r) => Some(r),
            None    => {
                // LLM failed — but we may still have triage-resolved hunks to act on.
                // If we have NOTHING actionable, return the unavailable signal.
                if resolved_count == 0 {
                    info!("[classify] classifier unavailable — skipping for {}", body.recording_id);
                    return (
                        StatusCode::OK,
                        Json(empty_response("USER_REPHRASE", "classifier unavailable")),
                    );
                }
                None
            }
        }
    };

    // 7. Merge triage-resolved + LLM-labelled hunks back into one ordered set.
    let result = merge_triage_with_llm(triage, llm_result);

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

    // Best-effort housekeeping: drop pending-promotion rows older than 30 days
    // so abandoned candidates don't sit forever waiting for a confirming sighting
    // that will never come.
    let _ = pending_promotions::prune_stale(
        &state.pool, &state.default_user_id, 30 * 24 * 3600 * 1000,
    );

    let mut queued_terms: Vec<String> = Vec::new();

    for cand in &result.candidates {
        let correct = cand.correct_form().trim();
        if correct.is_empty() { continue; }

        match cand.class {
            EditClass::SttError => {
                if !auto_promote_allowed { continue; }
                if !stt_promotion_allowed(cand, correct, &body.user_kept, &output_language) {
                    continue;
                }

                // K-event promotion gate with adaptive threshold.  Strong
                // signals (high-confidence capture + clear jargon + phonetic
                // OR LLM-confidence) promote at k=1 so users don't have to
                // confirm obvious corrections.  Weak signals (low jargon
                // score, ambiguous LLM confidence) require k=2 to guard
                // against single-event false promotions.
                let from = cand.transcript_form().trim();
                let k = pick_k_for_stt_error(
                    cand, correct, &body.capture_method,
                );
                let decision = pending_promotions::record_sighting(
                    &state.pool, &state.default_user_id,
                    correct, from, &output_language,
                    k,
                );
                let promote_now = matches!(
                    decision, Some(pending_promotions::PromotionDecision::Promote { .. }),
                );
                if !promote_now {
                    info!(
                        "[classify] STT_ERROR queued — k={k} not met for {correct:?} (jargon={:.2})",
                        phonetics::jargon_score(correct),
                    );
                    queued_terms.push(correct.to_string());
                    continue;
                }

                if vocabulary::upsert_for_language(
                    &state.pool, &state.default_user_id, correct, 1.0, "auto", &output_language,
                ) {
                    learned = true;
                    promoted_count += 1;
                    promoted_terms.push(correct.to_string());
                }
                if !from.is_empty()
                    && !from.eq_ignore_ascii_case(correct)
                    && stt_replacements::upsert(&state.pool, &state.default_user_id, from, correct, 1.0)
                {
                    promoted_count += 1;
                }
                pending_promotions::delete(
                    &state.pool, &state.default_user_id, correct, &output_language,
                );
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
            queued_terms,
        }),
    )
}

/// Adaptive k-event threshold for STT_ERROR promotions.
///
/// Returns 1 when the signal is strong enough that asking the user to
/// confirm the correction would feel slow + dumb (clear jargon + reliable
/// capture path + either phonetic agreement or high LLM confidence).
/// Returns 2 otherwise — single-event promotion of weak signals is the
/// documented cause of WisperFlow's dictionary-bloat problem.
///
/// The thresholds:
///   • capture_method must be AX or keystroke_verified (atomic-element-bound
///     reads that we can fully trust)
///   • jargon_score(correct) ≥ 0.6 — clearly an acronym, code identifier,
///     mixed-case, or digit-bearing term (n8n, k8s, MACOBS, iPhone)
///   • EITHER phonetic similarity ≥ 0.65 (the user's correction sounds like
///     what STT heard, so it's a plausible mishearing)
///     OR LLM confidence ≥ 0.85 (the labeler is very sure)
fn pick_k_for_stt_error(
    cand:           &LabelledHunk,
    correct:        &str,
    capture_method: &str,
) -> i64 {
    let high_conf_capture = matches!(capture_method, "ax" | "keystroke_verified");
    if !high_conf_capture {
        return pending_promotions::DEFAULT_K;
    }
    let jargon = phonetics::jargon_score(correct);
    if jargon < 0.6 {
        return pending_promotions::DEFAULT_K;
    }
    let phon_sim = phonetics::similarity(cand.transcript_form(), correct)
        .max(phonetics::similarity(cand.polish_form(), correct));
    let strong_phonetic   = phon_sim >= 0.65;
    let strong_llm_conf   = cand.confidence >= 0.85;
    if strong_phonetic || strong_llm_conf {
        1
    } else {
        pending_promotions::DEFAULT_K
    }
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

/// Merge phonetic-triage decisions with the LLM's labels for the ambiguous
/// hunks.  Triage-resolved hunks keep their synthetic labels; ambiguous hunks
/// take their labels from the LLM result (in input order).  The returned
/// `ClassifyResult` looks identical to one produced by the all-LLM path, so
/// the rest of the route doesn't care which source labelled which hunk.
fn merge_triage_with_llm(
    triage:     Vec<phonetic_triage::TriageDecision>,
    llm_result: Option<classifier::ClassifyResult>,
) -> classifier::ClassifyResult {
    let mut llm_iter = llm_result
        .as_ref()
        .map(|r| r.candidates.clone())
        .unwrap_or_default()
        .into_iter();

    let mut candidates: Vec<LabelledHunk> = Vec::with_capacity(triage.len());
    for d in triage {
        match d {
            phonetic_triage::TriageDecision::Resolved(lh) => candidates.push(lh),
            phonetic_triage::TriageDecision::Ambiguous     => {
                if let Some(lh) = llm_iter.next() {
                    candidates.push(lh);
                }
                // If the LLM didn't return a label for an ambiguous hunk
                // (failure / response-shape mismatch), we drop it — better
                // to ignore one signal than to hallucinate a class.
            }
        }
    }

    // Compute overall class — priority order STT_ERROR > POLISH_ERROR
    // > USER_REWRITE > USER_REPHRASE.
    let overall = candidates
        .iter()
        .map(|c| c.class)
        .max_by_key(|c| match c {
            EditClass::SttError     => 4,
            EditClass::PolishError  => 3,
            EditClass::UserRewrite  => 2,
            EditClass::UserRephrase => 1,
        })
        .unwrap_or(EditClass::UserRephrase);

    let confidence = if candidates.is_empty() {
        0.0
    } else {
        candidates.iter().map(|c| c.confidence).sum::<f64>() / candidates.len() as f64
    };

    let reason = match &llm_result {
        Some(r) if !r.reason.is_empty() => r.reason.clone(),
        _ => "phonetic triage labelled all hunks without LLM".to_string(),
    };

    classifier::ClassifyResult { class: overall, reason, candidates, confidence }
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
        queued_terms:   vec![],
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
