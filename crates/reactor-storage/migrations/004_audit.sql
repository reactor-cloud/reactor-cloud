-- Audit events for storage operations
CREATE TABLE _reactor_storage.audit_events (
    id            BIGSERIAL PRIMARY KEY,
    event_type    TEXT NOT NULL,                    -- 'bucket.create', 'object.put', 'object.delete', etc.
    bucket_id     UUID,                             -- may be null for cross-bucket events
    object_key    TEXT,                             -- may be null for bucket-level events
    actor_user_id UUID,                             -- may be null for anonymous/system
    actor_org_id  UUID,
    request_id    TEXT,                             -- X-Request-Id for correlation
    metadata      JSONB NOT NULL DEFAULT '{}',      -- event-specific details
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ON _reactor_storage.audit_events (bucket_id);
CREATE INDEX ON _reactor_storage.audit_events (actor_user_id);
CREATE INDEX ON _reactor_storage.audit_events (created_at);
CREATE INDEX ON _reactor_storage.audit_events (event_type, created_at);
