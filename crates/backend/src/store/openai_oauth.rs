use rusqlite::params;
use serde::Serialize;
use tracing::info;

use super::DbPool;

#[derive(Debug, Clone, Serialize)]
pub struct OpenAIToken {
    pub user_id: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: i64,   // unix ms
    pub connected_at: i64, // unix ms
}

/// Return the stored token for this user, or None if not connected.
pub fn get_token(pool: &DbPool, user_id: &str) -> Option<OpenAIToken> {
    let conn = pool.get().ok()?;
    conn.query_row(
        "SELECT user_id, access_token, refresh_token, expires_at, connected_at
           FROM openai_oauth WHERE user_id = ?1",
        params![user_id],
        |row| {
            Ok(OpenAIToken {
                user_id: row.get(0)?,
                access_token: row.get(1)?,
                refresh_token: row.get(2)?,
                expires_at: row.get(3)?,
                connected_at: row.get(4)?,
            })
        },
    )
    .ok()
}

/// Insert or replace the token for this user and update llm_provider → "openai_codex".
pub fn save_token(
    pool: &DbPool,
    user_id: &str,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_at: i64,
) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return,
    };
    let now = super::now_ms();

    conn.execute(
        "INSERT OR REPLACE INTO openai_oauth (user_id, access_token, refresh_token, expires_at, connected_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![user_id, access_token, refresh_token, expires_at, now],
    ).ok();

    // Automatically switch llm_provider to openai_codex
    conn.execute(
        "UPDATE preferences SET llm_provider = 'openai_codex', updated_at = ?2 WHERE user_id = ?1",
        params![user_id, now],
    )
    .ok();

    info!("[openai_oauth] token saved, llm_provider → openai_codex");
}

/// Update only the access_token + expires_at (after a refresh).
pub fn update_token(
    pool: &DbPool,
    user_id: &str,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_at: i64,
) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return,
    };
    conn.execute(
        "UPDATE openai_oauth SET access_token = ?2, refresh_token = COALESCE(?3, refresh_token), expires_at = ?4 WHERE user_id = ?1",
        params![user_id, access_token, refresh_token, expires_at],
    ).ok();
}

/// Delete the token and revert llm_provider → "gateway".
pub fn delete_token(pool: &DbPool, user_id: &str) {
    let conn = match pool.get() {
        Ok(c) => c,
        Err(_) => return,
    };
    let now = super::now_ms();

    conn.execute(
        "DELETE FROM openai_oauth WHERE user_id = ?1",
        params![user_id],
    )
    .ok();

    conn.execute(
        "UPDATE preferences SET llm_provider = 'gateway', updated_at = ?2 WHERE user_id = ?1",
        params![user_id, now],
    )
    .ok();

    info!("[openai_oauth] token deleted, llm_provider → gateway");
}
