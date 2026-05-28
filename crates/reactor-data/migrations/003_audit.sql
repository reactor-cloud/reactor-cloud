-- Mutation audit events (mirrors reactor_auth.audit_events shape)
CREATE TABLE _reactor_data.audit_events (
    id              UUID PRIMARY KEY,
    ts              TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor_user_id   UUID,
    actor_apikey_id UUID,
    org_id          UUID,
    request_id      TEXT NOT NULL,
    event_type      TEXT NOT NULL,            -- 'rows.insert' | 'rows.update' | 'rows.delete' | 'rpc.invoke'
    table_name      TEXT,
    row_count       INTEGER,
    details         JSONB NOT NULL DEFAULT '{}'::JSONB
);

CREATE INDEX ON _reactor_data.audit_events (org_id, ts DESC);
CREATE INDEX ON _reactor_data.audit_events (actor_user_id, ts DESC);
CREATE INDEX ON _reactor_data.audit_events (ts DESC);
