-- Voice Polish migration 002 — preference vectors table
-- Pure-SQLite approach: embeddings stored as BLOB (768 × f32 little-endian),
-- cosine similarity computed in Rust on the full user corpus.
-- At personal scale (< 1 000 edits) this is indistinguishable from sqlite-vec.

CREATE TABLE IF NOT EXISTS preference_vectors (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id       TEXT    NOT NULL,
    edit_event_id TEXT    NOT NULL UNIQUE,
    embedding     BLOB    NOT NULL  -- 768 × f32 LE = 3 072 bytes
);

CREATE INDEX IF NOT EXISTS idx_vec_user ON preference_vectors (user_id);
