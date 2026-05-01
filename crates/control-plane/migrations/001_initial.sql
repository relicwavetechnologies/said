-- Accounts ------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS accounts (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email           TEXT UNIQUE NOT NULL,
    password_hash   TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Sessions -------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS sessions (
    token           UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id      UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sessions_account ON sessions (account_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions (expires_at);

-- License keys ---------------------------------------------------------------
CREATE TABLE IF NOT EXISTS license_keys (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id      UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    tier            TEXT NOT NULL DEFAULT 'free',   -- 'free' | 'pro' | 'team'
    active          BOOLEAN NOT NULL DEFAULT true,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at      TIMESTAMPTZ                     -- NULL = never
);
CREATE INDEX IF NOT EXISTS idx_license_account ON license_keys (account_id);

-- Usage events (aggregate counts — NO user content) -------------------------
CREATE TABLE IF NOT EXISTS usage_events (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id      UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    event_date      DATE NOT NULL DEFAULT CURRENT_DATE,
    polish_count    INTEGER NOT NULL DEFAULT 0,
    word_count      INTEGER NOT NULL DEFAULT 0,
    model_used      TEXT NOT NULL,
    UNIQUE (account_id, event_date, model_used)
);
CREATE INDEX IF NOT EXISTS idx_usage_account_date ON usage_events (account_id, event_date DESC);
