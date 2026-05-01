//! Preference vector store — insert + cosine-similarity KNN.
//!
//! Embeddings are stored in `preference_vectors` as 768×f32 little-endian BLOBs.
//! KNN is computed entirely in Rust (dot-product after L2-normalisation).
//! At personal scale (< 1 000 vectors) this is effectively instant.

use rusqlite::params;
use tracing::warn;

use super::DbPool;
use crate::{
    embedder::gemini::{blob_to_floats, floats_to_blob},
    llm::prompt::RagExample,
};

/// A row fetched from `preference_vectors` + its linked edit event.
struct VectorRow {
    embedding:   Vec<f32>,
    ai_output:   String,
    user_kept:   String,
}

/// Insert (or replace) a preference vector for an edit event.
pub fn upsert_vector(pool: &DbPool, user_id: &str, edit_event_id: &str, embedding: &[f32]) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(e) => {
            warn!("[vectors] pool error: {e}");
            return;
        }
    };

    let blob = floats_to_blob(embedding);
    let _ = conn.execute(
        "INSERT OR REPLACE INTO preference_vectors (user_id, edit_event_id, embedding)
         VALUES (?1, ?2, ?3)",
        params![user_id, edit_event_id, blob],
    );
}

/// Retrieve the top-`k` most similar past edit examples for `query_embedding`.
///
/// Returns at most `k` examples where cosine similarity exceeds `min_sim`.
pub fn retrieve_similar(
    pool: &DbPool,
    user_id: &str,
    query: &[f32],
    k: usize,
    min_sim: f32,
) -> Vec<RagExample> {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return vec![],
    };

    // Pull all vectors for this user along with their edit text
    let mut stmt = match conn.prepare(
        "SELECT pv.embedding, ee.ai_output, ee.user_kept
         FROM preference_vectors pv
         JOIN edit_events ee ON ee.id = pv.edit_event_id
         WHERE pv.user_id = ?1",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };

    let rows: Vec<VectorRow> = stmt
        .query_map(params![user_id], |row| {
            let blob: Vec<u8> = row.get(0)?;
            let ai_output: String = row.get(1)?;
            let user_kept: String = row.get(2)?;
            Ok((blob, ai_output, user_kept))
        })
        .ok()
        .map(|iter| {
            iter.filter_map(|r| r.ok())
                .filter_map(|(blob, ai, kept)| {
                    blob_to_floats(&blob).map(|embedding| VectorRow {
                        embedding,
                        ai_output: ai,
                        user_kept: kept,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    if rows.is_empty() {
        return vec![];
    }

    // Compute cosine similarity for each row, keep top-k above threshold
    let query_norm = l2_norm(query);
    if query_norm == 0.0 {
        return vec![];
    }

    let mut scored: Vec<(f32, &VectorRow)> = rows
        .iter()
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

    // Sort descending by similarity
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(k);

    scored
        .into_iter()
        .map(|(_, row)| RagExample {
            ai_output: row.ai_output.clone(),
            user_kept: row.user_kept.clone(),
        })
        .collect()
}

/// Insert a new edit_event row. Returns the generated UUID.
pub fn insert_edit_event(
    pool: &DbPool,
    user_id: &str,
    recording_id: Option<&str>,
    transcript: &str,
    ai_output: &str,
    user_kept: &str,
    target_app: Option<&str>,
) -> Option<String> {
    let conn = pool.get().ok()?;
    let id = uuid::Uuid::new_v4().to_string();
    let now_ms = super::now_ms();

    conn.execute(
        "INSERT INTO edit_events
         (id, user_id, recording_id, timestamp_ms, transcript, ai_output, user_kept, target_app)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![id, user_id, recording_id, now_ms, transcript, ai_output, user_kept, target_app],
    ).ok()?;

    Some(id)
}

// ── Math helpers ──────────────────────────────────────────────────────────────

#[inline]
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[inline]
fn l2_norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}
