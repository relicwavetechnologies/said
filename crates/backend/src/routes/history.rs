use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::{store::history::Recording, AppState};

#[derive(Deserialize)]
pub struct HistoryQuery {
    #[serde(default = "default_limit")]
    limit: i64,
    before: Option<i64>,
}

fn default_limit() -> i64 { 50 }

pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Vec<Recording>>, StatusCode> {
    let user_id = state.default_user_id.clone();
    let items   = crate::store::history::list_recordings(&state.pool, &user_id, q.limit, q.before);
    Ok(Json(items))
}
