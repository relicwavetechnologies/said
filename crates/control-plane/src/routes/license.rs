//! GET /v1/license/check
//!
//! Returns the caller's current license tier, features, and limits.
//! Called by the Tauri desktop app on every launch (result cached locally 24h).

use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::{auth::AuthUser, routes::auth::license_features, AppState};

pub async fn check(
    State(state): State<AppState>,
    user:         AuthUser,
) -> Json<Value> {
    // Fetch active license (fallback to "free")
    let tier: String = sqlx::query_scalar(
        "SELECT tier FROM license_keys
          WHERE account_id = $1 AND active = true
            AND (expires_at IS NULL OR expires_at > now())
          ORDER BY created_at DESC LIMIT 1"
    )
    .bind(user.account_id)
    .fetch_optional(&state.db)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "free".into());

    let features = license_features(&tier);

    // Polish limits per day
    let daily_limit = match tier.as_str() {
        "pro"  => 500,
        "team" => 2000,
        _      => 50,   // free
    };

    Json(json!({
        "tier":        tier,
        "active":      true,
        "features":    features,
        "limits": {
            "daily_polishes": daily_limit,
        },
    }))
}
