use rusqlite::params;
use serde::{Deserialize, Serialize};
use tracing::info;

use super::{DbPool, now_ms};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id:                String,
    pub user_id:           String,
    pub timestamp_ms:      i64,
    pub transcript:        String,
    pub polished:          String,
    pub final_text:        Option<String>,
    pub word_count:        i64,
    pub recording_seconds: f64,
    pub model_used:        String,
    pub confidence:        Option<f64>,
    pub transcribe_ms:     Option<i64>,
    pub embed_ms:          Option<i64>,
    pub polish_ms:         Option<i64>,
    pub target_app:        Option<String>,
    pub edit_count:        i64,
    pub source:            String,
}

pub struct InsertRecording<'a> {
    pub id:                &'a str,
    pub user_id:           &'a str,
    pub transcript:        &'a str,
    pub polished:          &'a str,
    pub word_count:        i64,
    pub recording_seconds: f64,
    pub model_used:        &'a str,
    pub confidence:        Option<f64>,
    pub transcribe_ms:     Option<i64>,
    pub embed_ms:          Option<i64>,
    pub polish_ms:         Option<i64>,
    pub target_app:        Option<&'a str>,
    pub source:            &'a str,
}

pub fn insert_recording(pool: &DbPool, rec: InsertRecording<'_>) -> Option<()> {
    let conn = pool.get().ok()?;
    conn.execute(
        "INSERT INTO recordings
         (id, user_id, timestamp_ms, transcript, polished, word_count, recording_seconds,
          model_used, confidence, transcribe_ms, embed_ms, polish_ms, target_app, source)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            rec.id, rec.user_id, now_ms(),
            rec.transcript, rec.polished,
            rec.word_count, rec.recording_seconds,
            rec.model_used, rec.confidence,
            rec.transcribe_ms, rec.embed_ms, rec.polish_ms,
            rec.target_app, rec.source,
        ],
    )
    .ok()?;
    Some(())
}

pub fn list_recordings(pool: &DbPool, user_id: &str, limit: i64, before_ms: Option<i64>) -> Vec<Recording> {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return vec![],
    };
    let cutoff = before_ms.unwrap_or(i64::MAX);
    let mut stmt = conn
        .prepare(
            "SELECT id, user_id, timestamp_ms, transcript, polished, final_text,
                    word_count, recording_seconds, model_used, confidence,
                    transcribe_ms, embed_ms, polish_ms, target_app, edit_count, source
             FROM recordings
             WHERE user_id = ?1 AND timestamp_ms < ?2
             ORDER BY timestamp_ms DESC
             LIMIT ?3",
        )
        .expect("failed to prepare statement");

    stmt.query_map(params![user_id, cutoff, limit], |row| {
        Ok(Recording {
            id:                row.get(0)?,
            user_id:           row.get(1)?,
            timestamp_ms:      row.get(2)?,
            transcript:        row.get(3)?,
            polished:          row.get(4)?,
            final_text:        row.get(5)?,
            word_count:        row.get(6)?,
            recording_seconds: row.get(7)?,
            model_used:        row.get(8)?,
            confidence:        row.get(9)?,
            transcribe_ms:     row.get(10)?,
            embed_ms:          row.get(11)?,
            polish_ms:         row.get(12)?,
            target_app:        row.get(13)?,
            edit_count:        row.get(14)?,
            source:            row.get(15)?,
        })
    })
    .map(|rows| rows.flatten().collect())
    .unwrap_or_default()
}

/// Delete recordings older than 7 days. Called on a background timer.
pub fn cleanup_old_recordings(pool: &DbPool) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return,
    };
    let seven_days_ms = 7i64 * 86_400 * 1_000;
    let cutoff = now_ms() - seven_days_ms;
    match conn.execute(
        "DELETE FROM recordings WHERE timestamp_ms < ?1",
        params![cutoff],
    ) {
        Ok(n) if n > 0 => info!("cleaned up {n} old recordings"),
        _ => {}
    }
}

pub fn get_recording(pool: &DbPool, id: &str) -> Option<Recording> {
    let conn = pool.get().ok()?;
    conn.query_row(
        "SELECT id, user_id, timestamp_ms, transcript, polished, final_text,
                word_count, recording_seconds, model_used, confidence,
                transcribe_ms, embed_ms, polish_ms, target_app, edit_count, source
         FROM recordings WHERE id = ?1",
        params![id],
        |row| {
            Ok(Recording {
                id:                row.get(0)?,
                user_id:           row.get(1)?,
                timestamp_ms:      row.get(2)?,
                transcript:        row.get(3)?,
                polished:          row.get(4)?,
                final_text:        row.get(5)?,
                word_count:        row.get(6)?,
                recording_seconds: row.get(7)?,
                model_used:        row.get(8)?,
                confidence:        row.get(9)?,
                transcribe_ms:     row.get(10)?,
                embed_ms:          row.get(11)?,
                polish_ms:         row.get(12)?,
                target_app:        row.get(13)?,
                edit_count:        row.get(14)?,
                source:            row.get(15)?,
            })
        },
    )
    .ok()
}

pub fn apply_edit_feedback(pool: &DbPool, recording_id: &str, user_kept: &str) -> Option<()> {
    let conn = pool.get().ok()?;
    conn.execute(
        "UPDATE recordings
         SET final_text = ?1, edit_count = edit_count + 1
         WHERE id = ?2",
        params![user_kept, recording_id],
    ).ok()?;
    Some(())
}
