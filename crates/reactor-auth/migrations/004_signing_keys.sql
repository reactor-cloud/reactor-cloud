-- Migration 004: JWT Signing Keys
-- Creates the signing key rotation table for reactor-auth

-- JWT signing keys (rotation support)
CREATE TABLE reactor_auth.signing_keys (
    kid             TEXT PRIMARY KEY,                   -- e.g. 'k_01HZ...'
    algorithm       TEXT NOT NULL,                      -- 'RS256'
    private_key_pem TEXT NOT NULL,                      -- encrypted at app layer
    public_key_pem  TEXT NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    activated_at    TIMESTAMPTZ NOT NULL,
    rotated_at      TIMESTAMPTZ,                        -- when superseded
    retired_at      TIMESTAMPTZ                         -- when no longer in JWKS
);

-- Index for finding active and previous keys
CREATE INDEX signing_keys_active_idx ON reactor_auth.signing_keys (activated_at DESC) WHERE retired_at IS NULL;
