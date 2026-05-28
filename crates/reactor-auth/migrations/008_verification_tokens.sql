-- Migration 008: Email Verification Tokens
-- Creates table for email verification tokens used during signup

-- Email verification tokens
CREATE TABLE reactor_auth.verification_tokens (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES reactor_auth.users(id) ON DELETE CASCADE,
    token_hash      BYTEA NOT NULL UNIQUE,              -- sha256 of verification token
    token_type      TEXT NOT NULL DEFAULT 'email',      -- 'email', 'password_reset', etc.
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL,
    used_at         TIMESTAMPTZ                         -- non-null => already verified
);

CREATE INDEX verification_tokens_user_idx ON reactor_auth.verification_tokens (user_id, token_type) 
    WHERE used_at IS NULL;
CREATE INDEX verification_tokens_hash_idx ON reactor_auth.verification_tokens (token_hash) 
    WHERE used_at IS NULL;
CREATE INDEX verification_tokens_expires_idx ON reactor_auth.verification_tokens (expires_at) 
    WHERE used_at IS NULL;
