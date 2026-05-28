-- Migration: 001_metadata
-- Description: Projects, keys, identities, and consent tombstones

-- Create the analytics schema
CREATE SCHEMA IF NOT EXISTS _reactor_analytics;

-- Projects table
CREATE TABLE _reactor_analytics.projects (
  id           uuid PRIMARY KEY,
  org_id       uuid NOT NULL,
  name         text NOT NULL,
  created_at   timestamptz NOT NULL DEFAULT now(),
  deleted_at   timestamptz,
  UNIQUE (org_id, name)
);
CREATE INDEX ON _reactor_analytics.projects (org_id);

-- Project API keys table
CREATE TABLE _reactor_analytics.project_keys (
  id              uuid PRIMARY KEY,
  project_id      uuid NOT NULL REFERENCES _reactor_analytics.projects(id) ON DELETE CASCADE,
  key_prefix      text NOT NULL,           -- 'rapk_'
  key_hash        bytea NOT NULL,          -- argon2id hash of the full key
  key_last4       text NOT NULL,           -- for UI display
  name            text NOT NULL,           -- 'web-prod', 'web-staging'
  sampling_rate   double precision NOT NULL DEFAULT 1.0 CHECK (sampling_rate BETWEEN 0 AND 1),
  allowed_origins text[],                  -- nullable = no CORS check
  created_at      timestamptz NOT NULL DEFAULT now(),
  revoked_at      timestamptz
);
CREATE UNIQUE INDEX ON _reactor_analytics.project_keys (key_hash);
CREATE INDEX ON _reactor_analytics.project_keys (project_id);

-- Identities table (maps anonymous IDs to user IDs)
CREATE TABLE _reactor_analytics.identities (
  org_id          uuid NOT NULL,
  project_id      uuid NOT NULL,
  anonymous_id    text NOT NULL,
  user_id         text,                    -- nullable until identified
  first_seen_at   timestamptz NOT NULL DEFAULT now(),
  last_seen_at    timestamptz NOT NULL DEFAULT now(),
  traits          jsonb NOT NULL DEFAULT '{}'::jsonb,
  PRIMARY KEY (project_id, anonymous_id)
);
CREATE INDEX ON _reactor_analytics.identities (project_id, user_id) WHERE user_id IS NOT NULL;

-- Consent tombstones (opt-out tracking)
CREATE TABLE _reactor_analytics.consent_tombstones (
  project_id      uuid NOT NULL,
  anonymous_id    text NOT NULL,
  reason          text NOT NULL,           -- 'opt_out' | 'dnt' | 'erased'
  created_at      timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (project_id, anonymous_id)
);
