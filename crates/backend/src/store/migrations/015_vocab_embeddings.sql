-- Migration 015 — embeddings for vocabulary entries.
--
-- Each vocabulary term gets an embedding of "{term}. {example_context}"
-- so the polish-time selector can pick the K most relevant entries to the
-- current transcript instead of dumping all 200+ vocab rows into the prompt.
--
-- Same schema pattern as `preference_vectors` (256d f32 little-endian BLOB)
-- so we reuse `embedder::gemini::{floats_to_blob, blob_to_floats}` and the
-- existing 256d cache. Migration 011 confirmed 256 dims; we follow.
--
-- One row per (user_id, term). Updated_at lets us detect drift if we ever
-- choose to re-embed when example_context changes (today: first-observed
-- context wins, so embeddings are write-once per term).

CREATE TABLE IF NOT EXISTS vocab_embeddings (
    user_id    TEXT NOT NULL REFERENCES local_user(id) ON DELETE CASCADE,
    term       TEXT NOT NULL,
    embedding  BLOB NOT NULL,    -- 256 × f32 LE
    updated_at INTEGER NOT NULL,
    UNIQUE(user_id, term)
);
CREATE INDEX IF NOT EXISTS idx_vocab_embed_user ON vocab_embeddings (user_id);
