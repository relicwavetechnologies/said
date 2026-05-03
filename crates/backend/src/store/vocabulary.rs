//! Vocabulary store — STT-layer bias terms.
//!
//! These are correctly-spelled terms (jargon, names, brands, code identifiers)
//! that we want the STT engine to recognize.  At recording start we load the
//! top-N by weight and inject them into the STT request — Deepgram's `keyterm`
//! parameter for nova-3, Whisper's `initial_prompt` for OSS Whisper.
//!
//! Promotion paths (handled in `routes::classify`):
//!   • STT_ERROR classification with jargon-like candidate → auto-add (weight=1.0)
//!   • Repeat sighting of an existing term                  → bump weight
//!   • Manual user star (UI)                                → weight=2.0, source='starred'
//!
//! Decay: callers may periodically halve weights of terms not used in N days.
//! This module exposes the primitives; the schedule is policy elsewhere.

use rusqlite::params;
use serde::Serialize;
use tracing::info;

use super::{now_ms, DbPool};

#[derive(Debug, Clone, Serialize)]
pub struct VocabTerm {
    pub term:      String,
    pub weight:    f64,
    pub use_count: i64,
    pub last_used: i64,
    pub source:    String,
    /// Optional sentence-or-snippet the term was first observed in. Used by
    /// the polish LLM to do context-aware recognition of unseen STT
    /// mishearings ("main course ka IPO" → MACOBS, when MACOBS's example is
    /// "MACOBS ka IPO ka 12 hazaar batana"). Nullable for legacy rows.
    #[serde(default)]
    pub example_context: Option<String>,
}

/// Insert or strengthen a vocabulary term (language-agnostic — kept for
/// backward compatibility with manual-add and starred paths that don't
/// carry a language).  New code from the classifier should call
/// `upsert_for_language` instead so the term is bucketed correctly.
pub fn upsert(
    pool:    &DbPool,
    user_id: &str,
    term:    &str,
    bump:    f64,
    source:  &str,
) -> bool {
    upsert_inner(pool, user_id, term, bump, source, None, None)
}

/// Insert or strengthen a vocabulary term, recording the user's
/// `output_language` at the time of the sighting.  This is what the
/// learning pipeline calls so the keyterms slate can be filtered by
/// language at recording time (no Devanagari leaking into English-mode
/// Deepgram requests).
pub fn upsert_for_language(
    pool:    &DbPool,
    user_id: &str,
    term:    &str,
    bump:    f64,
    source:  &str,
    language: &str,
) -> bool {
    upsert_inner(pool, user_id, term, bump, source, Some(language), None)
}

/// Like `upsert_for_language` but also records an `example_context` — the
/// sentence (or snippet) the term was observed in at learning time. This
/// becomes part of the polish LLM's prompt context, enabling context-aware
/// recognition of unseen STT mishearings.
///
/// On repeat upserts:
///   • If the existing row has no example_context, the new one is set.
///   • If the existing row already has a context and the new context differs,
///     we keep the older one (first-observed wins for stability — the user's
///     earliest example is usually the most representative).
///   • Empty / whitespace-only context is treated as None.
pub fn upsert_for_language_with_context(
    pool:    &DbPool,
    user_id: &str,
    term:    &str,
    bump:    f64,
    source:  &str,
    language: &str,
    example_context: Option<&str>,
) -> bool {
    let ctx = example_context
        .map(str::trim)
        .filter(|s| !s.is_empty());
    upsert_inner(pool, user_id, term, bump, source, Some(language), ctx)
}

fn upsert_inner(
    pool:    &DbPool,
    user_id: &str,
    term:    &str,
    bump:    f64,
    source:  &str,
    language: Option<&str>,
    example_context: Option<&str>,
) -> bool {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let trimmed = term.trim();
    if trimmed.is_empty() {
        return false;
    }
    let now = now_ms();
    // Cap context to keep prompt size reasonable. 240 chars ≈ 60 tokens —
    // enough for one sentence, small enough that a vocab of 100 terms
    // adds < 1.5 KB to every polish prompt.
    const CONTEXT_MAX_CHARS: usize = 240;
    let trimmed_ctx: Option<String> = example_context.map(|c| {
        let s = c.trim();
        if s.chars().count() > CONTEXT_MAX_CHARS {
            s.chars().take(CONTEXT_MAX_CHARS).collect::<String>() + "…"
        } else {
            s.to_string()
        }
    });
    // ON CONFLICT for example_context: COALESCE keeps the first-observed
    // context (most representative; later sightings of the same term may
    // be in a generic sentence that strips meaning). New rows always set
    // it; updates only fill in if previously NULL.
    let n = match language {
        Some(lang) => conn.execute(
            "INSERT INTO vocabulary
                 (user_id, term, weight, use_count, last_used, source, language, example_context)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7)
             ON CONFLICT(user_id, term) DO UPDATE SET
                weight    = MIN(5.0, weight + ?3),
                use_count = use_count + 1,
                last_used = excluded.last_used,
                language  = excluded.language,
                example_context = COALESCE(vocabulary.example_context, excluded.example_context),
                source    = CASE
                              WHEN vocabulary.source = 'starred' THEN 'starred'
                              ELSE excluded.source
                            END",
            params![user_id, trimmed, bump, now, source, lang, trimmed_ctx],
        ),
        None => conn.execute(
            "INSERT INTO vocabulary
                 (user_id, term, weight, use_count, last_used, source, example_context)
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6)
             ON CONFLICT(user_id, term) DO UPDATE SET
                weight    = MIN(5.0, weight + ?3),
                use_count = use_count + 1,
                last_used = excluded.last_used,
                example_context = COALESCE(vocabulary.example_context, excluded.example_context),
                source    = CASE
                              WHEN vocabulary.source = 'starred' THEN 'starred'
                              ELSE excluded.source
                            END",
            params![user_id, trimmed, bump, now, source, trimmed_ctx],
        ),
    };
    match n {
        Ok(rows) => {
            info!(
                "[vocab] upsert term={trimmed:?} bump={bump} lang={:?} rows={rows}",
                language,
            );
            rows > 0
        }
        Err(e) => {
            tracing::warn!("[vocab] upsert failed: {e}");
            false
        }
    }
}

/// Decrement weight on a negative signal (e.g. user reverted a learned term).
/// Removes the row when weight drops to or below 0.
pub fn demote(pool: &DbPool, user_id: &str, term: &str, penalty: f64) -> bool {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let trimmed = term.trim();
    if trimmed.is_empty() {
        return false;
    }
    let updated = conn.execute(
        "UPDATE vocabulary SET weight = weight - ?3
           WHERE user_id = ?1 AND term = ?2",
        params![user_id, trimmed, penalty],
    ).unwrap_or(0);

    if updated == 0 {
        return false;
    }

    let removed = conn.execute(
        "DELETE FROM vocabulary
           WHERE user_id = ?1 AND term = ?2 AND weight <= 0.0 AND source != 'starred'",
        params![user_id, trimmed],
    ).unwrap_or(0);

    info!("[vocab] demote term={trimmed:?} penalty={penalty} removed={removed}");
    true
}

/// Top-N vocabulary terms by weight × recency.  Used to populate STT keyterms.
pub fn top_terms(pool: &DbPool, user_id: &str, limit: usize) -> Vec<VocabTerm> {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut stmt = match conn.prepare(
        "SELECT term, weight, use_count, last_used, source, example_context
           FROM vocabulary
          WHERE user_id = ?1
          ORDER BY weight DESC, last_used DESC
          LIMIT ?2",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(params![user_id, limit as i64], |row| {
        Ok(VocabTerm {
            term:            row.get(0)?,
            weight:          row.get(1)?,
            use_count:       row.get(2)?,
            last_used:       row.get(3)?,
            source:          row.get(4)?,
            example_context: row.get(5).ok(),
        })
    })
    .ok()
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Convenience: extract just the term strings, for STT API injection.
pub fn top_term_strings(pool: &DbPool, user_id: &str, limit: usize) -> Vec<String> {
    top_terms(pool, user_id, limit)
        .into_iter()
        .map(|t| t.term)
        .collect()
}

/// Top-N vocabulary terms scoped to a specific language.  Rows whose
/// `language` is NULL (legacy / language-agnostic) are always included so
/// the backfill from before migration 013 doesn't disappear from the
/// keyterms slate overnight.
pub fn top_term_strings_for_language(
    pool:     &DbPool,
    user_id:  &str,
    language: &str,
    limit:    usize,
) -> Vec<String> {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut stmt = match conn.prepare(
        "SELECT term FROM vocabulary
          WHERE user_id = ?1
            AND (language = ?2 OR language IS NULL)
          ORDER BY weight DESC, last_used DESC
          LIMIT ?3",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(params![user_id, language, limit as i64], |row| row.get::<_, String>(0))
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
}

/// Total count of vocabulary entries for a user (UI badge).
pub fn count(pool: &DbPool, user_id: &str) -> i64 {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return 0,
    };
    conn.query_row(
        "SELECT COUNT(*) FROM vocabulary WHERE user_id = ?1",
        params![user_id],
        |r| r.get(0),
    ).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use r2d2_sqlite::SqliteConnectionManager;

    fn mem_pool() -> DbPool {
        let mgr  = SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder().max_size(1).build(mgr).unwrap();
        // Minimal schema needed by these tests.
        let conn = pool.get().unwrap();
        conn.execute_batch(
            "CREATE TABLE local_user (id TEXT PRIMARY KEY);
             INSERT INTO local_user(id) VALUES ('u1');
             CREATE TABLE vocabulary (
                 user_id          TEXT NOT NULL REFERENCES local_user(id),
                 term             TEXT NOT NULL,
                 weight           REAL NOT NULL DEFAULT 1.0,
                 use_count        INTEGER NOT NULL DEFAULT 1,
                 last_used        INTEGER NOT NULL,
                 source           TEXT NOT NULL DEFAULT 'auto',
                 language         TEXT,
                 example_context  TEXT,
                 UNIQUE(user_id, term)
             );"
        ).unwrap();
        pool
    }

    #[test]
    fn first_insert_creates_row_with_weight_one() {
        let pool = mem_pool();
        assert!(upsert(&pool, "u1", "n8n", 1.0, "auto"));
        let terms = top_terms(&pool, "u1", 10);
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].term, "n8n");
        assert_eq!(terms[0].use_count, 1);
        assert!((terms[0].weight - 1.0).abs() < 1e-9);
    }

    #[test]
    fn repeat_upsert_bumps_weight_and_use_count() {
        let pool = mem_pool();
        upsert(&pool, "u1", "n8n", 1.0, "auto");
        upsert(&pool, "u1", "n8n", 1.0, "auto");
        let terms = top_terms(&pool, "u1", 10);
        assert_eq!(terms[0].use_count, 2);
        assert!(terms[0].weight > 1.0);
    }

    #[test]
    fn weight_capped_at_five() {
        let pool = mem_pool();
        for _ in 0..10 {
            upsert(&pool, "u1", "n8n", 1.0, "auto");
        }
        let terms = top_terms(&pool, "u1", 10);
        assert!(terms[0].weight <= 5.0 + 1e-9);
    }

    #[test]
    fn demote_drops_row_when_weight_zero() {
        let pool = mem_pool();
        upsert(&pool, "u1", "noisy", 1.0, "auto");
        assert_eq!(count(&pool, "u1"), 1);
        assert!(demote(&pool, "u1", "noisy", 1.5));   // weight goes negative
        assert_eq!(count(&pool, "u1"), 0);
    }

    #[test]
    fn demote_does_not_drop_starred_terms() {
        let pool = mem_pool();
        upsert(&pool, "u1", "MyName", 1.0, "starred");
        demote(&pool, "u1", "MyName", 5.0);
        assert_eq!(count(&pool, "u1"), 1, "starred terms must survive demotion");
    }

    #[test]
    fn empty_term_is_ignored() {
        let pool = mem_pool();
        assert!(!upsert(&pool, "u1", "  ", 1.0, "auto"));
        assert_eq!(count(&pool, "u1"), 0);
    }

    #[test]
    fn per_language_filter_only_returns_matching_or_null() {
        let pool = mem_pool();
        // Pre-013 row (language = NULL) — should always show.
        upsert(&pool, "u1", "legacy_term", 2.0, "auto");
        // English-bucketed term.
        upsert_for_language(&pool, "u1", "english_term", 1.0, "auto", "english");
        // Hinglish-bucketed term.
        upsert_for_language(&pool, "u1", "hinglish_term", 1.0, "auto", "hinglish");

        let english = top_term_strings_for_language(&pool, "u1", "english", 10);
        assert!(english.contains(&"legacy_term".into()),    "legacy term must be returned");
        assert!(english.contains(&"english_term".into()),   "english bucket must be returned");
        assert!(!english.contains(&"hinglish_term".into()), "hinglish term must NOT leak into english");

        let hinglish = top_term_strings_for_language(&pool, "u1", "hinglish", 10);
        assert!(hinglish.contains(&"legacy_term".into()));
        assert!(hinglish.contains(&"hinglish_term".into()));
        assert!(!hinglish.contains(&"english_term".into()));
    }

    #[test]
    fn upsert_with_context_stores_example() {
        let pool = mem_pool();
        assert!(upsert_for_language_with_context(
            &pool, "u1", "MACOBS", 1.0, "auto", "hinglish",
            Some("MACOBS ka IPO ka 12 hazaar batana"),
        ));
        let terms = top_terms(&pool, "u1", 10);
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].term, "MACOBS");
        assert_eq!(
            terms[0].example_context.as_deref(),
            Some("MACOBS ka IPO ka 12 hazaar batana"),
        );
    }

    #[test]
    fn first_observed_context_wins_on_repeat_upsert() {
        // Repeat sightings shouldn't overwrite the original context — the
        // first example is usually the most representative; later sentences
        // may strip the term out of its meaningful surroundings.
        let pool = mem_pool();
        upsert_for_language_with_context(
            &pool, "u1", "MACOBS", 1.0, "auto", "hinglish",
            Some("MACOBS ka IPO ka 12 hazaar batana"),
        );
        upsert_for_language_with_context(
            &pool, "u1", "MACOBS", 1.0, "auto", "hinglish",
            Some("yes"),  // a stripped-down later example
        );
        let terms = top_terms(&pool, "u1", 10);
        assert_eq!(
            terms[0].example_context.as_deref(),
            Some("MACOBS ka IPO ka 12 hazaar batana"),
            "first-observed context should be preserved",
        );
    }

    #[test]
    fn empty_context_treated_as_none() {
        let pool = mem_pool();
        upsert_for_language_with_context(
            &pool, "u1", "n8n", 1.0, "auto", "english",
            Some("   "),  // whitespace only
        );
        assert!(top_terms(&pool, "u1", 10)[0].example_context.is_none());
    }

    #[test]
    fn long_context_is_capped() {
        let pool = mem_pool();
        let long = "x".repeat(500);
        upsert_for_language_with_context(
            &pool, "u1", "TERM", 1.0, "auto", "english", Some(&long),
        );
        let stored = top_terms(&pool, "u1", 10)[0].example_context.clone().unwrap();
        // Capped at ~240 chars + ellipsis.
        assert!(stored.chars().count() <= 241, "got {}", stored.chars().count());
        assert!(stored.ends_with('…'));
    }

    #[test]
    fn top_terms_orders_by_weight_desc() {
        let pool = mem_pool();
        upsert(&pool, "u1", "low",  1.0, "auto");
        upsert(&pool, "u1", "high", 1.0, "auto");
        upsert(&pool, "u1", "high", 1.0, "auto");
        let terms = top_term_strings(&pool, "u1", 10);
        assert_eq!(terms[0], "high");
        assert_eq!(terms[1], "low");
    }
}
