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
}

/// Insert or strengthen a vocabulary term.  If the term already exists, we
/// add `bump` to its weight and increment use_count.
pub fn upsert(
    pool:    &DbPool,
    user_id: &str,
    term:    &str,
    bump:    f64,
    source:  &str,
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
    let n = conn.execute(
        "INSERT INTO vocabulary (user_id, term, weight, use_count, last_used, source)
         VALUES (?1, ?2, ?3, 1, ?4, ?5)
         ON CONFLICT(user_id, term) DO UPDATE SET
            weight    = MIN(5.0, weight + ?3),
            use_count = use_count + 1,
            last_used = excluded.last_used,
            source    = CASE
                          WHEN vocabulary.source = 'starred' THEN 'starred'
                          ELSE excluded.source
                        END",
        params![user_id, trimmed, bump, now, source],
    );
    match n {
        Ok(rows) => {
            info!("[vocab] upsert term={trimmed:?} bump={bump} rows={rows}");
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
        "SELECT term, weight, use_count, last_used, source
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
            term:      row.get(0)?,
            weight:    row.get(1)?,
            use_count: row.get(2)?,
            last_used: row.get(3)?,
            source:    row.get(4)?,
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
                 user_id   TEXT NOT NULL REFERENCES local_user(id),
                 term      TEXT NOT NULL,
                 weight    REAL NOT NULL DEFAULT 1.0,
                 use_count INTEGER NOT NULL DEFAULT 1,
                 last_used INTEGER NOT NULL,
                 source    TEXT NOT NULL DEFAULT 'auto',
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
