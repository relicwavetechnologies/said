use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::params;
use std::path::PathBuf;
use tracing::info;

pub mod history;
pub mod openai_oauth;
pub mod prefs;
pub mod users;
pub mod vectors;

pub type DbPool = Pool<SqliteConnectionManager>;

const MIGRATION_001: &str = include_str!("migrations/001_initial.sql");
const MIGRATION_002: &str = include_str!("migrations/002_vectors.sql");
const MIGRATION_003: &str = include_str!("migrations/003_output_language.sql");
const MIGRATION_004: &str = include_str!("migrations/004_api_keys.sql");
const MIGRATION_005: &str = include_str!("migrations/005_llm_provider.sql");
const MIGRATION_006: &str = include_str!("migrations/006_openai_oauth.sql");

/// Open (or create) the SQLite database at `path`, run pending migrations,
/// and return a connection pool.
pub fn open(path: &PathBuf) -> DbPool {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .expect("failed to create database directory");
    }

    let manager = SqliteConnectionManager::file(path)
        .with_init(|conn| {
            // 5-second busy timeout so a stale WAL lock from a previous session
            // doesn't block migration indefinitely.
            conn.execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA busy_timeout=5000;"
            )?;
            Ok(())
        });

    let pool = Pool::builder()
        .max_size(5)
        .connection_timeout(std::time::Duration::from_secs(10))
        .build(manager)
        .expect("failed to create SQLite connection pool");

    run_migrations(&pool);
    pool
}

fn run_migrations(pool: &DbPool) {
    let conn = pool.get().expect("pool get failed during migration");

    // Check schema version
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .unwrap_or(0);

    if version < 1 {
        info!("running migration 001_initial");
        conn.execute_batch(MIGRATION_001)
            .expect("migration 001 failed");
        conn.execute_batch("PRAGMA user_version = 1")
            .expect("failed to set user_version");
    }

    if version < 2 {
        info!("running migration 002_vectors");
        conn.execute_batch(MIGRATION_002)
            .expect("migration 002 failed");
        conn.execute_batch("PRAGMA user_version = 2")
            .expect("failed to set user_version to 2");
    }

    if version < 3 {
        info!("running migration 003_output_language");
        conn.execute_batch(MIGRATION_003)
            .expect("migration 003 failed");
        conn.execute_batch("PRAGMA user_version = 3")
            .expect("failed to set user_version to 3");
    }

    if version < 4 {
        info!("running migration 004_api_keys");
        conn.execute_batch(MIGRATION_004)
            .expect("migration 004 failed");
        conn.execute_batch("PRAGMA user_version = 4")
            .expect("failed to set user_version to 4");
    }

    if version < 5 {
        info!("running migration 005_llm_provider");
        conn.execute_batch(MIGRATION_005)
            .expect("migration 005 failed");
        conn.execute_batch("PRAGMA user_version = 5")
            .expect("failed to set user_version to 5");
    }

    if version < 6 {
        info!("running migration 006_openai_oauth");
        conn.execute_batch(MIGRATION_006)
            .expect("migration 006 failed");
        conn.execute_batch("PRAGMA user_version = 6")
            .expect("failed to set user_version to 6");
    }
}

/// Return the default database path: ~/Library/Application Support/VoicePolish/db.sqlite
pub fn default_db_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join("Library")
        .join("Application Support")
        .join("VoicePolish")
        .join("db.sqlite")
}

/// Ensure the single default local user exists.
/// Returns the user_id (UUID string).
pub fn ensure_default_user(pool: &DbPool) -> String {
    let conn = pool.get().expect("pool get");

    // Check if any user exists
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM local_user", [], |r| r.get(0))
        .unwrap_or(0);

    if count > 0 {
        // Return existing user id
        return conn
            .query_row("SELECT id FROM local_user LIMIT 1", [], |r| r.get(0))
            .expect("failed to read user id");
    }

    // Create default user
    let id = uuid::Uuid::new_v4().to_string();
    let now_ms = now_ms();
    conn.execute(
        "INSERT INTO local_user (id, email, license_tier, created_at)
         VALUES (?1, ?2, 'free', ?3)",
        params![id, "local@voicepolish.app", now_ms],
    )
    .expect("failed to create default user");

    // Create default preferences
    conn.execute(
        "INSERT INTO preferences (user_id, selected_model, tone_preset, language,
         auto_paste, edit_capture, polish_text_hotkey, updated_at)
         VALUES (?1, 'smart', 'neutral', 'auto', 1, 1, 'cmd+shift+p', ?2)",
        params![id, now_ms],
    )
    .expect("failed to create default preferences");

    info!("created default local user: {id}");
    id
}

pub fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
