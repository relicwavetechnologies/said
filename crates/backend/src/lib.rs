use axum::{
    http::{header, Method},
    middleware,
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

pub mod auth;
pub mod embedder;
pub mod llm;
pub mod routes;
pub mod stt;
pub mod store;

#[cfg(test)]
mod learning_flow_tests;

// ── Preferences hot-cache (Gap 3) ─────────────────────────────────────────────
//
// Avoids a SQLite SELECT on every voice/text/feedback request.
// TTL = 30 s. Invalidated immediately on PATCH /v1/preferences.
// At personal scale one user has exactly one entry, so the HashMap is a formality.

const PREFS_CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct CachedPrefs {
    pub prefs:      store::prefs::Preferences,
    pub cached_at:  Instant,
}

pub type PrefsCache = Arc<RwLock<Option<CachedPrefs>>>;

/// Read preferences, hitting the in-memory cache when fresh.
/// Falls back to SQLite on miss or TTL expiry.
pub async fn get_prefs_cached(
    cache: &PrefsCache,
    pool:  &store::DbPool,
    user_id: &str,
) -> Option<store::prefs::Preferences> {
    // ── Fast path: cache hit ──────────────────────────────────────────────────
    {
        let guard = cache.read().await;
        if let Some(ref entry) = *guard {
            if entry.cached_at.elapsed() < PREFS_CACHE_TTL {
                return Some(entry.prefs.clone());
            }
        }
    }

    // ── Slow path: SQLite read + re-populate cache ────────────────────────────
    let prefs = store::prefs::get_prefs(pool, user_id)?;
    let mut guard = cache.write().await;
    *guard = Some(CachedPrefs { prefs: prefs.clone(), cached_at: Instant::now() });
    tracing::debug!("[prefs-cache] miss → refreshed from SQLite");
    Some(prefs)
}

/// Invalidate the cache after a successful preferences update.
pub async fn invalidate_prefs_cache(cache: &PrefsCache) {
    let mut guard = cache.write().await;
    *guard = None;
    tracing::debug!("[prefs-cache] invalidated");
}

// ── Application state ─────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    pub pool:            store::DbPool,
    pub shared_secret:   Arc<String>,
    pub default_user_id: Arc<String>,
    /// Gap 3: in-memory preferences cache — avoids SQLite SELECT per request.
    pub prefs_cache:     PrefsCache,
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
        .route("/v1/edit-feedback",        post(routes::feedback::submit))
        .route("/v1/classify-edit",         post(routes::classify::classify))
        .route("/v1/pending-edits",        post(routes::pending_edits::create))
        .route("/v1/pending-edits",        get(routes::pending_edits::list))
        .route("/v1/pending-edits/:id/resolve", post(routes::pending_edits::resolve))
        .route("/v1/history",              get(routes::history::list))
        .route("/v1/recordings/:id",       axum::routing::delete(routes::history::delete))
        .route("/v1/recordings/:id/audio", get(routes::history::audio))
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
        prefs_cache:     Arc::new(RwLock::new(None)),
    };

    router_with_state(state)
}
