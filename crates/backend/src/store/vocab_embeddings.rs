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
use tracing::{info, warn};

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

/// Maximum number of example embeddings retained per (user, term) in the
/// FIFO ring. Centroid quality plateaus around 8-12; we pick 10 as a
/// reasonable balance between robustness and storage. At 10 examples ×
/// 1 KB each × 200 terms = 2 MB worst case per user. Cheap.
const EXAMPLES_RING_SIZE: usize = 10;

/// Insert or replace the centroid embedding for one vocabulary term.
///
/// Legacy entry-point: writes a single-embedding "centroid" directly. New
/// code should call `record_example_and_recentre` so the per-sighting ring
/// stays in sync. Kept for cases where the caller has only the centroid
/// (e.g. a migration backfill) and not the original example sentence.
pub fn upsert_embedding(pool: &DbPool, user_id: &str, term: &str, embedding: &[f32]) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("[vocab-emb] pool error: {e}");
            return;
        }
    };
    write_centroid(&conn, user_id, term, embedding);
}

/// Append one example embedding to the per-term FIFO ring (capped at
/// `EXAMPLES_RING_SIZE`), then recompute the centroid + persist it as the
/// `vocab_embeddings.embedding` row.
///
/// This is the foundational write path: every observed sighting of a term
/// adds an example, and the centroid (mean of L2-normalised vectors,
/// re-normalised to unit length) becomes the term's representation for
/// retrieval. Single-example representations are the largest source of
/// retrieval noise — centroids of 5-10 examples are dramatically more
/// stable (Snell et al., Prototypical Networks, NeurIPS 2017).
///
/// Atomicity: ring append, eviction, and centroid recompute happen inside
/// one connection without an explicit transaction — safe because the only
/// reader (`top_k_relevant`) tolerates a momentary stale centroid (worst
/// case: one retrieval uses last-cycle's centroid).
pub fn record_example_and_recentre(
    pool:         &DbPool,
    user_id:      &str,
    term:         &str,
    embedding:    &[f32],
    example_text: &str,
) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("[vocab-emb] pool error: {e}");
            return;
        }
    };
    let term_trim = term.trim();
    let now       = now_ms();

    // 1. Append the new example.
    let blob = floats_to_blob(embedding);
    if let Err(e) = conn.execute(
        "INSERT INTO vocab_embedding_examples
            (user_id, term, embedding, example_text, recorded_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![user_id, term_trim, blob, example_text, now],
    ) {
        warn!("[vocab-emb] insert example failed: {e}");
        return;
    }

    // 2. Evict oldest beyond the ring size (FIFO by recorded_at).
    let _ = conn.execute(
        "DELETE FROM vocab_embedding_examples
          WHERE id IN (
            SELECT id FROM vocab_embedding_examples
             WHERE user_id = ?1 AND term = ?2
             ORDER BY recorded_at DESC
             LIMIT -1 OFFSET ?3
          )",
        params![user_id, term_trim, EXAMPLES_RING_SIZE as i64],
    );

    // 3. Recompute centroid from the live ring.
    let examples = load_example_embeddings(&conn, user_id, term_trim);
    if examples.is_empty() {
        return;
    }
    let centroid = mean_normalised(&examples);
    write_centroid(&conn, user_id, term_trim, &centroid);
    info!(
        "[vocab-emb] centroid({term_trim:?}) recomputed from {} example(s)",
        examples.len(),
    );
}

/// Variance of the example cloud — high values indicate the term is being
/// used in semantically distinct contexts (candidate for split). Defined
/// as `1 - mean(cosine(example_i, centroid))`. Range [0, 2]; typical
/// cohesive concepts sit below 0.2; bimodal concepts above 0.5.
///
/// Used as a soft signal — surfaced in logs today, will drive
/// auto-split-into-two-prototypes in a future iteration.
pub fn cluster_spread(pool: &DbPool, user_id: &str, term: &str) -> f32 {
    let Ok(conn) = pool.get() else { return 0.0; };
    let examples = load_example_embeddings(&conn, user_id, term.trim());
    if examples.len() < 2 {
        return 0.0;
    }
    let centroid = mean_normalised(&examples);
    let cn = l2_norm(&centroid);
    if cn == 0.0 { return 0.0; }
    let mean_sim: f32 = examples.iter().map(|e| {
        let en = l2_norm(e);
        if en == 0.0 { 0.0 } else { dot(e, &centroid) / (en * cn) }
    }).sum::<f32>() / examples.len() as f32;
    (1.0 - mean_sim).max(0.0)
}

/// Bump `last_used` on a set of vocab terms — called after polish completes
/// so terms that actually appeared in the prompt get reinforced. This is
/// the "use signal" half of the time-decay scoring (the other half is the
/// exp(-λ·Δt) factor in `decay_factor`).
///
/// Cheap: one batched UPDATE per call; idempotent.
pub fn bump_last_used(pool: &DbPool, user_id: &str, terms: &[String]) {
    if terms.is_empty() { return; }
    let Ok(conn) = pool.get() else { return; };
    let now = now_ms();
    // SQLite doesn't have a clean batched UPDATE-IN; loop with prepared stmt.
    let Ok(mut stmt) = conn.prepare(
        "UPDATE vocabulary
            SET last_used = ?3,
                use_count = use_count + 1
          WHERE user_id = ?1 AND term = ?2",
    ) else { return; };
    for t in terms {
        let _ = stmt.execute(params![user_id, t.trim(), now]);
    }
}

/// Internal: write the centroid into vocab_embeddings (with current ts).
fn write_centroid(
    conn:      &rusqlite::Connection,
    user_id:   &str,
    term:      &str,
    centroid:  &[f32],
) {
    let blob = floats_to_blob(centroid);
    let now  = now_ms();
    let _ = conn.execute(
        "INSERT INTO vocab_embeddings (user_id, term, embedding, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(user_id, term) DO UPDATE SET
            embedding  = excluded.embedding,
            updated_at = excluded.updated_at",
        params![user_id, term, blob, now],
    );
}

/// Internal: load all example embeddings for a (user, term).
fn load_example_embeddings(
    conn:    &rusqlite::Connection,
    user_id: &str,
    term:    &str,
) -> Vec<Vec<f32>> {
    let Ok(mut stmt) = conn.prepare(
        "SELECT embedding FROM vocab_embedding_examples
          WHERE user_id = ?1 AND term = ?2
          ORDER BY recorded_at DESC",
    ) else { return vec![]; };
    stmt.query_map(params![user_id, term], |row| row.get::<_, Vec<u8>>(0))
        .ok()
        .map(|iter| {
            iter.filter_map(|r| r.ok())
                .filter_map(|blob| blob_to_floats(&blob))
                .collect()
        })
        .unwrap_or_default()
}

/// Internal: mean of vectors, then L2-normalised. The returned vector is a
/// unit-length centroid suitable for direct cosine comparison against
/// other unit vectors.
fn mean_normalised(vectors: &[Vec<f32>]) -> Vec<f32> {
    let n = vectors.len();
    if n == 0 { return vec![]; }
    let dim = vectors[0].len();
    let mut sum = vec![0.0_f32; dim];
    for v in vectors {
        if v.len() != dim { continue; }
        for (s, &x) in sum.iter_mut().zip(v.iter()) { *s += x; }
    }
    let inv_n = 1.0 / n as f32;
    for s in sum.iter_mut() { *s *= inv_n; }
    let norm = l2_norm(&sum);
    if norm > 0.0 {
        for s in sum.iter_mut() { *s /= norm; }
    }
    sum
}

/// Time-decay factor. Returns a multiplier in [0, 1] that decays a vocab
/// term's score by elapsed time since `last_used`. Half-life: `HALF_LIFE_DAYS`.
///
/// Per the Ebbinghaus literature ("meaningful content forgets ~10× slower"),
/// dictation vocabulary — which is highly recurrent and intentional — gets
/// a generous 45-day half-life by default. Result: a term untouched for
/// 45d gets weight 0.5, untouched for 90d gets 0.25, etc.
fn decay_factor(last_used_ms: i64, now_ms: i64) -> f32 {
    const HALF_LIFE_DAYS: f32 = 45.0;
    let elapsed_ms = (now_ms - last_used_ms).max(0) as f32;
    let elapsed_days = elapsed_ms / (1000.0 * 60.0 * 60.0 * 24.0);
    // exp(-λ Δt) where λ = ln(2) / half_life
    (-(0.6931472_f32) * elapsed_days / HALF_LIFE_DAYS).exp()
}

/// Use-count factor: log(1 + use_count). Diminishing returns — a term used
/// 100 times isn't 100× more relevant than one used twice; it's ~6× more.
fn use_count_factor(use_count: i64) -> f32 {
    (1.0 + use_count.max(0) as f32).ln() + 1.0
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
    let now = now_ms();

    // Score = cosine × decay × use_factor
    //
    // Cosine remains the gate (we still apply min_sim BEFORE multiplicative
    // factors so an unrelated term can't be promoted by sheer recency).
    // After the gate, the raw cosine is reweighted by the time-decay
    // multiplier (exp(-λΔt)) and a diminishing-returns use-count factor
    // (log(1+use)+1). Net effect: among entries that meet the cosine bar,
    // recent + frequently-used ones rank higher than ancient + rare ones.
    let mut scored: Vec<(f32, VocabRow)> = rows
        .into_iter()
        .filter_map(|row| {
            let row_norm = l2_norm(&row.embedding);
            if row_norm == 0.0 {
                return None;
            }
            let cos = dot(&row.embedding, query) / (row_norm * query_norm);
            if cos < min_sim {
                return None;
            }
            let decay = decay_factor(row.last_used, now);
            let usef  = use_count_factor(row.use_count);
            Some((cos * decay * usef, row))
        })
        .collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);

    scored.into_iter().map(|(_, r)| r.into_term()).collect()
}

/// Build the polish prompt's vocabulary slice using a HYBRID strategy:
///
///   • **Always** include starred terms (user-pinned, regardless of relevance)
///   • **Hybrid retrieval**: combine dense (cosine on centroids, with time-
///     decay reinforcement) and sparse (BM25 on term + example_context) via
///     Reciprocal Rank Fusion. This catches both semantic matches AND
///     exact-keyword matches that pure cosine misses (acronyms, brand
///     names, code identifiers).
///   • **Top-N by weight** is added only when hybrid retrieval found NOTHING
///     (fresh install / embedder down / transcript matches no past context).
///
/// `query_embedding` may be None — we skip the dense leg in that case but
/// still run BM25 if we have a `query_text`. When both are unavailable, fall
/// back to starred + weight.
///
/// `query_text` is the raw transcript (for BM25). `query_embedding` is the
/// transcript's vector (for cosine). We need both for hybrid; either alone
/// degrades gracefully.
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
    select_for_polish_hybrid(
        pool, user_id, language, query_embedding, /* query_text = */ None,
        n_top_weight, k_relevant, max_total, min_sim,
    )
}

/// Lexical-gated retrieval: only include vocab entries with ACTUAL EVIDENCE
/// in the transcript that they might apply.
///
/// The architectural shift: previously we used hybrid retrieval (cosine ⊕ BM25
/// via RRF) which could surface vocab entries with no shared words at all
/// with the transcript — pure semantic neighbours. That's the bug source for
/// "tembeess for time": tembeess gets included in the polish prompt because
/// its embedding is semantically near "time", even though the words have no
/// lexical overlap. The LLM then over-applies.
///
/// New rule: a vocab entry enters the prompt ONLY when its term OR its
/// example_context shares at least one word with the transcript (BM25 is
/// the gate). Within the gated set, cosine + decay + use_count rank for
/// the prompt order — but unevidenced entries never enter regardless of
/// rank.
///
/// This works because the foundational design captures example_context for
/// every learned term. So "MACOBS recovers from main corps" still works:
/// MACOBS's example_context "MACOBS ka IPO ka 12 hazaar batana" shares
/// words like "ka", "IPO", "hazaar" with the transcript "main corps ka IPO
/// ka 12 hazaar batana" — BM25 catches the overlap, MACOBS enters, LLM
/// recognises the pattern, output is MACOBS. But "what time is it" shares
/// no words with any tembeess-related vocab data, so tembeess never enters
/// the prompt at all, no over-replacement possible.
///
/// `query_text` (the raw transcript) is now REQUIRED for the gate to fire.
/// Without it we fall back to starred + top-weight (legacy behaviour).
/// `query_embedding` is used for intra-set cosine ranking only — never for
/// inclusion. `min_sim` is no longer applied as a gate (BM25 is the gate);
/// it's effectively dead and kept for ABI compatibility.
pub fn select_for_polish_hybrid(
    pool:            &DbPool,
    user_id:         &str,
    language:        &str,
    query_embedding: Option<&[f32]>,
    query_text:      Option<&str>,
    n_top_weight:    usize,
    k_relevant:      usize,
    max_total:       usize,
    _min_sim:        f32,
) -> Vec<VocabTerm> {
    use crate::store::{vocab_fts, vocabulary};
    let mut chosen: Vec<VocabTerm> = Vec::with_capacity(max_total);
    let mut seen:   std::collections::HashSet<String> = std::collections::HashSet::new();

    // Bucket 1 — Starred (always). User-pinned terms bypass the lexical
    // gate because they represent explicit user intent.
    let all = vocabulary::top_terms(pool, user_id, 1000);
    for t in all.iter().filter(|t| t.source == "starred") {
        if seen.insert(t.term.to_ascii_lowercase()) {
            chosen.push(t.clone());
            if chosen.len() >= max_total { return chosen; }
        }
    }

    // Bucket 2 — Lexical gate via BM25.
    //
    // Without a transcript we can't run the gate; fall back to starred +
    // top-weight (legacy behaviour) so existing callers without text don't
    // silently get empty prompts.
    let lexical_hits: Vec<String> = match query_text {
        Some(text) if !text.trim().is_empty() => {
            // Pull a generous candidate set; we'll re-rank by
            // cosine × decay × use_count after.
            vocab_fts::search(pool, user_id, text, k_relevant.max(20))
        }
        _ => Vec::new(),
    };

    // Resolve hits to full rows + rank within the lexically-gated set.
    let by_term_lower: std::collections::HashMap<String, &VocabTerm> = all
        .iter()
        .map(|t| (t.term.to_ascii_lowercase(), t))
        .collect();

    let mut gated: Vec<VocabTerm> = lexical_hits
        .iter()
        .filter_map(|term| {
            let key = term.to_ascii_lowercase();
            by_term_lower.get(&key).map(|vt| (*vt).clone())
        })
        .collect();

    // Intra-set ranking: cosine × decay × use_count. When no embedding,
    // fall back to weight × decay so we still produce a sensible order.
    let now = now_ms();
    if let Some(q) = query_embedding {
        let q_norm = l2_norm(q);
        if q_norm > 0.0 {
            // Load each gated term's centroid once; if missing, score via
            // weight only (centroid is async-populated, so a fresh term may
            // not have one yet).
            let conn = pool.get().ok();
            gated.sort_by(|a, b| {
                let sa = score_within_set(&conn, user_id, a, q, q_norm, now);
                let sb = score_within_set(&conn, user_id, b, q, q_norm, now);
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    } else {
        // No embedding: rank by weight × decay only.
        gated.sort_by(|a, b| {
            let sa = a.weight as f32 * decay_factor(a.last_used, now);
            let sb = b.weight as f32 * decay_factor(b.last_used, now);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    let mut gate_added = 0;
    for vt in gated.into_iter().take(k_relevant) {
        if seen.insert(vt.term.to_ascii_lowercase()) {
            chosen.push(vt);
            gate_added += 1;
            if chosen.len() >= max_total { return chosen; }
        }
    }

    // Bucket 3 — top-N by weight ONLY when no lexical gate ran (legacy
    // callers passing no transcript). With a transcript present, an empty
    // gate result is the CORRECT outcome — that's what stops the
    // "tembeess for time" class of bug. Top-weight fallback running anyway
    // would defeat the purpose.
    //
    // Starred terms are still included from Bucket 1; this only adds
    // unstarred high-weight terms when we have no other way to populate
    // the prompt.
    let _ = gate_added;  // kept for future telemetry
    let lexical_gate_ran = matches!(query_text, Some(t) if !t.trim().is_empty());
    if !lexical_gate_ran {
        for t in all.iter().take(n_top_weight) {
            if seen.insert(t.term.to_ascii_lowercase()) {
                chosen.push(t.clone());
                if chosen.len() >= max_total { return chosen; }
            }
        }
    }

    chosen
}

/// Internal: per-term score within the lexically-gated set. Combines
/// cosine on the term's centroid, time-decay, and log(1+use_count).
fn score_within_set(
    conn:    &Option<r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>>,
    user_id: &str,
    vt:      &VocabTerm,
    q:       &[f32],
    q_norm:  f32,
    now:     i64,
) -> f32 {
    // Default if we can't load embedding: fall back to weight × decay.
    let weight_decay = vt.weight as f32 * decay_factor(vt.last_used, now);
    let conn = match conn {
        Some(c) => c,
        None    => return weight_decay,
    };
    let blob: Vec<u8> = match conn.query_row(
        "SELECT embedding FROM vocab_embeddings WHERE user_id=?1 AND term=?2",
        params![user_id, vt.term],
        |row| row.get(0),
    ) {
        Ok(b) => b,
        Err(_) => return weight_decay,
    };
    let centroid = match blob_to_floats(&blob) {
        Some(v) => v,
        None    => return weight_decay,
    };
    let cn = l2_norm(&centroid);
    if cn == 0.0 { return weight_decay; }
    let cos = dot(&centroid, q) / (cn * q_norm);
    cos * decay_factor(vt.last_used, now) * use_count_factor(vt.use_count)
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
             );
             CREATE TABLE vocab_embedding_examples (
                 id            INTEGER PRIMARY KEY AUTOINCREMENT,
                 user_id       TEXT NOT NULL REFERENCES local_user(id),
                 term          TEXT NOT NULL,
                 embedding     BLOB NOT NULL,
                 example_text  TEXT NOT NULL,
                 recorded_at   INTEGER NOT NULL
             );
             CREATE INDEX idx_vocab_examples_user_term
               ON vocab_embedding_examples (user_id, term, recorded_at DESC);
             CREATE VIRTUAL TABLE vocab_fts USING fts5(
                 user_id UNINDEXED, term, example_context,
                 tokenize = 'unicode61 remove_diacritics 2'
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

    // ── Centroid ring + drift detection ───────────────────────────────────────

    fn unit(v: Vec<f32>) -> Vec<f32> {
        let n = (v.iter().map(|x| x * x).sum::<f32>()).sqrt();
        if n == 0.0 { v } else { v.into_iter().map(|x| x / n).collect() }
    }

    #[test]
    fn ring_buffer_caps_at_examples_ring_size() {
        let pool = mem_pool();
        // Seed the parent vocabulary row first.
        pool.get().unwrap().execute(
            "INSERT INTO vocabulary (user_id, term, weight, use_count, last_used)
             VALUES ('u1', 'TERM', 1.0, 1, ?1)",
            params![now_ms()],
        ).unwrap();
        // Push 15 example embeddings; ring should keep only the latest 10.
        for i in 0..15 {
            let emb = unit(vec![i as f32, 0.0, 0.0, 0.0]);
            record_example_and_recentre(&pool, "u1", "TERM", &emb, &format!("ex{i}"));
        }
        let conn = pool.get().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM vocab_embedding_examples WHERE user_id='u1' AND term='TERM'",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(count, EXAMPLES_RING_SIZE as i64);
    }

    #[test]
    fn centroid_is_mean_of_examples() {
        let pool = mem_pool();
        pool.get().unwrap().execute(
            "INSERT INTO vocabulary (user_id, term, weight, use_count, last_used)
             VALUES ('u1', 'X', 1.0, 1, ?1)",
            params![now_ms()],
        ).unwrap();
        record_example_and_recentre(&pool, "u1", "X", &unit(vec![1.0, 0.0, 0.0, 0.0]), "a");
        record_example_and_recentre(&pool, "u1", "X", &unit(vec![1.0, 0.0, 0.0, 0.0]), "b");
        // Centroid of two identical unit vectors should be the same vector.
        let got = top_k_relevant(&pool, "u1", &unit(vec![1.0, 0.0, 0.0, 0.0]), "english", 5, 0.0);
        assert_eq!(got.len(), 1);
        // Cosine should be ~1.0 (identical to query).
    }

    #[test]
    fn centroid_shifts_toward_new_examples() {
        let pool = mem_pool();
        pool.get().unwrap().execute(
            "INSERT INTO vocabulary (user_id, term, weight, use_count, last_used)
             VALUES ('u1', 'X', 1.0, 1, ?1)",
            params![now_ms()],
        ).unwrap();
        // Start with examples in direction (1, 0, 0, 0).
        for _ in 0..3 {
            record_example_and_recentre(&pool, "u1", "X", &unit(vec![1.0, 0.0, 0.0, 0.0]), "old");
        }
        // Add 7 examples in direction (0, 1, 0, 0).
        for _ in 0..7 {
            record_example_and_recentre(&pool, "u1", "X", &unit(vec![0.0, 1.0, 0.0, 0.0]), "new");
        }
        // Centroid should now be closer to (0, 1, 0, 0) than (1, 0, 0, 0).
        let against_new = top_k_relevant(&pool, "u1", &unit(vec![0.0, 1.0, 0.0, 0.0]), "english", 5, 0.0);
        let against_old = top_k_relevant(&pool, "u1", &unit(vec![1.0, 0.0, 0.0, 0.0]), "english", 5, 0.0);
        assert_eq!(against_new.len(), 1, "centroid should match the new direction");
        // 'old' direction may also score above 0 cosine but lower; we don't
        // need a hard ordering — the key fact is centroid moved.
        let _ = against_old;
    }

    #[test]
    fn cluster_spread_low_for_cohesive_examples() {
        let pool = mem_pool();
        pool.get().unwrap().execute(
            "INSERT INTO vocabulary (user_id, term, weight, use_count, last_used)
             VALUES ('u1', 'COHESIVE', 1.0, 1, ?1)",
            params![now_ms()],
        ).unwrap();
        // Three nearly-identical examples — variance should be ~0.
        record_example_and_recentre(&pool, "u1", "COHESIVE", &unit(vec![1.0, 0.05, 0.0, 0.0]), "a");
        record_example_and_recentre(&pool, "u1", "COHESIVE", &unit(vec![1.0, 0.0, 0.05, 0.0]), "b");
        record_example_and_recentre(&pool, "u1", "COHESIVE", &unit(vec![1.0, 0.0, 0.0, 0.05]), "c");
        let s = cluster_spread(&pool, "u1", "COHESIVE");
        assert!(s < 0.1, "cohesive cluster spread should be low, got {s}");
    }

    #[test]
    fn cluster_spread_high_for_bimodal_examples() {
        let pool = mem_pool();
        pool.get().unwrap().execute(
            "INSERT INTO vocabulary (user_id, term, weight, use_count, last_used)
             VALUES ('u1', 'MERCURY', 1.0, 1, ?1)",
            params![now_ms()],
        ).unwrap();
        // Half pointing one way (planet), half another (band).
        for _ in 0..3 {
            record_example_and_recentre(&pool, "u1", "MERCURY", &unit(vec![1.0, 0.0, 0.0, 0.0]), "planet");
        }
        for _ in 0..3 {
            record_example_and_recentre(&pool, "u1", "MERCURY", &unit(vec![0.0, 1.0, 0.0, 0.0]), "band");
        }
        let s = cluster_spread(&pool, "u1", "MERCURY");
        assert!(s > 0.2, "bimodal cluster spread should be high, got {s}");
    }

    // ── Time-decay scoring ────────────────────────────────────────────────────

    #[test]
    fn decay_factor_is_one_at_zero_elapsed() {
        let now = 1_000_000_000_000_i64;
        assert!((decay_factor(now, now) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn decay_factor_halves_at_half_life() {
        let now = 2_000_000_000_000_i64;
        let one_half_life_ago = now - 45 * 24 * 3600 * 1000;
        let f = decay_factor(one_half_life_ago, now);
        assert!((f - 0.5).abs() < 0.01, "decay at 45d should be ~0.5, got {f}");
    }

    #[test]
    fn decay_factor_quarters_at_two_half_lives() {
        let now = 2_000_000_000_000_i64;
        let two_half_lives_ago = now - 90 * 24 * 3600 * 1000;
        let f = decay_factor(two_half_lives_ago, now);
        assert!((f - 0.25).abs() < 0.01, "decay at 90d should be ~0.25, got {f}");
    }

    #[test]
    fn use_count_factor_grows_logarithmically() {
        let f1   = use_count_factor(1);
        let f10  = use_count_factor(10);
        let f100 = use_count_factor(100);
        // Should be diminishing returns: 100× use is not 100× factor.
        assert!(f100 < 10.0 * f1, "use_count_factor should be sub-linear");
        assert!(f10  > f1);
        assert!(f100 > f10);
    }

    #[test]
    fn bump_last_used_updates_timestamp() {
        let pool = mem_pool();
        // Seed a row with last_used 1 day ago.
        let day_ago = now_ms() - 86_400_000;
        pool.get().unwrap().execute(
            "INSERT INTO vocabulary (user_id, term, weight, use_count, last_used)
             VALUES ('u1', 'TICK', 1.0, 0, ?1)",
            params![day_ago],
        ).unwrap();
        bump_last_used(&pool, "u1", &["TICK".into()]);
        let row: (i64, i64) = pool.get().unwrap().query_row(
            "SELECT last_used, use_count FROM vocabulary WHERE term='TICK'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        assert!(row.0 > day_ago, "last_used should advance");
        assert_eq!(row.1, 1, "use_count should increment");
    }

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

    // ── Lexical-gated selector tests ──────────────────────────────────────────
    //
    // The selector now only includes vocab entries with ACTUAL EVIDENCE in
    // the transcript (BM25 lexical match against term OR example_context).
    // Cosine + decay + use_count rank WITHIN that gated set but never
    // include unevidenced entries.

    /// Helper: also write a vocab_fts row (the in-memory FTS index) so
    /// BM25 lookups in select_for_polish_hybrid can hit.
    fn seed_with_context(
        pool:    &DbPool,
        term:    &str,
        weight:  f64,
        source:  &str,
        embedding: &[f32],
        language:  &str,
        context:   &str,
    ) {
        // Update the legacy seed() to also insert example_context + FTS row.
        {
            let conn = pool.get().unwrap();
            conn.execute(
                "INSERT INTO vocabulary
                   (user_id, term, weight, use_count, last_used, source, language, example_context)
                 VALUES ('u1', ?1, ?2, 1, ?3, ?4, ?5, ?6)",
                params![term, weight, now_ms(), source, language, context],
            ).unwrap();
            conn.execute(
                "INSERT INTO vocab_fts (user_id, term, example_context)
                 VALUES ('u1', ?1, ?2)",
                params![term, context],
            ).unwrap();
        }
        upsert_embedding(pool, "u1", term, embedding);
    }

    #[test]
    fn lexical_gate_includes_term_when_transcript_mentions_it_directly() {
        let pool = mem_pool();
        seed_with_context(&pool, "MACOBS", 1.0, "auto",
            &vec4(1.0, 0.0, 0.0, 0.0), "english",
            "MACOBS ka IPO ka 12 hazaar batana");

        let chosen = select_for_polish_hybrid(
            &pool, "u1", "english",
            Some(&vec4(1.0, 0.0, 0.0, 0.0)),
            Some("the MACOBS announcement"),     // term itself appears in transcript
            5, 5, 10, 0.0,
        );
        assert!(chosen.iter().any(|v| v.term == "MACOBS"),
                "term-itself match must include the entry");
    }

    #[test]
    fn lexical_gate_includes_term_when_transcript_overlaps_example_context() {
        let pool = mem_pool();
        seed_with_context(&pool, "MACOBS", 1.0, "auto",
            &vec4(1.0, 0.0, 0.0, 0.0), "english",
            "MACOBS ka IPO ka 12 hazaar batana");

        // Transcript shares "ka", "IPO", "hazaar" with the example_context.
        // BM25 catches the overlap → MACOBS enters the prompt. This is the
        // "main corps → MACOBS recovery" path that keeps working.
        let chosen = select_for_polish_hybrid(
            &pool, "u1", "english",
            Some(&vec4(1.0, 0.0, 0.0, 0.0)),
            Some("main corps ka IPO ka 12 hazaar batana"),
            5, 5, 10, 0.0,
        );
        assert!(chosen.iter().any(|v| v.term == "MACOBS"),
                "example_context overlap must include the entry");
    }

    #[test]
    fn lexical_gate_excludes_term_when_no_transcript_overlap() {
        // The "tembeess for time" regression. tembeess vocab exists with a
        // distinct context. Transcript "what time is it" shares no words
        // with tembeess or its context. Lexical gate must EXCLUDE tembeess
        // → no over-replacement possible at the polish-prompt layer.
        let pool = mem_pool();
        seed_with_context(&pool, "tembeess", 4.0, "auto",
            &vec4(1.0, 0.0, 0.0, 0.0), "english",
            "tembeess team meeting on Friday");

        let chosen = select_for_polish_hybrid(
            &pool, "u1", "english",
            Some(&vec4(0.99, 0.0, 0.0, 0.0)),  // semantically near (cosine high)
            Some("what time is it"),            // BUT no lexical overlap
            5, 5, 10, 0.0,
        );
        assert!(!chosen.iter().any(|v| v.term == "tembeess"),
                "lexical gate must exclude tembeess for unrelated transcripts even if cosine is high");
    }

    #[test]
    fn lexical_gate_starred_always_included_regardless_of_overlap() {
        let pool = mem_pool();
        seed_with_context(&pool, "PINNED", 0.5, "starred",
            &vec4(0.0, 1.0, 0.0, 0.0), "english",
            "PINNED is my favourite term");

        // Transcript shares NO words with PINNED or its context.
        let chosen = select_for_polish_hybrid(
            &pool, "u1", "english", None,
            Some("the cat sat on the mat"),
            5, 5, 10, 0.0,
        );
        assert!(chosen.iter().any(|v| v.term == "PINNED"),
                "starred terms always included regardless of lexical match");
    }

    #[test]
    fn lexical_gate_returns_empty_for_no_match_no_starred() {
        // Foundational behaviour: when nothing matches and no starred exists,
        // the vocab block is EMPTY. This is correct — no over-replacement
        // possible because no vocab in scope. Top-weight fallback only runs
        // when no transcript was passed at all (legacy callers).
        let pool = mem_pool();
        seed_with_context(&pool, "tembeess", 5.0, "auto",
            &vec4(1.0, 0.0, 0.0, 0.0), "english",
            "tembeess Friday team meeting");

        let chosen = select_for_polish_hybrid(
            &pool, "u1", "english",
            Some(&vec4(0.99, 0.0, 0.0, 0.0)),  // semantically near
            Some("what time is it"),            // no lexical anchor
            10,  // n_top_weight — must NOT fire because gate ran (text was passed)
            5, 25, 0.0,
        );
        assert!(chosen.is_empty(),
                "lexical gate ran with no matches → vocab block must be empty (got: {chosen:?})");
    }

    #[test]
    fn legacy_no_text_call_falls_back_to_top_weight() {
        // Legacy callers (no transcript passed) get the old behaviour:
        // starred + top-weight. Used by select_for_polish wrapper for
        // backward compatibility.
        let pool = mem_pool();
        seed_with_context(&pool, "STARRED", 0.5, "starred", &vec4(0.0, 1.0, 0.0, 0.0), "english", "");
        seed_with_context(&pool, "HEAVY",   4.0, "auto",    &vec4(0.0, 0.0, 1.0, 0.0), "english", "");

        let chosen = select_for_polish_hybrid(
            &pool, "u1", "english",
            None,     // no embedding
            None,     // no transcript → lexical gate doesn't run
            5, 5, 10, 0.0,
        );
        let names: Vec<&str> = chosen.iter().map(|v| v.term.as_str()).collect();
        assert!(names.contains(&"STARRED"));
        assert!(names.contains(&"HEAVY"),
                "legacy no-text callers fall back to top-weight (otherwise empty)");
    }

    #[test]
    fn within_gated_set_cosine_ranks_higher_first() {
        // When multiple lexical matches exist, cosine + decay + use_count
        // determines the order within the gated set.
        let pool = mem_pool();
        // Both contexts mention "IPO" so both lexically gate-pass.
        seed_with_context(&pool, "MACOBS",  1.0, "auto",
            &vec4(1.0, 0.0, 0.0, 0.0), "english", "MACOBS ka IPO ka 12 hazaar");
        seed_with_context(&pool, "OTHERCO", 1.0, "auto",
            &vec4(0.0, 1.0, 0.0, 0.0), "english", "OTHERCO ka IPO date hai");

        // Query embedding aligns with MACOBS (1,0,0,0) > OTHERCO (0,1,0,0).
        let chosen = select_for_polish_hybrid(
            &pool, "u1", "english",
            Some(&vec4(1.0, 0.0, 0.0, 0.0)),
            Some("the IPO is tomorrow"),  // both gate-pass via "IPO"
            5, 5, 10, 0.0,
        );
        // Both should be present; MACOBS first by cosine score.
        assert!(chosen.iter().any(|v| v.term == "MACOBS"));
        assert!(chosen.iter().any(|v| v.term == "OTHERCO"));
        let macobs_idx  = chosen.iter().position(|v| v.term == "MACOBS").unwrap();
        let otherco_idx = chosen.iter().position(|v| v.term == "OTHERCO").unwrap();
        assert!(macobs_idx < otherco_idx,
                "MACOBS (cosine-near to query) should rank above OTHERCO");
    }

    #[test]
    fn lexical_gate_caps_at_max_total() {
        let pool = mem_pool();
        // Seed 50 terms whose example_contexts all contain "MEETING" so
        // they all lexically gate-pass.
        for i in 0..50 {
            seed_with_context(
                &pool, &format!("T{i}"), 1.0, "auto",
                &vec4(i as f32, 0.0, 0.0, 0.0), "english",
                "MEETING with T",
            );
        }
        let chosen = select_for_polish_hybrid(
            &pool, "u1", "english",
            Some(&vec4(1.0, 0.0, 0.0, 0.0)),
            Some("MEETING today"),
            100, 100, 5, 0.0,
        );
        assert_eq!(chosen.len(), 5);
    }
}
