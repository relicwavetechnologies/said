//! POST /v1/invite
//!
//! Sends an invite email via Resend (https://resend.com).
//!
//! Configuration (env vars on the backend host):
//!   RESEND_API_KEY  — required. Issue at resend.com/api-keys.
//!   RESEND_FROM     — optional. Defaults to "Said <onboarding@resend.dev>"
//!                     which works without DNS setup but lands in spam more often.
//!                     Set to "Said <hello@yourdomain.com>" once you've verified
//!                     a domain in Resend.
//!
//! When RESEND_API_KEY is missing, returns 503 with body
//! `{ "error": "email_not_configured" }` so the desktop client can fall
//! back to opening the user's mail app via `mailto:`.

use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::json;
use tracing::{info, warn};

use crate::AppState;

#[derive(Deserialize)]
pub struct InviteBody {
    pub to: String,
}

const SUBJECT: &str = "You should try Said";

const BODY_TEXT: &str = "\
Hey — I've been using Said to dictate and polish text.
It's quietly become my favourite way to write.

Thought you'd like it: https://said.app
";

const BODY_HTML: &str = "\
<div style=\"font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;font-size:15px;line-height:1.55;color:#1a1a1a;max-width:560px;\">
  <p>Hey —</p>
  <p>I've been using <strong>Said</strong> to dictate and polish text.<br/>It's quietly become my favourite way to write.</p>
  <p>Thought you'd like it: <a href=\"https://said.app\" style=\"color:#5b9b6a;font-weight:600;text-decoration:none;\">said.app</a></p>
</div>
";

pub async fn send(
    State(state): State<AppState>,
    Json(body):   Json<InviteBody>,
) -> (StatusCode, Json<serde_json::Value>) {
    let to = body.to.trim();
    if to.is_empty() || !to.contains('@') {
        return (StatusCode::BAD_REQUEST, Json(json!({ "error": "invalid_email" })));
    }

    let api_key = match std::env::var("RESEND_API_KEY") {
        Ok(k) if !k.trim().is_empty() => k,
        _ => {
            warn!("[invite] RESEND_API_KEY not set — returning email_not_configured");
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "email_not_configured" })),
            );
        }
    };

    let from = std::env::var("RESEND_FROM")
        .unwrap_or_else(|_| "Said <onboarding@resend.dev>".into());

    let payload = json!({
        "from":    from,
        "to":      [to],
        "subject": SUBJECT,
        "text":    BODY_TEXT,
        "html":    BODY_HTML,
    });

    let resp = state.http_client
        .post("https://api.resend.com/emails")
        .bearer_auth(&api_key)
        .json(&payload)
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => {
            info!("[invite] sent to {to}");
            (StatusCode::OK, Json(json!({ "ok": true })))
        }
        Ok(r) => {
            let status = r.status();
            let text   = r.text().await.unwrap_or_default();
            warn!("[invite] resend error {status}: {text}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "send_failed", "detail": text })),
            )
        }
        Err(e) => {
            warn!("[invite] network error: {e}");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "send_failed", "detail": e.to_string() })),
            )
        }
    }
}
