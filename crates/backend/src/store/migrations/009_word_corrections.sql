-- Explicit word-level substitution rules extracted from user edits.
-- Loaded in full at polish-time (always tiny — tens of rows at most).
CREATE TABLE IF NOT EXISTS word_corrections (
    user_id      TEXT NOT NULL,
    wrong_text   TEXT NOT NULL,
    correct_text TEXT NOT NULL,
    count        INTEGER NOT NULL DEFAULT 1,
    updated_at   INTEGER NOT NULL,
    UNIQUE(user_id, wrong_text)
);
