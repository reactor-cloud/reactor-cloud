-- Migration 007: Invitations table
-- Dedicated table for org invitations with cleaner separation from email tokens

CREATE TABLE reactor_auth.invitations (
    id              UUID PRIMARY KEY,
    token_hash      BYTEA NOT NULL UNIQUE,            -- sha256(token), indexed for lookup
    email           TEXT NOT NULL,                    -- target email address
    org_id          UUID NOT NULL REFERENCES reactor_auth.orgs(id) ON DELETE CASCADE,
    role_id         UUID NOT NULL REFERENCES reactor_auth.roles(id) ON DELETE CASCADE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL,
    used_at         TIMESTAMPTZ                       -- NULL until invitation is accepted
);

CREATE INDEX invitations_org_id_idx ON reactor_auth.invitations (org_id) WHERE used_at IS NULL;
CREATE INDEX invitations_email_idx ON reactor_auth.invitations (email) WHERE used_at IS NULL;
CREATE INDEX invitations_expires_idx ON reactor_auth.invitations (expires_at) WHERE used_at IS NULL;
