use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    Json,
};
use serde::Deserialize;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

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
    let items = crate::store::history::list_recordings(&state.pool, &user_id, q.limit, q.before);
    Ok(Json(items))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> StatusCode {
    // Also delete the WAV file if audio_id is linked
    if let Some(rec) = crate::store::history::get_recording(&state.pool, &id) {
        if let Some(audio_id) = rec.audio_id {
            let wav = audio_dir().join(format!("{audio_id}.wav"));
            let _ = std::fs::remove_file(wav);
        }
    }
    if crate::store::history::delete_recording(&state.pool, &id) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

pub async fn audio(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response<Body>, StatusCode> {
    let rec = crate::store::history::get_recording(&state.pool, &id)
        .ok_or(StatusCode::NOT_FOUND)?;

    let audio_id = rec.audio_id.ok_or(StatusCode::NOT_FOUND)?;
    let path = audio_dir().join(format!("{audio_id}.wav"));

    let file = File::open(&path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    Ok(Response::builder()
        .status(200)
        .header(header::CONTENT_TYPE, "audio/wav")
        .header(header::CACHE_CONTROL, "no-store")
        .body(body)
        .unwrap())
}

fn audio_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("VoicePolish")
        .join("audio")
}
