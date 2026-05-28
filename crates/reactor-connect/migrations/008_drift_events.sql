-- Schema drift events table
-- Tracks schema changes that require approval before sync can proceed

CREATE TABLE IF NOT EXISTS _reactor_connect.connection_drift_events (
    id                       uuid PRIMARY KEY,
    connection_id            uuid NOT NULL REFERENCES _reactor_connect.connections(id) ON DELETE CASCADE,
    org_id                   uuid NOT NULL,
    
    -- Drift detection
    stream_name              text NOT NULL,
    drift_type               text NOT NULL,          -- 'column_added' | 'column_removed' | 'type_changed' | 'primary_key_changed'
    severity                 text NOT NULL,          -- 'info' | 'warning' | 'breaking'
    
    -- Change details
    details_json             jsonb NOT NULL,         -- { column_name, old_type, new_type, ... }
    
    -- Approval status
    status                   text NOT NULL DEFAULT 'pending',  -- 'pending' | 'approved' | 'rejected'
    decided_by               uuid,                   -- User who approved/rejected
    decided_at               timestamptz,
    decision_reason          text,
    
    -- Timestamps
    detected_at              timestamptz NOT NULL DEFAULT now(),
    created_at               timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_drift_events_connection ON _reactor_connect.connection_drift_events (connection_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_drift_events_org ON _reactor_connect.connection_drift_events (org_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_drift_events_pending ON _reactor_connect.connection_drift_events (connection_id, status) WHERE status = 'pending';
