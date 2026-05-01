-- Migration 006: OpenAI OAuth token storage
-- Stores the user's connected ChatGPT account OAuth tokens locally.
-- access_token is used as Bearer for https://chatgpt.com/backend-api/codex/responses
CREATE TABLE IF NOT EXISTS openai_oauth (
    user_id      TEXT PRIMARY KEY REFERENCES local_user(id),
    access_token TEXT NOT NULL,
    refresh_token TEXT,
    expires_at   INTEGER NOT NULL,   -- unix ms
    connected_at INTEGER NOT NULL    -- unix ms
);
