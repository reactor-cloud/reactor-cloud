-- Migration 010: Platform Operations Support
-- Adds infrastructure for platform-level operators, WebAuthn/passkeys, and scoped sessions

-- ----------------------------------------------------------------------------
-- 1. Allow platform-scope memberships (NULL org_id)
-- ----------------------------------------------------------------------------

-- First, create a platform-scope roles table for global roles (separate from org-scoped roles)
CREATE TABLE reactor_auth.platform_roles (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name            TEXT NOT NULL UNIQUE,                   -- 'platform_operator', 'platform_admin'
    description     TEXT,
    is_system       BOOLEAN NOT NULL DEFAULT TRUE,          -- system roles cannot be deleted
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Platform role permissions (similar to role_permissions but for platform roles)
CREATE TABLE reactor_auth.platform_role_permissions (
    role_id         UUID NOT NULL REFERENCES reactor_auth.platform_roles(id) ON DELETE CASCADE,
    permission      TEXT NOT NULL,                          -- 'ops:*', 'cloud:*', 'vault:read'
    requires_step_up BOOLEAN NOT NULL DEFAULT FALSE,        -- if true, MFA step-up required
    PRIMARY KEY (role_id, permission)
);

-- Platform memberships (user <-> platform role relationship, no org required)
CREATE TABLE reactor_auth.platform_memberships (
    user_id         UUID NOT NULL REFERENCES reactor_auth.users(id) ON DELETE CASCADE,
    role_id         UUID NOT NULL REFERENCES reactor_auth.platform_roles(id) ON DELETE RESTRICT,
    granted_by      UUID REFERENCES reactor_auth.users(id), -- who granted this (NULL for bootstrap)
    granted_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at      TIMESTAMPTZ,                            -- soft-delete for audit trail
    PRIMARY KEY (user_id, role_id)
);

CREATE INDEX platform_memberships_user_idx ON reactor_auth.platform_memberships (user_id) WHERE revoked_at IS NULL;

-- Seed the platform_operator role with default permissions
INSERT INTO reactor_auth.platform_roles (id, name, description, is_system)
VALUES (
    '00000000-0000-0000-0000-000000000001'::UUID,
    'platform_operator',
    'Platform operator with full access to deployment, cloud control plane, and vault operations',
    TRUE
);

-- Add permissions for platform_operator role
INSERT INTO reactor_auth.platform_role_permissions (role_id, permission, requires_step_up) VALUES
    ('00000000-0000-0000-0000-000000000001'::UUID, 'ops:*', FALSE),
    ('00000000-0000-0000-0000-000000000001'::UUID, 'ops:read', FALSE),
    ('00000000-0000-0000-0000-000000000001'::UUID, 'ops:deploy', FALSE),
    ('00000000-0000-0000-0000-000000000001'::UUID, 'cloud:*', FALSE),
    ('00000000-0000-0000-0000-000000000001'::UUID, 'cloud:projects:read', FALSE),
    ('00000000-0000-0000-0000-000000000001'::UUID, 'cloud:projects:create', FALSE),
    ('00000000-0000-0000-0000-000000000001'::UUID, 'cloud:projects:delete', TRUE),  -- step-up required
    ('00000000-0000-0000-0000-000000000001'::UUID, 'vault:read', FALSE),
    ('00000000-0000-0000-0000-000000000001'::UUID, 'vault:write', TRUE),             -- step-up required
    ('00000000-0000-0000-0000-000000000001'::UUID, 'ops:cluster_admin', TRUE);       -- step-up required

-- ----------------------------------------------------------------------------
-- 2. WebAuthn/Passkey credentials table
-- ----------------------------------------------------------------------------

CREATE TABLE reactor_auth.webauthn_credentials (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id         UUID NOT NULL REFERENCES reactor_auth.users(id) ON DELETE CASCADE,
    credential_id   BYTEA NOT NULL UNIQUE,                  -- base64url-decoded credential ID
    public_key      BYTEA NOT NULL,                         -- COSE public key
    aaguid          UUID,                                   -- authenticator attestation GUID
    counter         BIGINT NOT NULL DEFAULT 0,              -- signature counter for replay protection
    transports      TEXT[] DEFAULT '{}',                    -- 'usb', 'nfc', 'ble', 'internal', 'hybrid'
    name            TEXT,                                   -- user-friendly name for the credential
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at    TIMESTAMPTZ
);

CREATE INDEX webauthn_credentials_user_idx ON reactor_auth.webauthn_credentials (user_id);
CREATE INDEX webauthn_credentials_credential_id_idx ON reactor_auth.webauthn_credentials (credential_id);

-- ----------------------------------------------------------------------------
-- 3. WebAuthn challenges table (short-lived, for registration/authentication)
-- ----------------------------------------------------------------------------

CREATE TABLE reactor_auth.webauthn_challenges (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    session_id      UUID NOT NULL,                          -- can be a browser session or API session ID
    challenge       BYTEA NOT NULL,                         -- random challenge bytes
    challenge_type  TEXT NOT NULL,                          -- 'registration' or 'authentication'
    user_id         UUID REFERENCES reactor_auth.users(id) ON DELETE CASCADE,  -- NULL for registration
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL,                   -- typically 5 minutes from creation
    consumed_at     TIMESTAMPTZ,                            -- set when challenge is used
    CONSTRAINT webauthn_challenges_type_check CHECK (challenge_type IN ('registration', 'authentication'))
);

CREATE INDEX webauthn_challenges_session_idx ON reactor_auth.webauthn_challenges (session_id) WHERE consumed_at IS NULL;
CREATE INDEX webauthn_challenges_expires_idx ON reactor_auth.webauthn_challenges (expires_at) WHERE consumed_at IS NULL;

-- ----------------------------------------------------------------------------
-- 4. Extend sessions table with scopes and MFA timestamps
-- ----------------------------------------------------------------------------

-- Add scopes column - JSON array of granted scopes for this session
ALTER TABLE reactor_auth.sessions
    ADD COLUMN IF NOT EXISTS scopes JSONB DEFAULT '[]'::JSONB;

-- Add mfa_at - timestamp when MFA was last verified (for session)
ALTER TABLE reactor_auth.sessions
    ADD COLUMN IF NOT EXISTS mfa_at TIMESTAMPTZ;

-- Add step_up_at - timestamp when step-up authentication was last completed
ALTER TABLE reactor_auth.sessions
    ADD COLUMN IF NOT EXISTS step_up_at TIMESTAMPTZ;

-- Create index for finding sessions with recent MFA
CREATE INDEX sessions_mfa_at_idx ON reactor_auth.sessions (user_id, mfa_at DESC) WHERE revoked_at IS NULL;

-- ----------------------------------------------------------------------------
-- 5. Authorization codes table for PKCE flow
-- ----------------------------------------------------------------------------

CREATE TABLE reactor_auth.authorization_codes (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    code_hash       BYTEA NOT NULL UNIQUE,                  -- sha256 of authorization code
    user_id         UUID NOT NULL REFERENCES reactor_auth.users(id) ON DELETE CASCADE,
    client_id       TEXT NOT NULL,                          -- 'reactor-cli', 'reactor-web'
    redirect_uri    TEXT NOT NULL,
    scopes          JSONB NOT NULL DEFAULT '[]'::JSONB,     -- requested scopes
    code_challenge  TEXT NOT NULL,                          -- PKCE S256 challenge
    code_challenge_method TEXT NOT NULL DEFAULT 'S256',
    nonce           TEXT,                                   -- optional nonce for OIDC
    state           TEXT,                                   -- state parameter for CSRF protection
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL,                   -- typically 10 minutes from creation
    used_at         TIMESTAMPTZ,                            -- set when code is exchanged
    session_id      UUID REFERENCES reactor_auth.sessions(id) ON DELETE CASCADE,  -- resulting session
    CONSTRAINT auth_codes_challenge_method_check CHECK (code_challenge_method IN ('S256', 'plain'))
);

CREATE INDEX authorization_codes_hash_idx ON reactor_auth.authorization_codes (code_hash) WHERE used_at IS NULL;
CREATE INDEX authorization_codes_expires_idx ON reactor_auth.authorization_codes (expires_at) WHERE used_at IS NULL;

-- ----------------------------------------------------------------------------
-- 6. Ops audit log (separate from cloud audit_log, captures operator actions)
-- ----------------------------------------------------------------------------

CREATE TABLE reactor_auth.ops_audit_log (
    id              BIGSERIAL PRIMARY KEY,
    ts              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_user_id   UUID REFERENCES reactor_auth.users(id),
    actor_ip        INET,
    actor_user_agent TEXT,
    action          TEXT NOT NULL,                          -- 'deploy', 'project.create', 'vault.read'
    scope_used      TEXT,                                   -- which scope was used for authorization
    resource_type   TEXT,                                   -- 'deployment', 'project', 'vault_secret'
    resource_id     TEXT,                                   -- ID of the affected resource
    status          TEXT NOT NULL DEFAULT 'success',        -- 'success', 'denied', 'error'
    details         JSONB NOT NULL DEFAULT '{}'::JSONB,
    step_up_used    BOOLEAN NOT NULL DEFAULT FALSE          -- whether step-up auth was required/used
);

CREATE INDEX ops_audit_log_ts_idx ON reactor_auth.ops_audit_log (ts DESC);
CREATE INDEX ops_audit_log_actor_idx ON reactor_auth.ops_audit_log (actor_user_id, ts DESC);
CREATE INDEX ops_audit_log_action_idx ON reactor_auth.ops_audit_log (action, ts DESC);
CREATE INDEX ops_audit_log_resource_idx ON reactor_auth.ops_audit_log (resource_type, resource_id);

-- ----------------------------------------------------------------------------
-- Comments
-- ----------------------------------------------------------------------------

COMMENT ON TABLE reactor_auth.platform_roles IS 'Global platform roles (not org-scoped) for operators and admins';
COMMENT ON TABLE reactor_auth.platform_role_permissions IS 'Permissions granted to platform roles, with step-up requirements';
COMMENT ON TABLE reactor_auth.platform_memberships IS 'User membership in platform roles';
COMMENT ON TABLE reactor_auth.webauthn_credentials IS 'WebAuthn/passkey credentials for MFA and passwordless authentication';
COMMENT ON TABLE reactor_auth.webauthn_challenges IS 'Short-lived challenges for WebAuthn registration/authentication ceremonies';
COMMENT ON TABLE reactor_auth.authorization_codes IS 'OAuth 2.0 authorization codes for PKCE flow';
COMMENT ON TABLE reactor_auth.ops_audit_log IS 'Audit log for operator actions (deploy, cloud, vault)';
COMMENT ON COLUMN reactor_auth.platform_role_permissions.requires_step_up IS 'If true, MFA step-up is required to use this permission';
COMMENT ON COLUMN reactor_auth.sessions.scopes IS 'JSON array of scopes granted to this session';
COMMENT ON COLUMN reactor_auth.sessions.mfa_at IS 'Timestamp when MFA was last verified for this session';
COMMENT ON COLUMN reactor_auth.sessions.step_up_at IS 'Timestamp when step-up authentication was last completed';
