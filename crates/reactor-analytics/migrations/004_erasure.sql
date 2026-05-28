-- Migration: 004_erasure
-- Description: Erasure log table for GDPR audit trail

CREATE TABLE _reactor_analytics.erasures (
  id              uuid PRIMARY KEY,
  project_id      uuid NOT NULL,
  subject_kind    text NOT NULL,           -- 'user' | 'anonymous'
  subject_id      text NOT NULL,
  rows_deleted    bigint NOT NULL,
  actor_user_id   uuid,
  request_id      text NOT NULL,
  created_at      timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX ON _reactor_analytics.erasures (project_id, created_at DESC);
