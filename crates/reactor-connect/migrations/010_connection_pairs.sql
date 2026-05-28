-- Connection pairs table for bidirectional sync coordination
-- Links two connections that replicate data in opposite directions

CREATE TABLE IF NOT EXISTS _reactor_connect.connection_pairs (
    id                       uuid PRIMARY KEY,
    org_id                   uuid NOT NULL,
    name                     text NOT NULL,
    
    -- The two connections (A → B and B → A)
    connection_a_id          uuid NOT NULL REFERENCES _reactor_connect.connections(id) ON DELETE CASCADE,
    connection_b_id          uuid NOT NULL REFERENCES _reactor_connect.connections(id) ON DELETE CASCADE,
    
    -- Conflict resolution policy
    conflict_policy_id       uuid REFERENCES _reactor_connect.conflict_policies(id),
    
    -- Loop protection settings
    loop_protection_enabled  boolean NOT NULL DEFAULT true,
    loop_protection_window   interval NOT NULL DEFAULT '5 minutes',
    
    -- Status
    enabled                  boolean NOT NULL DEFAULT true,
    
    -- Timestamps
    created_at               timestamptz NOT NULL DEFAULT now(),
    updated_at               timestamptz NOT NULL DEFAULT now(),
    
    CONSTRAINT unique_pair_name_per_org UNIQUE (org_id, name),
    CONSTRAINT different_connections CHECK (connection_a_id != connection_b_id)
);

CREATE INDEX IF NOT EXISTS idx_connection_pairs_org ON _reactor_connect.connection_pairs (org_id);
CREATE INDEX IF NOT EXISTS idx_connection_pairs_conn_a ON _reactor_connect.connection_pairs (connection_a_id);
CREATE INDEX IF NOT EXISTS idx_connection_pairs_conn_b ON _reactor_connect.connection_pairs (connection_b_id);

-- Track synced records to prevent loops
-- This is ephemeral - cleaned up by TTL worker, also stored in reactor-cache KV
CREATE TABLE IF NOT EXISTS _reactor_connect.sync_loop_markers (
    id                       uuid PRIMARY KEY,
    pair_id                  uuid NOT NULL REFERENCES _reactor_connect.connection_pairs(id) ON DELETE CASCADE,
    stream_name              text NOT NULL,
    record_key               jsonb NOT NULL,
    origin_connection_id     uuid NOT NULL,
    synced_at                timestamptz NOT NULL DEFAULT now(),
    expires_at               timestamptz NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sync_loop_markers_pair ON _reactor_connect.sync_loop_markers (pair_id, stream_name, record_key);
CREATE INDEX IF NOT EXISTS idx_sync_loop_markers_expiry ON _reactor_connect.sync_loop_markers (expires_at);

-- Add pair_id to connections for easy lookup
ALTER TABLE _reactor_connect.connections 
ADD COLUMN IF NOT EXISTS pair_id uuid REFERENCES _reactor_connect.connection_pairs(id);
