-- Reactor Connect schema migrations
-- All tables in a single file for initial setup

CREATE SCHEMA IF NOT EXISTS _reactor_connect;

-- 1. Instances (configured connector + credential pointer)
CREATE TABLE IF NOT EXISTS _reactor_connect.instances (
    id                       uuid PRIMARY KEY,
    org_id                   uuid NOT NULL,
    type_id                  text NOT NULL,              -- e.g., 'stripe', 'salesforce', 'airbyte:facebook-marketing'
    name                     text NOT NULL,              -- user-defined unique name within org
    config_json              jsonb NOT NULL DEFAULT '{}',-- non-secret connector config (instance_url, etc.)
    vault_ref                text,                       -- doc path to vault secret: tenant/{project}/connect/{org}/instances/{id}
    credential_state         text NOT NULL DEFAULT 'pending', -- pending | ready | expired | error
    credential_error         text,                       -- last error message if credential_state = error
    enabled                  boolean NOT NULL DEFAULT true,
    created_at               timestamptz NOT NULL DEFAULT now(),
    updated_at               timestamptz NOT NULL DEFAULT now(),
    UNIQUE (org_id, name)
);

CREATE INDEX IF NOT EXISTS idx_instances_org ON _reactor_connect.instances (org_id);
CREATE INDEX IF NOT EXISTS idx_instances_type ON _reactor_connect.instances (type_id);

-- 2. Connections (stream bindings: source → destination)
CREATE TABLE IF NOT EXISTS _reactor_connect.connections (
    id                       uuid PRIMARY KEY,
    org_id                   uuid NOT NULL,
    name                     text NOT NULL,              -- user-defined unique name within org
    
    -- Source configuration
    source_instance_id       uuid REFERENCES _reactor_connect.instances(id) ON DELETE SET NULL,
    source_kind              text NOT NULL,              -- 'instance' | 'data' (reactor-data table)
    source_config_json       jsonb NOT NULL DEFAULT '{}',-- selected streams, modes, primary keys
    
    -- Destination configuration
    dest_instance_id         uuid REFERENCES _reactor_connect.instances(id) ON DELETE SET NULL,
    dest_kind                text NOT NULL,              -- 'instance' | 'data' | 'storage'
    dest_config_json         jsonb NOT NULL DEFAULT '{}',-- table name, bucket, etc.
    
    -- Schedule
    schedule_kind            text NOT NULL DEFAULT 'manual', -- 'cron' | 'on_event' | 'manual'
    schedule_config_json     jsonb NOT NULL DEFAULT '{}',
    
    -- Options
    options_json             jsonb NOT NULL DEFAULT '{}',-- schema_drift, max_rows_per_run, etc.
    
    -- State
    enabled                  boolean NOT NULL DEFAULT false,
    direction                text NOT NULL DEFAULT 'inbound', -- 'inbound' | 'outbound'
    last_sync_at             timestamptz,
    job_name                 text,                       -- reactor-jobs job name (set after delegation)
    
    created_at               timestamptz NOT NULL DEFAULT now(),
    updated_at               timestamptz NOT NULL DEFAULT now(),
    UNIQUE (org_id, name)
);

CREATE INDEX IF NOT EXISTS idx_connections_org ON _reactor_connect.connections (org_id);
CREATE INDEX IF NOT EXISTS idx_connections_source ON _reactor_connect.connections (source_instance_id);
CREATE INDEX IF NOT EXISTS idx_connections_dest ON _reactor_connect.connections (dest_instance_id);
CREATE INDEX IF NOT EXISTS idx_connections_enabled ON _reactor_connect.connections (enabled) WHERE enabled;

-- 3. Connection state (per-stream cursor / Airbyte state messages)
CREATE TABLE IF NOT EXISTS _reactor_connect.connection_state (
    connection_id            uuid NOT NULL REFERENCES _reactor_connect.connections(id) ON DELETE CASCADE,
    stream_name              text NOT NULL,
    state_json               jsonb NOT NULL,             -- AirbyteStateMessage payload
    updated_at               timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (connection_id, stream_name)
);

-- 4. Sync runs (run history)
CREATE TABLE IF NOT EXISTS _reactor_connect.sync_runs (
    id                       uuid PRIMARY KEY,
    connection_id            uuid NOT NULL REFERENCES _reactor_connect.connections(id) ON DELETE CASCADE,
    org_id                   uuid NOT NULL,
    jobs_run_id              uuid,                       -- reference to reactor-jobs run (if delegated)
    status                   text NOT NULL DEFAULT 'pending', -- pending | running | succeeded | failed | cancelled | blocked_drift
    records_read             jsonb NOT NULL DEFAULT '{}',-- { stream_name: count, ... }
    records_written          jsonb NOT NULL DEFAULT '{}',
    error_code               text,
    error_message            text,
    error_suggested_fix      text,
    started_at               timestamptz,
    finished_at              timestamptz,
    created_at               timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_sync_runs_connection ON _reactor_connect.sync_runs (connection_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_sync_runs_org ON _reactor_connect.sync_runs (org_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_sync_runs_status ON _reactor_connect.sync_runs (status) 
    WHERE status IN ('pending', 'running');

-- 5. Conflict policies (for bidirectional sync)
CREATE TABLE IF NOT EXISTS _reactor_connect.conflict_policies (
    id                       uuid PRIMARY KEY,
    org_id                   uuid NOT NULL,
    connection_a_id          uuid NOT NULL REFERENCES _reactor_connect.connections(id) ON DELETE CASCADE,
    connection_b_id          uuid NOT NULL REFERENCES _reactor_connect.connections(id) ON DELETE CASCADE,
    rules_json               jsonb NOT NULL,             -- [{ stream, policy, tiebreak? }, ...]
    created_at               timestamptz NOT NULL DEFAULT now(),
    updated_at               timestamptz NOT NULL DEFAULT now(),
    UNIQUE (connection_a_id, connection_b_id)
);

CREATE INDEX IF NOT EXISTS idx_conflict_policies_org ON _reactor_connect.conflict_policies (org_id);

-- 6. Receivers (webhook ingress tokens + dispatch config)
CREATE TABLE IF NOT EXISTS _reactor_connect.receivers (
    id                       uuid PRIMARY KEY,
    instance_id              uuid NOT NULL REFERENCES _reactor_connect.instances(id) ON DELETE CASCADE,
    org_id                   uuid NOT NULL,
    webhook_name             text NOT NULL,              -- matches WebhookDescriptor.name
    token                    text NOT NULL UNIQUE,       -- stable ingress URL token
    dispatch_kind            text NOT NULL,              -- 'job' | 'stream' | 'action' | 'function'
    dispatch_config_json     jsonb NOT NULL,             -- { job: 'name' } or { connection: 'name' } etc.
    enabled                  boolean NOT NULL DEFAULT true,
    last_received_at         timestamptz,
    created_at               timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_receivers_instance ON _reactor_connect.receivers (instance_id);
CREATE INDEX IF NOT EXISTS idx_receivers_org ON _reactor_connect.receivers (org_id);
CREATE INDEX IF NOT EXISTS idx_receivers_token ON _reactor_connect.receivers (token) WHERE enabled;

-- 7. Action invocations (lightweight log)
CREATE TABLE IF NOT EXISTS _reactor_connect.action_invocations (
    id                       uuid PRIMARY KEY,
    instance_id              uuid NOT NULL REFERENCES _reactor_connect.instances(id) ON DELETE CASCADE,
    org_id                   uuid NOT NULL,
    action_name              text NOT NULL,
    input_hash               bytea,                      -- sha256 of input for dedup tracking
    idempotency_key          text,
    dry_run                  boolean NOT NULL DEFAULT false,
    status                   text NOT NULL,              -- succeeded | failed
    duration_ms              integer,
    error_code               text,
    error_message            text,
    created_at               timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_action_invocations_instance ON _reactor_connect.action_invocations (instance_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_action_invocations_org ON _reactor_connect.action_invocations (org_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_action_invocations_idempotency ON _reactor_connect.action_invocations (instance_id, idempotency_key) 
    WHERE idempotency_key IS NOT NULL;

-- 8. Audit events
CREATE TABLE IF NOT EXISTS _reactor_connect.audit_events (
    id                       uuid PRIMARY KEY,
    ts                       timestamptz NOT NULL DEFAULT now(),
    actor_user_id            uuid,
    actor_apikey_id          uuid,
    org_id                   uuid,
    instance_id              uuid,
    connection_id            uuid,
    receiver_id              uuid,
    event_type               text NOT NULL,
    details                  jsonb NOT NULL DEFAULT '{}',
    request_id               text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_org ON _reactor_connect.audit_events (org_id, ts DESC);
CREATE INDEX IF NOT EXISTS idx_audit_instance ON _reactor_connect.audit_events (instance_id, ts DESC);

-- 9. Discovered catalogs (cached discover results)
CREATE TABLE IF NOT EXISTS _reactor_connect.discovered_catalogs (
    instance_id              uuid PRIMARY KEY REFERENCES _reactor_connect.instances(id) ON DELETE CASCADE,
    catalog_json             jsonb NOT NULL,             -- [StreamDescriptor, ...]
    discovered_at            timestamptz NOT NULL DEFAULT now(),
    expires_at               timestamptz NOT NULL        -- TTL for cache (typically 1h)
);

-- 10. Sandbox schemas (ephemeral schemas for sandbox runs)
CREATE TABLE IF NOT EXISTS _reactor_connect.sandbox_schemas (
    id                       uuid PRIMARY KEY,
    connection_id            uuid NOT NULL REFERENCES _reactor_connect.connections(id) ON DELETE CASCADE,
    schema_name              text NOT NULL UNIQUE,       -- _sandbox_<uuid>
    promote_token_hash       bytea NOT NULL,             -- sha256 of HMAC-signed promote token
    diff_json                jsonb NOT NULL DEFAULT '{}',-- { stream: { added_columns, type_changes, row_delta }, ... }
    created_at               timestamptz NOT NULL DEFAULT now(),
    expires_at               timestamptz NOT NULL        -- TTL (typically 1h after creation)
);

CREATE INDEX IF NOT EXISTS idx_sandbox_schemas_expires ON _reactor_connect.sandbox_schemas (expires_at);
