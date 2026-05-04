//! Shared-secret bearer-token middleware.
//!
//! Tauri generates a UUID at startup, passes it to the backend via the
//! `POLISH_SHARED_SECRET` environment variable, and sends it as
//! `Authorization: Bearer <uuid>` on every request.
//!
//! This prevents any other process on the machine from probing the daemon's
//! localhost port.

use axum::{
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::Response,
};

use crate::AppState;

pub async fn require_secret(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let provided = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .unwrap_or("");

    if provided == state.shared_secret.as_str() {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
