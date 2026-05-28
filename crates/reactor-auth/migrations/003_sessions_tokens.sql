-- Migration 003: Sessions and Refresh Tokens
-- Creates session management tables for reactor-auth

-- Sessions (one row per active session)
CREATE TABLE reactor_auth.sessions (
    id              UUID PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES reactor_auth.users(id) ON DELETE CASCADE,
    amr             TEXT[] NOT NULL DEFAULT '{}',       -- auth methods used: 'pwd', 'totp', etc.
    ip              INET,
    user_agent      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_seen_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at      TIMESTAMPTZ
);

CREATE INDEX sessions_user_id_idx ON reactor_auth.sessions (user_id);
CREATE INDEX sessions_user_active_idx ON reactor_auth.sessions (user_id) WHERE revoked_at IS NULL;

-- Refresh tokens (rotating, single-use)
CREATE TABLE reactor_auth.refresh_tokens (
    id              UUID PRIMARY KEY,
    session_id      UUID NOT NULL REFERENCES reactor_auth.sessions(id) ON DELETE CASCADE,
    token_hash      BYTEA NOT NULL UNIQUE,              -- sha256 of refresh token
    issued_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL,
    used_at         TIMESTAMPTZ,                        -- non-null => burned
    replaced_by     UUID                                -- next token after rotation
);

CREATE INDEX refresh_tokens_session_id_idx ON reactor_auth.refresh_tokens (session_id) WHERE used_at IS NULL;
CREATE INDEX refresh_tokens_hash_idx ON reactor_auth.refresh_tokens (token_hash);
