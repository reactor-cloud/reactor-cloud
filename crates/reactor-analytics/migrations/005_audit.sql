-- Migration: 005_audit
-- Description: Audit events table for admin actions

CREATE TABLE _reactor_analytics.audit_events (
  id              uuid PRIMARY KEY,
  timestamp       timestamptz NOT NULL DEFAULT now(),
  actor_user_id   uuid,
  actor_apikey_id uuid,
  org_id          uuid,
  project_id      uuid,
  event_type      text NOT NULL,
  details         jsonb NOT NULL DEFAULT '{}'::jsonb,
  request_id      text NOT NULL
);

CREATE INDEX ON _reactor_analytics.audit_events (org_id, timestamp DESC);
CREATE INDEX ON _reactor_analytics.audit_events (project_id, timestamp DESC);
CREATE INDEX ON _reactor_analytics.audit_events (event_type, timestamp DESC);
