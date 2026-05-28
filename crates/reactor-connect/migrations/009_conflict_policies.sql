-- Conflict policies table
-- Defines how conflicts are resolved during bidirectional sync

CREATE TABLE IF NOT EXISTS _reactor_connect.conflict_policies (
    id                       uuid PRIMARY KEY,
    org_id                   uuid NOT NULL,
    name                     text NOT NULL,
    description              text,
    
    -- Policy rules (DSL stored as JSON)
    rules_json               jsonb NOT NULL,
    
    -- Named policy shortcuts
    named_policy             text,             -- 'source_wins' | 'dest_wins' | 'latest_wins' | 'custom'
    
    -- Status
    is_default               boolean NOT NULL DEFAULT false,
    enabled                  boolean NOT NULL DEFAULT true,
    
    -- Timestamps
    created_at               timestamptz NOT NULL DEFAULT now(),
    updated_at               timestamptz NOT NULL DEFAULT now(),
    
    CONSTRAINT unique_policy_name_per_org UNIQUE (org_id, name)
);

CREATE INDEX IF NOT EXISTS idx_conflict_policies_org ON _reactor_connect.conflict_policies (org_id);
CREATE INDEX IF NOT EXISTS idx_conflict_policies_default ON _reactor_connect.conflict_policies (org_id, is_default) WHERE is_default = true;

-- Associate policies with connections
ALTER TABLE _reactor_connect.connections 
ADD COLUMN IF NOT EXISTS conflict_policy_id uuid REFERENCES _reactor_connect.conflict_policies(id);

-- Track conflict resolution events
CREATE TABLE IF NOT EXISTS _reactor_connect.conflict_resolution_log (
    id                       uuid PRIMARY KEY,
    connection_id            uuid NOT NULL REFERENCES _reactor_connect.connections(id) ON DELETE CASCADE,
    org_id                   uuid NOT NULL,
    policy_id                uuid REFERENCES _reactor_connect.conflict_policies(id),
    
    -- Conflict details
    stream_name              text NOT NULL,
    record_key               jsonb NOT NULL,       -- Primary key of the conflicting record
    
    -- Resolution
    source_a_value           jsonb,
    source_b_value           jsonb,
    resolved_value           jsonb,
    resolution_rule          text NOT NULL,        -- Which rule was applied
    
    -- Timestamps
    resolved_at              timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_conflict_log_connection ON _reactor_connect.conflict_resolution_log (connection_id, resolved_at DESC);
CREATE INDEX IF NOT EXISTS idx_conflict_log_org ON _reactor_connect.conflict_resolution_log (org_id, resolved_at DESC);
