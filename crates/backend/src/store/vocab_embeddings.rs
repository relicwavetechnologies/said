//! Vocabulary embedding store + relevance-aware retrieval.
//!
//! At learn time we embed `"{term}. {example_context}"` and persist the
//! 256d vector here. At polish time we embed the transcript (already done
//! for RAG), then `top_k_relevant` cosine-ranks vocab against it. Combined
//! with `select_for_polish` (in this module), the polish prompt receives
//! a small relevance-scoped slice of vocab instead of the full table.
//!
//! Why this matters at scale: 200+ vocab entries × 80 tokens each ≈ 16 KB
//! of prompt on every recording. The LLM's attention degrades, latency
//! climbs, and the *one entry that matters* gets diluted. Vector retrieval
//! gives us the entries that match what the user just *said* — typically
//! 10–20 entries, all relevant.
//!
//! See `vectors.rs` for the parallel implementation on edit-event RAG.

use rusqlite::params;
use tracing::warn;

use super::{now_ms, DbPool};
use crate::embedder::gemini::{blob_to_floats, floats_to_blob};
use crate::store::vocabulary::VocabTerm;

/// One vocab entry plus its embedding, as loaded from the joined query.
struct VocabRow {
    term:            String,
    embedding:       Vec<f32>,
    weight:          f64,
    use_count:       i64,
    last_used:       i64,
    source:          String,
    example_context: Option<String>,
    term_type:       Option<String>,
}

impl VocabRow {
    fn into_term(self) -> VocabTerm {
        VocabTerm {
            term:            self.term,
            weight:          self.weight,
            use_count:       self.use_count,
            last_used:       self.last_used,
            source:          self.source,
            example_context: self.example_context,
            term_type:       self.term_type,
        }
    }
}

/// Insert or replace the embedding for one vocabulary term.
///
/// Called fire-and-forget after the term is upserted into `vocabulary` —
/// we don't block the learning path on the embedding round-trip. If the
/// embedder is unavailable (no API key, network down) the term still
/// works, it just won't participate in relevance retrieval until embedded.
pub fn upsert_embedding(pool: &DbPool, user_id: &str, term: &str, embedding: &[f32]) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("[vocab-emb] pool error: {e}");
            return;
        }
    };
    let blob = floats_to_blob(embedding);
    let now  = now_ms();
    let _ = conn.execute(
        "INSERT INTO vocab_embeddings (user_id, term, embedding, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(user_id, term) DO UPDATE SET
            embedding  = excluded.embedding,
            updated_at = excluded.updated_at",
        params![user_id, term.trim(), blob, now],
    );
}

/// Remove an embedding when its parent term is deleted from `vocabulary`.
/// Called by the vocabulary delete path; safe to call when no row exists.
pub fn delete(pool: &DbPool, user_id: &str, term: &str) {
    let Ok(conn) = pool.get() else { return; };
    let _ = conn.execute(
        "DELETE FROM vocab_embeddings WHERE user_id = ?1 AND term = ?2",
        params![user_id, term.trim()],
    );
}

/// Top-K vocab terms (with their full row data) by cosine similarity to
/// `query_embedding`. Filters out rows below `min_sim`. Returns at most K.
///
/// Filters by `language` — passes rows whose vocabulary.language is NULL
/// (legacy / language-agnostic) or matches.
pub fn top_k_relevant(
    pool:        &DbPool,
    user_id:     &str,
    query:       &[f32],
    language:    &str,
    k:           usize,
    min_sim:     f32,
) -> Vec<VocabTerm> {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    let mut stmt = match conn.prepare(
        "SELECT v.term, ve.embedding, v.weight, v.use_count, v.last_used,
                v.source, v.example_context, v.term_type
           FROM vocab_embeddings ve
           JOIN vocabulary v
             ON v.user_id = ve.user_id AND v.term = ve.term
          WHERE ve.user_id = ?1
            AND (v.language = ?2 OR v.language IS NULL)",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let rows: Vec<VocabRow> = stmt.query_map(params![user_id, language], |row| {
        let blob: Vec<u8> = row.get(1)?;
        Ok((
            row.get::<_, String>(0)?,
            blob,
            row.get::<_, f64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, Option<String>>(7)?,
        ))
    })
    .ok()
    .map(|iter| {
        iter.filter_map(|r| r.ok())
            .filter_map(|(term, blob, weight, uc, lu, src, ctx, ty)| {
                blob_to_floats(&blob).map(|embedding| VocabRow {
                    term,
                    embedding,
                    weight,
                    use_count: uc,
                    last_used: lu,
                    source: src,
                    example_context: ctx,
                    term_type: ty,
                })
            })
            .collect()
    })
    .unwrap_or_default();

    if rows.is_empty() {
        return vec![];
    }

    let query_norm = l2_norm(query);
    if query_norm == 0.0 {
        return vec![];
    }

    let mut scored: Vec<(f32, VocabRow)> = rows
        .into_iter()
        .filter_map(|row| {
            let row_norm = l2_norm(&row.embedding);
            if row_norm == 0.0 {
                return None;
            }
            let sim = dot(&row.embedding, query) / (row_norm * query_norm);
            if sim >= min_sim {
                Some((sim, row))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);

    scored.into_iter().map(|(_, r)| r.into_term()).collect()
}

/// Build the polish prompt's vocabulary slice using a hybrid strategy:
///
///   • **Always** include starred terms (user-pinned, regardless of weight)
///   • **If query embedding present + relevance hits found**, include the
///     top-K vector-relevant terms — these are the entries that actually
///     match what the user just said.
///   • **Top-N by weight** is added only when we have NO relevance hits
///     (fallback for fresh installs / embedder unavailable / first-time
///     short transcripts that can't match anything yet).
///   • Deduplicate by term, cap at `max_total`.
///
/// Why we don't ALWAYS include top-weight: it injects high-weight terms
/// (e.g. MACOBS) into prompts for unrelated transcripts ("main is here"),
/// where the LLM can over-apply them. Relevance retrieval already gives
/// us the entries that matter for the current transcript; adding "trust
/// fallback" entries when relevance succeeded only adds noise.
///
/// `query_embedding` may be None — in that case we skip the relevance
/// stage and fall back to starred + weight (the only way to populate
/// the prompt without an embedding).
pub fn select_for_polish(
    pool:            &DbPool,
    user_id:         &str,
    language:        &str,
    query_embedding: Option<&[f32]>,
    n_top_weight:    usize,
    k_relevant:      usize,
    max_total:       usize,
    min_sim:         f32,
) -> Vec<VocabTerm> {
    use crate::store::vocabulary;
    let mut chosen: Vec<VocabTerm> = Vec::with_capacity(max_total);
    let mut seen:   std::collections::HashSet<String> = std::collections::HashSet::new();

    // Bucket 1 — starred (user-pinned). Always in, regardless of relevance.
    let all = vocabulary::top_terms(pool, user_id, 1000);
    for t in all.iter().filter(|t| t.source == "starred") {
        if seen.insert(t.term.to_ascii_lowercase()) {
            chosen.push(t.clone());
            if chosen.len() >= max_total { return chosen; }
        }
    }

    // Bucket 2 — top-K by cosine similarity to query.
    let mut relevance_hits: Vec<VocabTerm> = Vec::new();
    if let Some(q) = query_embedding {
        relevance_hits = top_k_relevant(pool, user_id, q, language, k_relevant, min_sim);
        for t in &relevance_hits {
            if seen.insert(t.term.to_ascii_lowercase()) {
                chosen.push(t.clone());
                if chosen.len() >= max_total { return chosen; }
            }
        }
    }

    // Bucket 3 — top-N by weight, ONLY when relevance retrieval found nothing.
    // This is the fallback path: fresh install, embedder unavailable, or a
    // transcript that doesn't match any past context. It's better to include
    // the most-trusted entries than to send an empty vocab block — the LLM
    // can still recognise verbatim matches.
    if relevance_hits.is_empty() {
        for t in all.iter().take(n_top_weight) {
            if seen.insert(t.term.to_ascii_lowercase()) {
                chosen.push(t.clone());
                if chosen.len() >= max_total { return chosen; }
            }
        }
    }

    chosen
}

// ── Math helpers (kept local — same impl as vectors.rs) ───────────────────────

#[inline]
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[inline]
fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use r2d2_sqlite::SqliteConnectionManager;

    fn mem_pool() -> DbPool {
        let mgr  = SqliteConnectionManager::memory();
        // r2d2's :memory: connections are per-connection isolated, so multi-
        // conn pools each get a fresh empty DB. Single-conn is correct; the
        // helpers in this module that take `pool: &DbPool` must be careful
        // never to hold a conn open while calling another store fn.
        let pool = r2d2::Pool::builder().max_size(1).build(mgr).unwrap();
        pool.get().unwrap().execute_batch(
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
                 term_type        TEXT,
                 UNIQUE(user_id, term)
             );
             CREATE TABLE vocab_embeddings (
                 user_id    TEXT NOT NULL REFERENCES local_user(id),
                 term       TEXT NOT NULL,
                 embedding  BLOB NOT NULL,
                 updated_at INTEGER NOT NULL,
                 UNIQUE(user_id, term)
             );"
        ).unwrap();
        pool
    }

    fn seed(pool: &DbPool, term: &str, weight: f64, source: &str, embedding: &[f32], language: &str) {
        // Scope the conn so it's released before upsert_embedding takes its
        // own from the pool (max_size=1 in tests would deadlock otherwise).
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO vocabulary (user_id, term, weight, use_count, last_used, source, language)
                 VALUES ('u1', ?1, ?2, 1, ?3, ?4, ?5)",
                params![term, weight, now_ms(), source, language],
            ).unwrap();
        }
        upsert_embedding(pool, "u1", term, embedding);
    }

    /// Build a tiny 4-d unit-ish vector for testing cosine math.
    fn vec4(a: f32, b: f32, c: f32, d: f32) -> Vec<f32> { vec![a, b, c, d] }

    #[test]
    fn upsert_and_retrieve_round_trip() {
        let pool = mem_pool();
        seed(&pool, "MACOBS", 2.0, "auto", &vec4(1.0, 0.0, 0.0, 0.0), "english");
        let got = top_k_relevant(&pool, "u1", &vec4(1.0, 0.0, 0.0, 0.0), "english", 5, 0.0);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].term, "MACOBS");
    }

    #[test]
    fn cosine_ordering_correct() {
        let pool = mem_pool();
        // Aligned with query → high similarity
        seed(&pool, "FINANCE", 1.0, "auto", &vec4(1.0, 0.0, 0.0, 0.0), "english");
        // Orthogonal → zero
        seed(&pool, "COOKING", 1.0, "auto", &vec4(0.0, 1.0, 0.0, 0.0), "english");
        // Slightly aligned
        seed(&pool, "ECONOMY", 1.0, "auto", &vec4(0.7, 0.3, 0.0, 0.0), "english");

        let got = top_k_relevant(&pool, "u1", &vec4(1.0, 0.0, 0.0, 0.0), "english", 5, 0.0);
        assert_eq!(got.len(), 3);
        assert_eq!(got[0].term, "FINANCE");   // sim = 1.0
        assert_eq!(got[1].term, "ECONOMY");   // sim ≈ 0.92
        assert_eq!(got[2].term, "COOKING");   // sim = 0.0
    }

    #[test]
    fn min_sim_filters_out_low_relevance() {
        let pool = mem_pool();
        seed(&pool, "FINANCE", 1.0, "auto", &vec4(1.0, 0.0, 0.0, 0.0), "english");
        seed(&pool, "COOKING", 1.0, "auto", &vec4(0.0, 1.0, 0.0, 0.0), "english");
        let got = top_k_relevant(&pool, "u1", &vec4(1.0, 0.0, 0.0, 0.0), "english", 5, 0.5);
        assert_eq!(got.len(), 1);   // COOKING filtered (sim = 0.0)
        assert_eq!(got[0].term, "FINANCE");
    }

    #[test]
    fn delete_clears_embedding() {
        let pool = mem_pool();
        seed(&pool, "TERM", 1.0, "auto", &vec4(1.0, 0.0, 0.0, 0.0), "english");
        delete(&pool, "u1", "TERM");
        assert!(top_k_relevant(&pool, "u1", &vec4(1.0, 0.0, 0.0, 0.0), "english", 5, 0.0).is_empty());
    }

    #[test]
    fn upsert_replaces_existing_embedding() {
        let pool = mem_pool();
        seed(&pool, "TERM", 1.0, "auto", &vec4(1.0, 0.0, 0.0, 0.0), "english");
        // Re-embed with a different vector.
        upsert_embedding(&pool, "u1", "TERM", &vec4(0.0, 1.0, 0.0, 0.0));
        // Original-direction query should now miss; new-direction should hit.
        assert!(top_k_relevant(&pool, "u1", &vec4(1.0, 0.0, 0.0, 0.0), "english", 5, 0.5).is_empty());
        assert_eq!(
            top_k_relevant(&pool, "u1", &vec4(0.0, 1.0, 0.0, 0.0), "english", 5, 0.5).len(),
            1,
        );
    }

    // ── Hybrid selector tests ──────────────────────────────────────────────────

    #[test]
    fn selector_includes_starred_and_relevance_when_query_matches() {
        let pool = mem_pool();
        seed(&pool, "STARRED",  0.5, "starred", &vec4(0.0, 1.0, 0.0, 0.0), "english");
        seed(&pool, "TOPWEIGHT",4.0, "auto",    &vec4(0.0, 0.0, 1.0, 0.0), "english");
        seed(&pool, "RELEVANT", 1.0, "auto",    &vec4(1.0, 0.0, 0.0, 0.0), "english");
        seed(&pool, "OTHER",    1.0, "auto",    &vec4(0.0, 0.5, 0.5, 0.0), "english");

        let chosen = select_for_polish(
            &pool, "u1", "english",
            Some(&vec4(1.0, 0.0, 0.0, 0.0)),
            /* n_top_weight */ 1,
            /* k_relevant   */ 1,
            /* max_total    */ 10,
            /* min_sim      */ 0.3,
        );

        let names: Vec<&str> = chosen.iter().map(|v| v.term.as_str()).collect();
        assert!(names.contains(&"STARRED"),  "starred always included");
        assert!(names.contains(&"RELEVANT"), "vector-relevant included");
        // TOPWEIGHT must NOT appear — relevance found a hit, no fallback fires.
        // This is the "main → MACOBS" guard: high-weight terms don't pollute
        // prompts for transcripts where they don't match.
        assert!(!names.contains(&"TOPWEIGHT"),
                "top-weight must NOT inject when relevance found hits");
    }

    #[test]
    fn selector_falls_back_to_top_weight_when_no_relevance() {
        // The exact scenario for the over-replacement bug: user dictates
        // a short utterance like "main is here". Relevance retrieval finds
        // nothing (no MACOBS context match). Top-weight kicks in as
        // fallback so the polish prompt isn't empty.
        let pool = mem_pool();
        seed(&pool, "STARRED",  0.5, "starred", &vec4(0.0, 1.0, 0.0, 0.0), "english");
        seed(&pool, "MACOBS",   4.0, "auto",    &vec4(1.0, 0.0, 0.0, 0.0), "english");

        // Query is orthogonal to MACOBS embedding — no relevance hit.
        let chosen = select_for_polish(
            &pool, "u1", "english",
            Some(&vec4(0.0, 0.0, 0.0, 1.0)),  // unrelated query
            /* n_top_weight */ 5,
            /* k_relevant   */ 5,
            /* max_total    */ 10,
            /* min_sim      */ 0.5,
        );

        let names: Vec<&str> = chosen.iter().map(|v| v.term.as_str()).collect();
        assert!(names.contains(&"STARRED"));
        assert!(names.contains(&"MACOBS"), "top-weight fallback when no relevance");
    }

    #[test]
    fn selector_does_not_inject_irrelevant_top_weight_when_relevance_succeeded() {
        // The "main → MACOBS" regression case at the selector level.
        // User has high-weight MACOBS. They dictate a transcript that
        // matches an entirely different vocab entry (ITALIAN). MACOBS must
        // NOT appear in the chosen list.
        let pool = mem_pool();
        seed(&pool, "MACOBS",  5.0, "auto", &vec4(1.0, 0.0, 0.0, 0.0), "english");
        seed(&pool, "ITALIAN", 1.0, "auto", &vec4(0.0, 1.0, 0.0, 0.0), "english");

        let chosen = select_for_polish(
            &pool, "u1", "english",
            Some(&vec4(0.0, 1.0, 0.0, 0.0)),  // matches ITALIAN, not MACOBS
            5, 5, 10, 0.5,
        );
        let names: Vec<&str> = chosen.iter().map(|v| v.term.as_str()).collect();
        assert!(names.contains(&"ITALIAN"));
        assert!(!names.contains(&"MACOBS"),
                "high-weight MACOBS must NOT pollute an Italian-cooking prompt");
    }

    #[test]
    fn selector_dedupes_when_buckets_overlap() {
        let pool = mem_pool();
        // One term that is BOTH high-weight AND high-relevance.
        seed(&pool, "BOTH", 5.0, "auto", &vec4(1.0, 0.0, 0.0, 0.0), "english");
        let chosen = select_for_polish(
            &pool, "u1", "english",
            Some(&vec4(1.0, 0.0, 0.0, 0.0)),
            5, 5, 10, 0.0,
        );
        assert_eq!(chosen.len(), 1, "duplicate must be deduped");
    }

    #[test]
    fn selector_caps_at_max_total() {
        let pool = mem_pool();
        for i in 0..50 {
            seed(&pool, &format!("T{i}"), 1.0, "auto",
                 &vec4(i as f32, 0.0, 0.0, 0.0), "english");
        }
        let chosen = select_for_polish(
            &pool, "u1", "english",
            Some(&vec4(1.0, 0.0, 0.0, 0.0)),
            100, 100, 5, 0.0,
        );
        assert_eq!(chosen.len(), 5);
    }

    #[test]
    fn selector_works_without_query_embedding() {
        // When embedder is unavailable, fall back to starred + weight.
        let pool = mem_pool();
        seed(&pool, "STARRED",  0.5, "starred", &vec4(1.0, 0.0, 0.0, 0.0), "english");
        seed(&pool, "TOPWEIGHT",4.0, "auto",    &vec4(0.0, 1.0, 0.0, 0.0), "english");
        let chosen = select_for_polish(
            &pool, "u1", "english",
            None,  // no embedding
            5, 5, 10, 0.0,
        );
        let names: Vec<&str> = chosen.iter().map(|v| v.term.as_str()).collect();
        assert!(names.contains(&"STARRED"));
        assert!(names.contains(&"TOPWEIGHT"));
    }
}
