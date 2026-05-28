-- API keys for machine-to-machine authentication
CREATE TABLE IF NOT EXISTS reactor_auth.api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES reactor_auth.users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    -- Only store a hash of the key, never the plaintext
    key_hash TEXT NOT NULL,
    -- Store the first 8 chars of the key for identification (e.g., "rk_live_abc...")
    prefix TEXT NOT NULL,
    -- Optional scopes to limit what the key can do (null = all permissions)
    scopes JSONB DEFAULT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Track when the key was last used for auditing
    last_used_at TIMESTAMPTZ DEFAULT NULL,
    -- Soft-delete via revoked_at (keeps audit trail)
    revoked_at TIMESTAMPTZ DEFAULT NULL
);

-- Index for looking up keys by user
CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON reactor_auth.api_keys(user_id);

-- Index for validating keys by hash (used during authentication)
CREATE INDEX IF NOT EXISTS idx_api_keys_key_hash ON reactor_auth.api_keys(key_hash) WHERE revoked_at IS NULL;

-- Index for listing non-revoked keys
CREATE INDEX IF NOT EXISTS idx_api_keys_active ON reactor_auth.api_keys(user_id, revoked_at) WHERE revoked_at IS NULL;
