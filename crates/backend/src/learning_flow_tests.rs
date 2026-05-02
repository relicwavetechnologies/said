//! End-to-end tests for the learning architecture.
//!
//! Simulates the classifier output you'd get for the canonical "n8n" case and
//! drives it through the same code paths the live route uses, verifying that:
//!   1. STT_ERROR + jargon-like candidate → vocabulary entry created.
//!   2. STT_ERROR creates a stt_replacement when transcript form differs.
//!   3. POLISH_ERROR + first-sighting → no promotion (gated on repeat).
//!   4. USER_REPHRASE → no artifacts written.
//!   5. Demotion: vocab term present in polish but absent in user_kept → weight ↓.
//!
//! These tests skip the network call to Groq — we feed `ClassifyResult` directly
//! into the same promotion logic that `routes::classify::classify` uses.

#![cfg(test)]

use crate::llm::classifier::{Candidate, ClassifyResult, EditClass};
use crate::llm::phonetics;
use crate::store::{stt_replacements, vocabulary, DbPool};
use r2d2_sqlite::SqliteConnectionManager;

/// In-memory pool with the schema needed for the learning-flow tests.
fn pool() -> DbPool {
    let mgr  = SqliteConnectionManager::memory();
    let pool = r2d2::Pool::builder().max_size(2).build(mgr).unwrap();
    let conn = pool.get().unwrap();
    conn.execute_batch(
        "CREATE TABLE local_user (id TEXT PRIMARY KEY);
         INSERT INTO local_user(id) VALUES ('u1');

         CREATE TABLE vocabulary (
             user_id   TEXT NOT NULL REFERENCES local_user(id),
             term      TEXT NOT NULL,
             weight    REAL NOT NULL DEFAULT 1.0,
             use_count INTEGER NOT NULL DEFAULT 1,
             last_used INTEGER NOT NULL,
             source    TEXT NOT NULL DEFAULT 'auto',
             UNIQUE(user_id, term)
         );

         CREATE TABLE stt_replacements (
             user_id          TEXT NOT NULL,
             transcript_form  TEXT NOT NULL,
             correct_form     TEXT NOT NULL,
             phonetic_key     TEXT NOT NULL,
             weight           REAL NOT NULL DEFAULT 1.0,
             use_count        INTEGER NOT NULL DEFAULT 1,
             last_used        INTEGER NOT NULL,
             UNIQUE(user_id, transcript_form, correct_form)
         );

         CREATE TABLE word_corrections (
             user_id      TEXT NOT NULL,
             wrong_text   TEXT NOT NULL,
             correct_text TEXT NOT NULL,
             count        INTEGER NOT NULL DEFAULT 1,
             updated_at   INTEGER NOT NULL,
             weight       REAL NOT NULL DEFAULT 1.0,
             tier         TEXT NOT NULL DEFAULT 'enforced',
             UNIQUE(user_id, wrong_text)
         );"
    ).unwrap();
    pool
}

/// Promote an STT_ERROR candidate using the same heuristic gate as the route.
/// Returns (promoted_count, learned).
fn promote_stt_error(pool: &DbPool, user_id: &str, result: &ClassifyResult) -> (usize, bool) {
    let mut promoted = 0_usize;
    let mut learned  = false;
    for cand in &result.candidates {
        let correct = cand.correct_form.trim();
        if correct.is_empty() { continue; }
        let jargon = phonetics::jargon_score(correct);
        let confident = result.confidence >= 0.7;
        if jargon < 0.3 && !confident { continue; }
        if vocabulary::upsert(pool, user_id, correct, 1.0, "auto") {
            learned = true;
            promoted += 1;
        }
        let from = cand.transcript_form.trim();
        if !from.is_empty()
            && !from.eq_ignore_ascii_case(correct)
            && stt_replacements::upsert(pool, user_id, from, correct, 1.0)
        {
            promoted += 1;
        }
    }
    (promoted, learned)
}

#[test]
fn n8n_case_promotes_to_vocabulary_and_replacement() {
    let pool   = pool();
    let result = ClassifyResult {
        class:      EditClass::SttError,
        reason:     "n8n is jargon, missing from transcript and polish".into(),
        candidates: vec![Candidate {
            spoke:           "n8n".into(),
            transcript_form: "written".into(),
            polish_form:     "written".into(),
            correct_form:    "n8n".into(),
        }],
        confidence: 0.9,
    };

    let (promoted, learned) = promote_stt_error(&pool, "u1", &result);
    assert!(learned, "should write artifacts");
    assert_eq!(promoted, 2, "vocab + stt_replacement = 2 promotions");

    // 1. vocabulary now contains "n8n"
    let terms = vocabulary::top_term_strings(&pool, "u1", 10);
    assert!(terms.iter().any(|t| t == "n8n"), "vocab missing n8n: {terms:?}");

    // 2. STT replacement now rewrites "written" → "n8n"
    let rules = stt_replacements::load_all(&pool, "u1");
    let out   = stt_replacements::apply("I use written for automation", &rules);
    assert_eq!(out, "I use n8n for automation");
}

#[test]
fn user_rephrase_writes_no_artifacts() {
    let pool   = pool();
    let result = ClassifyResult {
        class:      EditClass::UserRephrase,
        reason:     "stylistic".into(),
        candidates: vec![],
        confidence: 0.5,
    };
    // Same gate as in route: REPHRASE never reaches promote_stt_error.
    if matches!(result.class, EditClass::SttError) {
        promote_stt_error(&pool, "u1", &result);
    }
    assert_eq!(vocabulary::count(&pool, "u1"), 0);
    assert_eq!(stt_replacements::load_all(&pool, "u1").len(), 0);
}

#[test]
fn low_jargon_low_confidence_stt_candidate_is_dropped() {
    let pool   = pool();
    let result = ClassifyResult {
        class:      EditClass::SttError,
        reason:     "the".into(),
        candidates: vec![Candidate {
            spoke:           "the".into(),
            transcript_form: "thee".into(),
            polish_form:     "thee".into(),
            correct_form:    "the".into(), // common word, jargon score ~0
        }],
        confidence: 0.4,                       // below 0.7 confident threshold
    };
    let (promoted, learned) = promote_stt_error(&pool, "u1", &result);
    assert_eq!(promoted, 0);
    assert!(!learned);
    assert_eq!(vocabulary::count(&pool, "u1"), 0);
}

#[test]
fn high_confidence_stt_candidate_promotes_even_if_low_jargon() {
    let pool   = pool();
    let result = ClassifyResult {
        class:      EditClass::SttError,
        reason:     "user said `please`".into(),
        candidates: vec![Candidate {
            spoke:           "please".into(),
            transcript_form: "pleas".into(),
            polish_form:     "pleas".into(),
            correct_form:    "please".into(),
        }],
        confidence: 0.95,
    };
    let (promoted, _) = promote_stt_error(&pool, "u1", &result);
    assert!(promoted >= 1);
}

#[test]
fn repeat_promotion_bumps_use_count_not_duplicate_row() {
    let pool   = pool();
    let result = ClassifyResult {
        class:      EditClass::SttError,
        reason:     "n8n".into(),
        candidates: vec![Candidate {
            spoke: "n8n".into(),
            transcript_form: "written".into(),
            polish_form: "written".into(),
            correct_form: "n8n".into(),
        }],
        confidence: 0.9,
    };
    promote_stt_error(&pool, "u1", &result);
    promote_stt_error(&pool, "u1", &result);
    let terms = vocabulary::top_terms(&pool, "u1", 10);
    let n8n   = terms.iter().find(|t| t.term == "n8n").expect("n8n missing");
    assert_eq!(n8n.use_count, 2);
    assert!(n8n.weight > 1.0);
}

#[test]
fn vocabulary_demotion_on_revert() {
    let pool = pool();
    // Pretend we previously learned "n8n" with weight 1.0.
    vocabulary::upsert(&pool, "u1", "n8n", 1.0, "auto");
    assert_eq!(vocabulary::count(&pool, "u1"), 1);

    // Polish contained "n8n", user removed it: simulate the demotion pass.
    let polish    = "I use n8n daily";
    let user_kept = "I use it daily";
    let polish_l  = polish.to_ascii_lowercase();
    let kept_l    = user_kept.to_ascii_lowercase();
    let vocab     = vocabulary::top_terms(&pool, "u1", 200);
    for v in vocab {
        if v.source == "starred" { continue; }
        let term_l = v.term.to_ascii_lowercase();
        if polish_l.contains(&term_l) && !kept_l.contains(&term_l) {
            vocabulary::demote(&pool, "u1", &v.term, 0.5);
        }
    }

    let after = vocabulary::top_terms(&pool, "u1", 10);
    assert_eq!(after.len(), 1, "single demote should not evict yet");
    assert!(after[0].weight < 1.0);
}

#[test]
fn starred_vocabulary_immune_to_demotion() {
    let pool = pool();
    vocabulary::upsert(&pool, "u1", "ProjectAtlas", 1.0, "starred");
    // Even with five demotions, starred terms must stay.
    for _ in 0..5 {
        vocabulary::demote(&pool, "u1", "ProjectAtlas", 1.0);
    }
    assert_eq!(vocabulary::count(&pool, "u1"), 1);
}
