//! POST /v1/pre-embed
//!
//! Speculative embedding — fired by Tauri the moment the user stops speaking
//! (CloseStream sent to Deepgram), while the 500ms drain window is still open.
//!
//! Returns 202 immediately; embedding runs fire-and-forget in the background.
//! When the full /v1/voice/polish request arrives ~500ms later, the embedding
//! is already in the SQLite cache → 0ms embed wait instead of 250–300ms.

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use tracing::{debug, info};

use crate::{embedder::gemini, store::prefs::get_prefs, AppState};

#[derive(Deserialize)]
pub struct PreEmbedBody {
    pub text: String,
}

pub async fn handler(
    State(state): State<AppState>,
    Json(body): Json<PreEmbedBody>,
) -> StatusCode {
    let text = body.text.trim().to_string();
    if text.is_empty() {
        return StatusCode::BAD_REQUEST;
    }

    debug!("[pre-embed] received {} chars — spawning background embed", text.len());

    let pool        = state.pool.clone();
    let http_client = state.http_client.clone();
    let user_id     = state.default_user_id.as_str().to_string();

    // Fire-and-forget — caller gets 202 immediately, embedding stores in SQLite cache.
    tokio::spawn(async move {
        let gemini_key = get_prefs(&pool, &user_id)
            .and_then(|p| p.gemini_api_key)
            .or_else(|| std::env::var("GEMINI_API_KEY").ok())
            .unwrap_or_default();

        if gemini_key.is_empty() {
            debug!("[pre-embed] skipped — no Gemini key");
            return;
        }

        let start = std::time::Instant::now();
        match gemini::embed(&http_client, &pool, &text, &gemini_key).await {
            Some(_) => info!("[pre-embed] cached in {}ms ({} chars)", start.elapsed().as_millis(), text.len()),
            None    => debug!("[pre-embed] embed returned None"),
        }
    });

    StatusCode::ACCEPTED
}
