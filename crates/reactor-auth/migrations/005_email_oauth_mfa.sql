-- Migration 005: Email Tokens, OAuth State, and MFA Factors
-- Creates tables for email verification, OAuth, and MFA

-- Email tokens (verify, recover, invite)
CREATE TABLE reactor_auth.email_tokens (
    token_hash      BYTEA PRIMARY KEY,                  -- sha256(token)
    purpose         TEXT NOT NULL,                      -- 'signup', 'recovery', 'email_change', 'invite'
    user_id         UUID REFERENCES reactor_auth.users(id) ON DELETE CASCADE,
    org_id          UUID REFERENCES reactor_auth.orgs(id) ON DELETE CASCADE,  -- for invites
    email           TEXT,                               -- target email for invites
    role_id         UUID REFERENCES reactor_auth.roles(id) ON DELETE CASCADE, -- for invites
    payload         JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL,
    used_at         TIMESTAMPTZ
);

CREATE INDEX email_tokens_user_id_idx ON reactor_auth.email_tokens (user_id) WHERE used_at IS NULL;
CREATE INDEX email_tokens_org_id_idx ON reactor_auth.email_tokens (org_id) WHERE used_at IS NULL;
CREATE INDEX email_tokens_expires_idx ON reactor_auth.email_tokens (expires_at) WHERE used_at IS NULL;

-- OAuth state (PKCE)
CREATE TABLE reactor_auth.oauth_states (
    state           TEXT PRIMARY KEY,                   -- random url-safe
    provider        TEXT NOT NULL,
    code_verifier   TEXT NOT NULL,
    redirect_to     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL                -- 10 min
);

CREATE INDEX oauth_states_expires_idx ON reactor_auth.oauth_states (expires_at);

-- MFA factors
CREATE TABLE reactor_auth.mfa_factors (
    id              UUID PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES reactor_auth.users(id) ON DELETE CASCADE,
    factor_type     TEXT NOT NULL,                      -- 'totp'
    secret          BYTEA NOT NULL,                     -- encrypted at app layer
    verified_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX mfa_factors_user_id_idx ON reactor_auth.mfa_factors (user_id);
