use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use tracing::info;

use crate::{store::prefs::{PrefsUpdate, Preferences}, AppState};

pub async fn get_prefs(State(state): State<AppState>) -> Result<Json<Preferences>, StatusCode> {
    let user_id = state.default_user_id.clone();
    let prefs   = crate::store::prefs::get_prefs(&state.pool, &user_id)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(prefs))
}

pub async fn patch_prefs(
    State(state): State<AppState>,
    Json(update): Json<PrefsUpdate>,
) -> Result<Json<Preferences>, StatusCode> {
    info!("[patch_prefs] backend received: llm_provider={:?} selected_model={:?} gateway_key_set={} gemini_key_set={}",
        update.llm_provider,
        update.selected_model,
        update.gateway_api_key.as_ref().map(|v| v.is_some()).unwrap_or(false),
        update.gemini_api_key.as_ref().map(|v| v.is_some()).unwrap_or(false),
    );
    let user_id = state.default_user_id.clone();
    let prefs   = crate::store::prefs::update_prefs(&state.pool, &user_id, update)
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
    info!("[patch_prefs] after update: llm_provider={:?}", prefs.llm_provider);
    Ok(Json(prefs))
}
