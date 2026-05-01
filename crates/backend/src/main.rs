use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "polish-backend", about = "Voice Polish local daemon")]
struct Cli {
    /// TCP port to listen on
    #[arg(long, default_value = "48484")]
    port: u16,

    /// Path to the SQLite database file (default: ~/Library/Application Support/VoicePolish/db.sqlite)
    #[arg(long)]
    db: Option<String>,
}

#[tokio::main]
async fn main() {
    // ── Structured logging ────────────────────────────────────────────────────
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("polish_backend=debug".parse().unwrap()),
        )
        .init();

    // ── Load env vars ─────────────────────────────────────────────────────────
    voice_polish_core::load_env();

    let cli = Cli::parse();

    // ── Resolve DB path ───────────────────────────────────────────────────────
    let db_path = if let Some(ref path) = cli.db {
        std::path::PathBuf::from(path)
    } else {
        polish_backend::store::default_db_path()
    };

    // ── Fingerprint — visible in logs so we can confirm binary version ───────
    info!("polish-backend build={} features=openai_oauth+codex_api", env!("CARGO_PKG_VERSION"));

    // ── Open DB + ensure default user ─────────────────────────────────────────
    let pool    = polish_backend::store::open(&db_path);
    let user_id = polish_backend::store::ensure_default_user(&pool);
    let secret  = std::env::var("POLISH_SHARED_SECRET")
        .unwrap_or_else(|_| "dev-secret".into());

    let state = polish_backend::AppState {
        pool:            pool.clone(),
        shared_secret:   std::sync::Arc::new(secret),
        default_user_id: std::sync::Arc::new(user_id.clone()),
    };

    // ── Build router ──────────────────────────────────────────────────────────
    let router = polish_backend::router_with_state(state);

    // ── Bind listener ─────────────────────────────────────────────────────────
    let addr     = format!("127.0.0.1:{}", cli.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("failed to bind {addr}: {e}"));

    info!("polish-backend listening on http://{addr}");

    // ── 7-day cleanup task (every 6 hours) ────────────────────────────────────
    {
        let pool2 = pool.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(6 * 3600));
            interval.tick().await; // skip first immediate tick
            loop {
                interval.tick().await;
                polish_backend::store::history::cleanup_old_recordings(&pool2);
                info!("[cleanup] 7-day recording sweep complete");
            }
        });
    }

    // ── Hourly metering report task ───────────────────────────────────────────
    {
        let pool3    = pool.clone();
        let user_id2 = user_id.clone();
        tokio::spawn(async move {
            let cloud_url = std::env::var("CLOUD_API_URL")
                .unwrap_or_else(|_| "https://cloud.voicepolish.app".into());
            let http = reqwest::Client::new();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
            interval.tick().await; // skip first tick so we don't report on startup
            loop {
                interval.tick().await;
                send_metering_report(&pool3, &user_id2, &http, &cloud_url).await;
            }
        });
    }

    // ── Graceful shutdown on SIGTERM / SIGINT ─────────────────────────────────
    let shutdown = async {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM listener");
            let mut sigint  = signal(SignalKind::interrupt()).expect("SIGINT listener");
            tokio::select! {
                _ = sigterm.recv() => info!("received SIGTERM — shutting down"),
                _ = sigint.recv()  => info!("received SIGINT — shutting down"),
            }
        }
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c().await.ok();
            info!("received Ctrl-C — shutting down");
        }
    };

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown)
        .await
        .expect("server error");

    info!("polish-backend stopped");
}

// ── Metering batch ────────────────────────────────────────────────────────────

/// Aggregate recording counts from the last ~24h and POST to the cloud
/// metering endpoint. Silently skips if the user has no cloud token.
async fn send_metering_report(
    pool:      &polish_backend::store::DbPool,
    user_id:   &str,
    http:      &reqwest::Client,
    cloud_url: &str,
) {
    use polish_backend::store::users;
    use tracing::{debug, warn};

    // Read cloud token
    let Some(user) = users::get_user(pool, user_id) else { return; };
    let Some(token) = user.cloud_token else {
        debug!("[metering] no cloud token — skipping");
        return;
    };

    // Aggregate from recordings over the last 7 days (matches history retention)
    let events: Vec<serde_json::Value> = {
        let conn = match pool.get() {
            Ok(c) => c,
            Err(_) => return,
        };
        let cutoff_ms: i64 = (polish_backend::store::now_ms()) - (7 * 24 * 3600 * 1000);

        match conn.prepare(
            "SELECT DATE(datetime(timestamp_ms / 1000, 'unixepoch')) as date,
                    model_used,
                    COUNT(*) as polish_count,
                    SUM(word_count) as word_count
               FROM recordings
              WHERE user_id = ?1 AND timestamp_ms >= ?2
              GROUP BY date, model_used"
        ) {
            Ok(mut stmt) => {
                stmt.query_map(
                    rusqlite::params![user_id, cutoff_ms],
                    |row| {
                        let date:         String = row.get(0)?;
                        let model:        String = row.get(1)?;
                        let polish_count: i64   = row.get(2)?;
                        let word_count:   i64   = row.get(3)?;
                        Ok(serde_json::json!({
                            "date":         date,
                            "model":        model,
                            "polish_count": polish_count,
                            "word_count":   word_count,
                        }))
                    },
                )
                .ok()
                .map(|rows| rows.flatten().collect())
                .unwrap_or_default()
            }
            Err(_) => vec![],
        }
    };

    if events.is_empty() {
        debug!("[metering] no events to report");
        return;
    }

    let url     = format!("{}/v1/metering/report", cloud_url.trim_end_matches('/'));
    let payload = serde_json::json!({ "events": events });

    match http
        .post(&url)
        .bearer_auth(&token)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            info!("[metering] reported {} event rows to cloud", events.len());
        }
        Ok(resp) => {
            warn!("[metering] cloud returned {}", resp.status());
        }
        Err(e) => {
            debug!("[metering] cloud unreachable: {e}");
        }
    }
}
