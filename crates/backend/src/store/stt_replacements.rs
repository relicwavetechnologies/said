//! Post-STT replacement store.
//!
//! When STT keeps emitting a wrong form for a term, biasing alone isn't enough
//! — we also rewrite the transcript before it reaches the LLM polish step.
//!
//! Each row stores `(transcript_form → correct_form)` plus a phonetic key on
//! the transcript_form.  Application is two-pass:
//!   1. Exact (case-insensitive) whole-word match on `transcript_form` → swap.
//!   2. (Optional) fuzzy match: for each transcript token, compute its phonetic
//!      key and look up by `phonetic_key`; require similarity ≥ threshold.
//!
//! The phonetic pass catches small STT variations ("aiden" / "aidan" / "ate-n")
//! that the exact pass would miss without exploding the table size.

use rusqlite::params;
use serde::Serialize;
use tracing::info;

use crate::llm::phonetics;
use super::{now_ms, DbPool};

#[derive(Debug, Clone, Serialize)]
pub struct SttReplacement {
    pub transcript_form: String,
    pub correct_form:    String,
    pub phonetic_key:    String,
    pub weight:          f64,
    pub use_count:       i64,
    pub last_used:       i64,
}

/// Upsert a (transcript_form → correct_form) replacement rule.  Same-pair
/// repeats bump weight + use_count; conflicting pairs (same transcript_form,
/// different correct_form) coexist as separate rows so we can see the
/// distribution and pick the highest-weight one at apply time.
pub fn upsert(
    pool:            &DbPool,
    user_id:         &str,
    transcript_form: &str,
    correct_form:    &str,
    bump:            f64,
) -> bool {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let from = transcript_form.trim().to_ascii_lowercase();
    let to   = correct_form.trim().to_string();
    if from.is_empty() || to.is_empty() {
        return false;
    }
    let key = phonetics::phonetic_key(&from);
    let now = now_ms();
    let rows = conn.execute(
        "INSERT INTO stt_replacements
            (user_id, transcript_form, correct_form, phonetic_key, weight, use_count, last_used)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6)
         ON CONFLICT(user_id, transcript_form, correct_form) DO UPDATE SET
            weight    = MIN(5.0, weight + ?5),
            use_count = use_count + 1,
            last_used = excluded.last_used",
        params![user_id, from, to, key, bump, now],
    ).unwrap_or(0);

    info!("[stt-repl] upsert {from:?} → {to:?} (key={key}, bump={bump}, rows={rows})");
    rows > 0
}

/// Decrement weight on revert; delete when ≤ 0.
pub fn demote(
    pool:            &DbPool,
    user_id:         &str,
    transcript_form: &str,
    correct_form:    &str,
    penalty:         f64,
) -> bool {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let from = transcript_form.trim().to_ascii_lowercase();
    let to   = correct_form.trim();
    if from.is_empty() || to.is_empty() {
        return false;
    }
    let updated = conn.execute(
        "UPDATE stt_replacements SET weight = weight - ?3
           WHERE user_id = ?1 AND transcript_form = ?2 AND correct_form = ?4",
        params![user_id, from, penalty, to],
    ).unwrap_or(0);
    if updated == 0 { return false; }

    let _ = conn.execute(
        "DELETE FROM stt_replacements
           WHERE user_id = ?1 AND transcript_form = ?2 AND correct_form = ?3 AND weight <= 0.0",
        params![user_id, from, to],
    );
    info!("[stt-repl] demote {from:?} → {to:?} penalty={penalty}");
    true
}

/// Load all replacements for a user.  Always small (tens of rows); we apply
/// them in a single linear pass over the transcript.
pub fn load_all(pool: &DbPool, user_id: &str) -> Vec<SttReplacement> {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let mut stmt = match conn.prepare(
        "SELECT transcript_form, correct_form, phonetic_key, weight, use_count, last_used
           FROM stt_replacements
          WHERE user_id = ?1
          ORDER BY weight DESC",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(params![user_id], |row| {
        Ok(SttReplacement {
            transcript_form: row.get(0)?,
            correct_form:    row.get(1)?,
            phonetic_key:    row.get(2)?,
            weight:          row.get(3)?,
            use_count:       row.get(4)?,
            last_used:       row.get(5)?,
        })
    })
    .ok()
    .map(|rows| rows.filter_map(|r| r.ok()).collect())
    .unwrap_or_default()
}

/// Apply replacements to a transcript.  Pure function; safe to unit-test.
///
/// Algorithm:
///   1. For each whitespace-separated token, find the highest-weight rule
///      whose `transcript_form` equals the lowercased token (exact pass).
///   2. If no exact match, compute the token's phonetic key and look for a
///      rule with the same key; require similarity ≥ 0.85 to apply (fuzzy).
///   3. Replace the token (preserving any trailing punctuation it carried).
pub fn apply(transcript: &str, rules: &[SttReplacement]) -> String {
    if rules.is_empty() {
        return transcript.to_string();
    }
    transcript
        .split_inclusive(char::is_whitespace)
        .map(|chunk| apply_to_chunk(chunk, rules))
        .collect()
}

fn apply_to_chunk(chunk: &str, rules: &[SttReplacement]) -> String {
    // Split chunk into core word + trailing whitespace/punct.
    let trimmed_end = chunk.trim_end();
    let trailing    = &chunk[trimmed_end.len()..];

    let core_end = trimmed_end
        .rfind(|c: char| c.is_alphanumeric())
        .map(|i| i + trimmed_end[i..].chars().next().unwrap().len_utf8())
        .unwrap_or(trimmed_end.len());
    let leading_punct = trimmed_end
        .find(|c: char| c.is_alphanumeric())
        .unwrap_or(trimmed_end.len());

    let prefix_punct = &trimmed_end[..leading_punct];
    let suffix_punct = &trimmed_end[core_end..];
    let core         = &trimmed_end[leading_punct..core_end];

    if core.is_empty() {
        return chunk.to_string();
    }
    let core_lower = core.to_ascii_lowercase();

    // Pass 1: exact match (highest weight wins; rules already sorted DESC).
    if let Some(r) = rules.iter().find(|r| r.transcript_form == core_lower) {
        return format!("{prefix_punct}{}{suffix_punct}{trailing}", r.correct_form);
    }

    // Pass 2: phonetic match.
    let key = phonetics::phonetic_key(&core_lower);
    if !key.is_empty()
        && let Some(r) = rules.iter().find(|r| r.phonetic_key == key)
    {
        let sim = phonetics::similarity(&core_lower, &r.transcript_form);
        if sim >= 0.85 {
            return format!("{prefix_punct}{}{suffix_punct}{trailing}", r.correct_form);
        }
    }

    chunk.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(from: &str, to: &str) -> SttReplacement {
        SttReplacement {
            transcript_form: from.to_string(),
            correct_form:    to.to_string(),
            phonetic_key:    phonetics::phonetic_key(from),
            weight:          1.0,
            use_count:       1,
            last_used:       0,
        }
    }

    #[test]
    fn exact_replacement_preserves_punctuation() {
        let rules = vec![rule("written", "n8n")];
        let out = apply("I use written, daily", &rules);
        assert_eq!(out, "I use n8n, daily");
    }

    #[test]
    fn no_rules_passthrough() {
        let out = apply("hello world", &[]);
        assert_eq!(out, "hello world");
    }

    #[test]
    fn case_insensitive_match() {
        let rules = vec![rule("aiden", "n8n")];
        let out = apply("Aiden is great", &rules);
        assert_eq!(out, "n8n is great");
    }

    #[test]
    fn does_not_match_substring() {
        let rules = vec![rule("ai", "AI")];
        // "aiden" should NOT match the rule keyed on "ai" — we operate on
        // whole tokens.
        let out = apply("aiden saw the rain", &rules);
        assert_eq!(out, "aiden saw the rain");
    }

    #[test]
    fn phonetic_fuzzy_match() {
        // rule from "aiden", transcript has slight variant "aidan"
        let rules = vec![rule("aiden", "n8n")];
        let out = apply("aidan is great", &rules);
        // Both phonetic-key to "ATN"; similarity == 1.0 since keys equal.
        assert_eq!(out, "n8n is great");
    }

    #[test]
    fn phonetic_does_not_overreach() {
        // "hello" and "aiden" have very different keys — must not swap.
        let rules = vec![rule("aiden", "n8n")];
        let out = apply("hello world", &rules);
        assert_eq!(out, "hello world");
    }

    #[test]
    fn highest_weight_wins_when_multiple_rules_match() {
        let mut a = rule("written", "n8n");
        a.weight = 0.5;
        let mut b = rule("written", "Wisp");
        b.weight = 2.0;
        let rules = vec![b, a]; // sorted by weight DESC by load_all
        let out = apply("I use written daily", &rules);
        assert_eq!(out, "I use Wisp daily");
    }

    // ── pool-backed tests ──────────────────────────────────────────────────────
    use r2d2_sqlite::SqliteConnectionManager;
    use crate::store::DbPool;

    fn mem_pool() -> DbPool {
        let mgr  = SqliteConnectionManager::memory();
        let pool = r2d2::Pool::builder().max_size(1).build(mgr).unwrap();
        let conn = pool.get().unwrap();
        conn.execute_batch(
            "CREATE TABLE local_user (id TEXT PRIMARY KEY);
             INSERT INTO local_user(id) VALUES ('u1');
             CREATE TABLE stt_replacements (
                 user_id          TEXT NOT NULL,
                 transcript_form  TEXT NOT NULL,
                 correct_form     TEXT NOT NULL,
                 phonetic_key     TEXT NOT NULL,
                 weight           REAL NOT NULL DEFAULT 1.0,
                 use_count        INTEGER NOT NULL DEFAULT 1,
                 last_used        INTEGER NOT NULL,
                 UNIQUE(user_id, transcript_form, correct_form)
             );"
        ).unwrap();
        pool
    }

    #[test]
    fn upsert_then_load_then_apply_round_trip() {
        let pool = mem_pool();
        super::upsert(&pool, "u1", "Written", "n8n", 1.0);
        let rules = super::load_all(&pool, "u1");
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].transcript_form, "written"); // lowercased
        let out = super::apply("I use written for automation", &rules);
        assert_eq!(out, "I use n8n for automation");
    }

    #[test]
    fn demote_until_evict() {
        let pool = mem_pool();
        super::upsert(&pool, "u1", "writen", "n8n", 1.0);
        assert_eq!(super::load_all(&pool, "u1").len(), 1);
        super::demote(&pool, "u1", "writen", "n8n", 1.5);
        assert_eq!(
            super::load_all(&pool, "u1").len(), 0,
            "rule must evict when weight ≤ 0",
        );
    }
}
