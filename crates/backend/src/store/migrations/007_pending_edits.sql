-- Pending edits awaiting user approval before being saved to the learning corpus.
CREATE TABLE IF NOT EXISTS pending_edits (
    id           TEXT PRIMARY KEY,
    user_id      TEXT NOT NULL REFERENCES local_user(id) ON DELETE CASCADE,
    recording_id TEXT REFERENCES recordings(id) ON DELETE SET NULL,
    ai_output    TEXT NOT NULL,
    user_kept    TEXT NOT NULL,
    timestamp_ms INTEGER NOT NULL,
    resolved     INTEGER NOT NULL DEFAULT 0  -- 0=pending, 1=approved, 2=skipped
);
CREATE INDEX IF NOT EXISTS idx_pending_user
    ON pending_edits (user_id, resolved, timestamp_ms DESC);
