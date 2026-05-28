-- Migration 006: Audit Events
-- Creates the audit log table for reactor-auth

-- Audit events (write-only)
CREATE TABLE reactor_auth.audit_events (
    id              UUID PRIMARY KEY,
    ts              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    actor_user_id   UUID,
    actor_apikey_id UUID,
    org_id          UUID,
    event_type      TEXT NOT NULL,                      -- 'user.signup', 'session.created', etc.
    resource        TEXT,
    ip              INET,
    user_agent      TEXT,
    details         JSONB NOT NULL DEFAULT '{}'::JSONB
);

CREATE INDEX audit_events_org_ts_idx ON reactor_auth.audit_events (org_id, ts DESC);
CREATE INDEX audit_events_user_ts_idx ON reactor_auth.audit_events (actor_user_id, ts DESC);
CREATE INDEX audit_events_type_ts_idx ON reactor_auth.audit_events (event_type, ts DESC);
