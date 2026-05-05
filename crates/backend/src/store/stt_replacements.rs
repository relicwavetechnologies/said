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

use super::{DbPool, now_ms};
use crate::llm::phonetics;

#[derive(Debug, Clone, Serialize)]
pub struct SttReplacement {
    pub transcript_form: String,
    pub correct_form: String,
    pub phonetic_key: String,
    pub weight: f64,
    pub use_count: i64,
    pub last_used: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    Exact,
    Phonetic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMatch {
    pub transcript_form: String,
    pub correct_form: String,
    pub kind: MatchKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyResult {
    pub text: String,
    pub matches: Vec<AppliedMatch>,
}

/// Upsert a (transcript_form → correct_form) replacement rule.
///
/// Same-pair repeats bump weight + use_count; conflicting pairs (same
/// transcript_form, different correct_form) coexist as separate rows so we
/// can see the distribution and pick the highest-weight one at apply time.
///
/// Each row in `stt_replacements` is one **alias** for the canonical
/// `correct_form`. A canonical can have many aliases — different shapes STT
/// has been observed producing for the same intended utterance. Aliases are
/// learned from BOTH the polish span (what the LLM thought STT said) AND
/// the raw transcript span (what STT actually emitted) — see
/// `upsert_aliases` in this module for the foundational caller.
///
/// Apply (`apply()` below) does longest-match phrase replacement, so a
/// multi-word alias will match a multi-word transcript span and a
/// single-word alias matches a single token. No language-specific stopword
/// list, no fragmenting heuristic — just learn the actual STT output shape
/// and match against it directly.
pub fn upsert(
    pool: &DbPool,
    user_id: &str,
    transcript_form: &str,
    correct_form: &str,
    bump: f64,
) -> bool {
    upsert_inner(pool, user_id, transcript_form, correct_form, bump, None)
}

pub fn upsert_with_language(
    pool: &DbPool,
    user_id: &str,
    transcript_form: &str,
    correct_form: &str,
    bump: f64,
    language: &str,
) -> bool {
    upsert_inner(
        pool,
        user_id,
        transcript_form,
        correct_form,
        bump,
        Some(language),
    )
}

fn upsert_inner(
    pool: &DbPool,
    user_id: &str,
    transcript_form: &str,
    correct_form: &str,
    bump: f64,
    language: Option<&str>,
) -> bool {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let from = transcript_form.trim().to_ascii_lowercase();
    let to = correct_form.trim().to_string();
    if from.is_empty() || to.is_empty() {
        return false;
    }
    // Don't learn no-op rules (alias == canonical, case-insensitive ASCII).
    if from.is_ascii() && to.is_ascii() && from == to.to_ascii_lowercase() {
        return false;
    }
    let key = phonetics::phonetic_key(&from);
    let now = now_ms();
    let rows = conn
        .execute(
            "INSERT INTO stt_replacements
            (user_id, transcript_form, correct_form, phonetic_key, weight, use_count, last_used, language)
         VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?7)
         ON CONFLICT(user_id, transcript_form, correct_form) DO UPDATE SET
            weight    = MIN(5.0, weight + ?5),
            use_count = use_count + 1,
            last_used = excluded.last_used,
            language  = COALESCE(excluded.language, stt_replacements.language)",
            params![user_id, from, to, key, bump, now, language],
        )
        .unwrap_or(0);

    info!("[stt-repl] upsert {from:?} → {to:?} (key={key}, bump={bump}, rows={rows})");
    rows > 0
}

/// Upsert all known aliases for a canonical correct_form in one shot.
///
/// This is the foundational learning entrypoint. Given the (transcript_window,
/// polish_window) pair from a learning event, it stores BOTH spans as aliases
/// for the canonical. Why both:
///
///   • `polish_window` — what the LLM polish step output for the misheard
///     region. Future runs that go through identical polish behaviour will
///     match this exact form.
///
///   • `transcript_window` — what Deepgram (or any STT) actually emitted.
///     Future runs in which polish behaves differently — or in which we
///     bypass polish entirely (debug, alt models) — will match this.
///
/// Both spans are stored at full weight; the longest-match phrase apply
/// handles overlap correctly (the longer alias wins per starting position).
///
/// `transcript_window` may be empty (positional alignment failed in the
/// diff stage) — we just skip it in that case.
pub fn upsert_aliases(
    pool: &DbPool,
    user_id: &str,
    transcript_window: &str,
    polish_window: &str,
    correct_form: &str,
    bump: f64,
) -> usize {
    upsert_aliases_for_language(
        pool,
        user_id,
        transcript_window,
        polish_window,
        correct_form,
        bump,
        "",
    )
}

pub fn upsert_aliases_for_language(
    pool: &DbPool,
    user_id: &str,
    transcript_window: &str,
    polish_window: &str,
    correct_form: &str,
    bump: f64,
    language: &str,
) -> usize {
    let mut written = 0;
    let lang = if language.trim().is_empty() {
        None
    } else {
        Some(language)
    };
    if !polish_window.trim().is_empty()
        && upsert_inner(pool, user_id, polish_window, correct_form, bump, lang)
    {
        written += 1;
    }
    if !transcript_window.trim().is_empty()
        && transcript_window.trim() != polish_window.trim()
        && upsert_inner(pool, user_id, transcript_window, correct_form, bump, lang)
    {
        written += 1;
    }
    written
}

/// Decrement weight on revert; delete when ≤ 0.
pub fn demote(
    pool: &DbPool,
    user_id: &str,
    transcript_form: &str,
    correct_form: &str,
    penalty: f64,
) -> bool {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let from = transcript_form.trim().to_ascii_lowercase();
    let to = correct_form.trim();
    if from.is_empty() || to.is_empty() {
        return false;
    }
    let updated = conn
        .execute(
            "UPDATE stt_replacements SET weight = weight - ?3
           WHERE user_id = ?1 AND transcript_form = ?2 AND correct_form = ?4",
            params![user_id, from, penalty, to],
        )
        .unwrap_or(0);
    if updated == 0 {
        return false;
    }

    let _ = conn.execute(
        "DELETE FROM stt_replacements
           WHERE user_id = ?1 AND transcript_form = ?2 AND correct_form = ?3 AND weight <= 0.0",
        params![user_id, from, to],
    );
    info!("[stt-repl] demote {from:?} → {to:?} penalty={penalty}");
    true
}

/// Drop every alias whose `correct_form` matches the given canonical (case-
/// insensitive). Called from the vocabulary delete path so a removed vocab
/// term can no longer fire as a pre-polish substitution. Without this, the
/// raw STT layer would keep rewriting "main corps" → "MACOBS" even after
/// the user explicitly deleted MACOBS from their vocabulary.
///
/// Returns the number of rows removed (for logging / regression assertions).
pub fn delete_by_correct_form(pool: &DbPool, user_id: &str, correct_form: &str) -> usize {
    let Ok(conn) = pool.get() else {
        return 0;
    };
    let canon = correct_form.trim();
    if canon.is_empty() {
        return 0;
    }
    let n = conn
        .execute(
            "DELETE FROM stt_replacements
          WHERE user_id = ?1
            AND lower(correct_form) = lower(?2)",
            params![user_id, canon],
        )
        .unwrap_or(0);
    if n > 0 {
        info!("[stt-repl] cleared {n} alias(es) pointing at {canon:?}");
    }
    n
}

/// Load all replacements for a user.  Always small (tens of rows); we apply
/// them in a single linear pass over the transcript.
pub fn load_all(pool: &DbPool, user_id: &str) -> Vec<SttReplacement> {
    load_for_language(pool, user_id, "")
}

pub fn load_for_language(pool: &DbPool, user_id: &str, language: &str) -> Vec<SttReplacement> {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let sql = if language.trim().is_empty() {
        "SELECT transcript_form, correct_form, phonetic_key, weight, use_count, last_used
           FROM stt_replacements
          WHERE user_id = ?1
          ORDER BY weight DESC"
    } else {
        "SELECT transcript_form, correct_form, phonetic_key, weight, use_count, last_used
           FROM stt_replacements
          WHERE user_id = ?1
            AND (language = ?2 OR language IS NULL)
          ORDER BY weight DESC"
    };
    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    if language.trim().is_empty() {
        stmt.query_map(params![user_id], |row| {
            Ok(SttReplacement {
                transcript_form: row.get(0)?,
                correct_form: row.get(1)?,
                phonetic_key: row.get(2)?,
                weight: row.get(3)?,
                use_count: row.get(4)?,
                last_used: row.get(5)?,
            })
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    } else {
        stmt.query_map(params![user_id, language], |row| {
            Ok(SttReplacement {
                transcript_form: row.get(0)?,
                correct_form: row.get(1)?,
                phonetic_key: row.get(2)?,
                weight: row.get(3)?,
                use_count: row.get(4)?,
                last_used: row.get(5)?,
            })
        })
        .ok()
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }
}

/// Apply replacements to a transcript. Pure function; safe to unit-test.
///
/// Algorithm — **longest-match phrase replacement** with phonetic fallback:
///
///   1. Tokenise the transcript into (word, original_chunk) pairs preserving
///      whitespace + punctuation around each word.
///
///   2. Sort rules by `transcript_form` token-count DESC (longest first), so
///      a 3-word alias gets a chance to match before a 1-word alias on the
///      same starting position.
///
///   3. Walk left-to-right. At each cursor position:
///        a. Try the longest rule whose tokens equal the next N tokens
///           (case-insensitive on the WORD CORES — punctuation around them
///           is preserved verbatim from the input). If hit, emit the
///           canonical, advance N tokens, continue.
///        b. Otherwise try a single-token phonetic match (key-based,
///           similarity ≥ 0.85). If hit, emit the canonical for that one
///           token, advance 1, continue.
///        c. Otherwise emit the current chunk verbatim, advance 1.
///
/// This handles BOTH single-word rules ("corps" → "MACOBS") and multi-word
/// rules ("Main corps" → "MACOBS") uniformly — no token-only / phrase-only
/// fork, and no language-specific stopword tricks. Whatever STT span we
/// learned, we match the same span shape.
pub fn apply(transcript: &str, rules: &[SttReplacement]) -> String {
    apply_with_matches(transcript, rules).text
}

pub fn apply_with_matches(transcript: &str, rules: &[SttReplacement]) -> ApplyResult {
    if rules.is_empty() {
        return ApplyResult {
            text: transcript.to_string(),
            matches: vec![],
        };
    }

    // Pre-compute (word_count, lowercased_words) for each rule so the inner
    // loop is cheap. Sort by word_count DESC so longest-match wins.
    let mut indexed: Vec<(Vec<String>, &SttReplacement)> = rules
        .iter()
        .map(|r| {
            let words: Vec<String> = r
                .transcript_form
                .split_whitespace()
                .map(|w| w.to_ascii_lowercase())
                .collect();
            (words, r)
        })
        .filter(|(w, _)| !w.is_empty())
        .collect();
    indexed.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    // Tokenise the transcript while preserving each token's original chunk
    // (the word + any trailing whitespace and punctuation glued to it).
    let chunks: Vec<&str> = split_chunks(transcript);
    let cores: Vec<String> = chunks
        .iter()
        .map(|c| word_core(c).to_ascii_lowercase())
        .collect();

    let mut out = String::with_capacity(transcript.len());
    let mut matches = Vec::new();
    let mut i = 0;
    while i < chunks.len() {
        // Empty cores (pure-whitespace / pure-punct chunks) can't match anything.
        if cores[i].is_empty() {
            out.push_str(chunks[i]);
            i += 1;
            continue;
        }

        // Try multi-token rules at this position (longest first).
        let mut matched = None;
        for (rule_words, rule) in &indexed {
            let n = rule_words.len();
            if i + n > chunks.len() {
                continue;
            }
            // Compare rule words to the cores starting at i.
            let mut ok = true;
            let mut consumed = 0;
            let mut k = i;
            while consumed < n && k < chunks.len() {
                if cores[k].is_empty() {
                    // Skip pure-punct chunks inside the window — they don't
                    // count against the rule's word index.
                    k += 1;
                    continue;
                }
                if cores[k] != rule_words[consumed] {
                    ok = false;
                    break;
                }
                consumed += 1;
                k += 1;
            }
            if ok && consumed == n {
                matched = Some((rule, k));
                break;
            }
        }

        if let Some((rule, end)) = matched {
            // Replace the matched span with the canonical, preserving the
            // leading punctuation of the first chunk and the trailing
            // whitespace/punct of the last consumed chunk.
            let first = chunks[i];
            let last = chunks[end - 1];
            let (lead, _) = split_punct(first);
            let (_, trail) = split_punct_trailing(last);
            out.push_str(lead);
            out.push_str(&rule.correct_form);
            out.push_str(trail);
            matches.push(AppliedMatch {
                transcript_form: rule.transcript_form.clone(),
                correct_form: rule.correct_form.clone(),
                kind: MatchKind::Exact,
            });
            i = end;
            continue;
        }

        // Fall back to single-token phonetic match.
        let chunk = chunks[i];
        let core = &cores[i];
        let key = phonetics::phonetic_key(core);
        let mut phonetic_hit = None;
        if !key.is_empty() {
            for (_, rule) in &indexed {
                if rule.phonetic_key == key {
                    let sim = phonetics::similarity(core, &rule.transcript_form);
                    if sim >= 0.85 {
                        phonetic_hit = Some(rule);
                        break;
                    }
                }
            }
        }
        if let Some(rule) = phonetic_hit {
            let (lead, trail) = split_punct(chunk);
            let (_, trail2) = split_punct_trailing(trail);
            out.push_str(lead);
            out.push_str(&rule.correct_form);
            out.push_str(trail2);
            matches.push(AppliedMatch {
                transcript_form: rule.transcript_form.clone(),
                correct_form: rule.correct_form.clone(),
                kind: MatchKind::Phonetic,
            });
            i += 1;
            continue;
        }

        out.push_str(chunk);
        i += 1;
    }

    ApplyResult { text: out, matches }
}

/// Split a transcript into chunks where each chunk is one whitespace-bounded
/// segment INCLUDING the trailing whitespace. Empty chunks are skipped.
fn split_chunks(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Walk to next whitespace
        while i < bytes.len() && !bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        // Walk through the whitespace
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        out.push(&text[start..i]);
        start = i;
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// The "core" word of a chunk: alphanumeric+_+- substring with leading and
/// trailing punctuation stripped. Empty if the chunk has no word characters.
fn word_core(chunk: &str) -> &str {
    let trimmed = chunk.trim();
    let leading = trimmed
        .find(|c: char| c.is_alphanumeric() || c == '_' || c == '-')
        .unwrap_or(trimmed.len());
    let trailing = trimmed
        .rfind(|c: char| c.is_alphanumeric() || c == '_' || c == '-')
        .map(|i| i + trimmed[i..].chars().next().unwrap().len_utf8())
        .unwrap_or(trimmed.len());
    if leading >= trailing {
        return "";
    }
    &trimmed[leading..trailing]
}

/// Returns `(leading_punct_or_whitespace, rest)` for a chunk.
fn split_punct(chunk: &str) -> (&str, &str) {
    let split = chunk
        .find(|c: char| c.is_alphanumeric() || c == '_' || c == '-')
        .unwrap_or(chunk.len());
    (&chunk[..split], &chunk[split..])
}

/// Returns `(rest, trailing_punct_and_whitespace)` for a chunk.
fn split_punct_trailing(chunk: &str) -> (&str, &str) {
    let split = chunk
        .rfind(|c: char| c.is_alphanumeric() || c == '_' || c == '-')
        .map(|i| i + chunk[i..].chars().next().unwrap().len_utf8())
        .unwrap_or(0);
    (&chunk[..split], &chunk[split..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(from: &str, to: &str) -> SttReplacement {
        SttReplacement {
            transcript_form: from.to_string(),
            correct_form: to.to_string(),
            phonetic_key: phonetics::phonetic_key(from),
            weight: 1.0,
            use_count: 1,
            last_used: 0,
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
    use crate::store::DbPool;
    use r2d2_sqlite::SqliteConnectionManager;

    fn mem_pool() -> DbPool {
        let mgr = SqliteConnectionManager::memory();
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
                 language         TEXT,
                 UNIQUE(user_id, transcript_form, correct_form)
             );",
        )
        .unwrap();
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

    // ── Foundational learning + apply behavior ─────────────────────────────

    #[test]
    fn upsert_aliases_stores_both_polish_and_transcript_spans() {
        // Foundational case: STT emitted "मैं Corps" (raw transcript span),
        // polish rendered it as "Main corps", user fixed to "MACOBS".
        // BOTH spans should land as aliases so we match either shape later.
        let pool = mem_pool();
        let n = super::upsert_aliases(
            &pool,
            "u1",
            /* transcript_window */ "मैं Corps",
            /* polish_window     */ "Main corps",
            /* correct_form      */ "MACOBS",
            1.0,
        );
        assert_eq!(n, 2, "expected both spans stored");
        let rules = super::load_all(&pool, "u1");
        assert!(rules.iter().any(|r| r.transcript_form == "main corps"));
        assert!(rules.iter().any(|r| r.transcript_form == "मैं corps"));
    }

    #[test]
    fn upsert_aliases_skips_empty_transcript_window() {
        // Diff couldn't positionally align — transcript_window is empty.
        // We still store the polish span as an alias.
        let pool = mem_pool();
        let n = super::upsert_aliases(&pool, "u1", "", "Main corps", "MACOBS", 1.0);
        assert_eq!(n, 1);
    }

    #[test]
    fn upsert_aliases_dedupes_when_polish_equals_transcript() {
        // STT and polish were identical (no polish rewrite for this region).
        // Don't double-count — store once.
        let pool = mem_pool();
        let n = super::upsert_aliases(&pool, "u1", "Main corps", "Main corps", "MACOBS", 1.0);
        assert_eq!(n, 1);
        let rules = super::load_all(&pool, "u1");
        assert_eq!(rules.len(), 1);
    }

    #[test]
    fn delete_by_correct_form_clears_all_aliases_pointing_at_it() {
        // Regression: when a user deletes a vocab term, EVERY alias that
        // would otherwise rewrite raw STT into that canonical must die too.
        // Without this, the pre-polish layer keeps mapping "main corps" →
        // "MACOBS" even after MACOBS was explicitly removed from vocab.
        let pool = mem_pool();
        super::upsert_aliases(&pool, "u1", "मैं Corps", "Main corps", "MACOBS", 1.0);
        super::upsert(&pool, "u1", "main corp", "MACOBS", 1.0);
        // Unrelated alias for a different canonical — must survive.
        super::upsert(&pool, "u1", "Written", "n8n", 1.0);
        assert_eq!(super::load_all(&pool, "u1").len(), 4);

        // Case-insensitive on the canonical so `delete("macobs")` still wipes
        // an alias stored as "MACOBS".
        let n = super::delete_by_correct_form(&pool, "u1", "macobs");
        assert_eq!(n, 3, "expected 3 MACOBS aliases removed");

        let remaining = super::load_all(&pool, "u1");
        assert_eq!(remaining.len(), 1);
        assert_eq!(
            remaining[0].correct_form, "n8n",
            "unrelated alias for n8n must survive"
        );
    }

    #[test]
    fn delete_by_correct_form_is_safe_on_no_match() {
        let pool = mem_pool();
        super::upsert(&pool, "u1", "Written", "n8n", 1.0);
        let n = super::delete_by_correct_form(&pool, "u1", "NeverExisted");
        assert_eq!(n, 0);
        assert_eq!(
            super::load_all(&pool, "u1").len(),
            1,
            "unrelated rule untouched"
        );
    }

    #[test]
    fn upsert_skips_no_op_rule() {
        // Don't waste a row on alias == canonical.
        let pool = mem_pool();
        assert!(!super::upsert(&pool, "u1", "macobs", "MACOBS", 1.0));
        assert_eq!(super::load_all(&pool, "u1").len(), 0);
    }

    #[test]
    fn apply_matches_multi_word_phrase_at_any_position() {
        // The MACOBS regression: a "Main corps" rule must fire on
        // "मैं Main corps का IPO" when those two words appear together.
        let rules = vec![rule("Main corps", "MACOBS")];
        let out = super::apply("मैं Main corps का IPO", &rules);
        assert_eq!(out, "मैं MACOBS का IPO");
    }

    #[test]
    fn apply_longest_match_wins_at_overlapping_starts() {
        // Two rules could both match starting from "Main corps" — the longer
        // one ("Main corps detailed") should win when its full span is present.
        let rules = vec![
            rule("Main corps detailed", "MACOBS_DETAILED"),
            rule("Main corps", "MACOBS"),
        ];
        let out = super::apply("Please Main corps detailed today", &rules);
        assert_eq!(out, "Please MACOBS_DETAILED today");
    }

    #[test]
    fn apply_falls_back_to_shorter_when_longer_misses() {
        // "Main corps detailed" rule exists but transcript has only
        // "Main corps". Shorter "Main corps" rule must still fire.
        let rules = vec![
            rule("Main corps detailed", "MACOBS_DETAILED"),
            rule("Main corps", "MACOBS"),
        ];
        let out = super::apply("Today Main corps please", &rules);
        assert_eq!(out, "Today MACOBS please");
    }

    #[test]
    fn apply_handles_devanagari_alias() {
        // Raw STT alias contains Devanagari — must match a Devanagari span.
        let rules = vec![rule("मैं corps", "MACOBS")];
        let out = super::apply("मैं corps का password", &rules);
        assert_eq!(out, "MACOBS का password");
    }

    #[test]
    fn apply_preserves_punctuation_around_match() {
        let rules = vec![rule("Main corps", "MACOBS")];
        let out = super::apply("Hi (Main corps), today", &rules);
        // Leading "(" and trailing "," + space preserved
        assert_eq!(out, "Hi (MACOBS), today");
    }

    #[test]
    fn demote_until_evict() {
        let pool = mem_pool();
        super::upsert(&pool, "u1", "writen", "n8n", 1.0);
        assert_eq!(super::load_all(&pool, "u1").len(), 1);
        super::demote(&pool, "u1", "writen", "n8n", 1.5);
        assert_eq!(
            super::load_all(&pool, "u1").len(),
            0,
            "rule must evict when weight ≤ 0",
        );
    }
}
