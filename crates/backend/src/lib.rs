use axum::{
    http::{header, Method},
    middleware,
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};

pub mod auth;
pub mod embedder;
pub mod llm;
pub mod routes;
pub mod stt;
pub mod store;

// ── Application state ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub pool:            store::DbPool,
    pub shared_secret:   Arc<String>,
    pub default_user_id: Arc<String>,
}

// ── Router factory ────────────────────────────────────────────────────────────

pub fn router_with_state(state: AppState) -> Router {
    // Public routes (no auth)
    let public = Router::new()
        .route("/v1/health", get(routes::health::handler));

    // Authenticated routes (require shared-secret bearer)
    let authenticated = Router::new()
        .route("/v1/voice/polish",    post(routes::voice::polish))
        .route("/v1/text/polish",     post(routes::text::polish))
        .route("/v1/edit-feedback",   post(routes::feedback::submit))
        .route("/v1/history",         get(routes::history::list))
        .route("/v1/preferences",     get(routes::prefs::get_prefs))
        .route("/v1/preferences",     patch(routes::prefs::patch_prefs))
        // Cloud auth bridge — store/clear cloud token, query cloud status
        .route("/v1/cloud/token",     axum::routing::put(routes::cloud::store_token))
        .route("/v1/cloud/token",     axum::routing::delete(routes::cloud::clear_token))
        .route("/v1/cloud/status",    get(routes::cloud::status))
        // OpenAI Codex OAuth
        .route("/v1/openai-oauth/initiate",   post(routes::openai_oauth::initiate))
        .route("/v1/openai-oauth/status",     get(routes::openai_oauth::status))
        .route("/v1/openai-oauth/disconnect", axum::routing::delete(routes::openai_oauth::disconnect))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_secret,
        ));

    // CORS — allow the Tauri webview origin and localhost dev server
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT]);

    public
        .merge(authenticated)
        .layer(cors)
        .with_state(state)
}

/// Convenience builder used by main.rs — reads shared secret from env,
/// opens the DB, ensures the default user exists, and returns a ready Router.
pub fn router() -> Router {
    let secret      = std::env::var("POLISH_SHARED_SECRET")
        .unwrap_or_else(|_| "dev-secret".into());
    let db_path     = store::default_db_path();
    let pool        = store::open(&db_path);
    let user_id     = store::ensure_default_user(&pool);

    let state = AppState {
        pool,
        shared_secret:   Arc::new(secret),
        default_user_id: Arc::new(user_id),
    };

    router_with_state(state)
}
