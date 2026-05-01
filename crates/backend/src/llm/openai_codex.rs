//! OpenAI Codex Responses API client.
//!
//! Uses the same OAuth token that the official Codex CLI uses.
//! Endpoint: https://chatgpt.com/backend-api/codex/responses
//!
//! The payload format differs from standard Chat Completions:
//!   - `instructions` = system prompt
//!   - `input` = array of {type, role, content} (user/assistant messages only)
//! SSE events use `response.output_text.delta` (not `choices[].delta.content`).

use reqwest::Client;
use serde_json::json;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};
use std::time::Instant;

pub use super::PolishResult;

const CODEX_ENDPOINT: &str = "https://chatgpt.com/backend-api/codex/responses";
const TOKEN_URL:       &str = "https://auth.openai.com/oauth/token";
const CLIENT_ID:       &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// Available model IDs via the Codex Responses endpoint.
pub const MODEL_SMART:  &str = "gpt-5.4";
pub const MODEL_MINI:   &str = "gpt-5.4-mini";

/// Stream a polish operation through the Codex endpoint.
/// `model` should be `MODEL_SMART` or `MODEL_MINI`.
pub async fn stream_polish(
    client:        &Client,
    access_token:  &str,
    model:         &str,
    system_prompt: &str,
    user_message:  &str,
    token_tx:      mpsc::Sender<String>,
) -> Result<PolishResult, String> {
    let start = Instant::now();

    // Build Codex-format payload
    let payload = json!({
        "model":               model,
        "instructions":        system_prompt,
        "input": [
            { "type": "message", "role": "user", "content": user_message }
        ],
        "tools":               [],
        "tool_choice":         "auto",
        "parallel_tool_calls": false,
        "reasoning":           { "summary": "auto" },
        "store":               false,
        "stream":              true,
        "prompt_cache_key":    uuid::Uuid::new_v4().to_string(),
    });

    info!("[codex] POST {CODEX_ENDPOINT} model={model}");

    let resp = client
        .post(CODEX_ENDPOINT)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("codex request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        warn!("[codex] HTTP {status}: {body}");
        return Err(format!("Codex API error {status}: {body}"));
    }

    // Parse SSE stream
    let mut polished = String::new();
    let bytes        = resp.bytes().await.map_err(|e| format!("codex read failed: {e}"))?;
    let text         = String::from_utf8_lossy(&bytes);

    for line in text.lines() {
        if !line.starts_with("data: ") { continue; }
        let data = &line[6..];
        if data == "[DONE]" { break; }

        let Ok(evt) = serde_json::from_str::<serde_json::Value>(data) else { continue };

        match evt.get("type").and_then(|t| t.as_str()) {
            Some("response.output_text.delta") => {
                let delta = evt.get("delta")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");
                if !delta.is_empty() {
                    polished.push_str(delta);
                    // Best-effort send; drop if receiver closed
                    let _ = token_tx.try_send(delta.to_string());
                }
            }
            Some("response.completed") => {
                debug!("[codex] response.completed");
                break;
            }
            _ => {}
        }
    }

    let polish_ms = start.elapsed().as_millis() as u64;
    info!("[codex] done in {polish_ms}ms, {} chars", polished.len());
    Ok(PolishResult { polished, polish_ms })
}

// ── Token refresh ─────────────────────────────────────────────────────────────

pub struct RefreshedToken {
    pub access_token:  String,
    pub refresh_token: String,
    pub expires_at_ms: i64,
}

/// Exchange a refresh_token for a new access_token.
pub async fn refresh_token(client: &Client, refresh_tok: &str) -> Result<RefreshedToken, String> {
    let payload = json!({
        "client_id":     CLIENT_ID,
        "refresh_token": refresh_tok,
        "grant_type":    "refresh_token",
    });

    let resp = client
        .post(TOKEN_URL)
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("refresh request failed: {e}"))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body   = resp.text().await.unwrap_or_default();
        return Err(format!("refresh error {status}: {body}"));
    }

    let v: serde_json::Value = resp.json().await
        .map_err(|e| format!("refresh parse failed: {e}"))?;

    let access_token  = v["access_token"].as_str().unwrap_or("").to_string();
    let refresh_token = v["refresh_token"].as_str()
        .unwrap_or(refresh_tok).to_string();
    let expires_in    = v["expires_in"].as_i64().unwrap_or(864_000);
    let expires_at_ms = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64)
        + expires_in * 1000;

    Ok(RefreshedToken { access_token, refresh_token, expires_at_ms })
}
