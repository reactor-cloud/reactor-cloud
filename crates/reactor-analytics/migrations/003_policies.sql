-- Migration: 003_policies
-- Description: Policy configuration table for analytics

-- Store policy definitions for analytics resources
CREATE TABLE _reactor_analytics.policies (
  id              uuid PRIMARY KEY,
  project_id      uuid NOT NULL REFERENCES _reactor_analytics.projects(id) ON DELETE CASCADE,
  resource_type   text NOT NULL,           -- 'events', 'query', 'export', 'erase'
  policy_expr     text NOT NULL,           -- Policy expression
  description     text,
  created_at      timestamptz NOT NULL DEFAULT now(),
  updated_at      timestamptz NOT NULL DEFAULT now()
);

-- Index for fast policy lookup
CREATE INDEX ON _reactor_analytics.policies (project_id, resource_type);

-- Default policies for a project can be inserted at project creation time
