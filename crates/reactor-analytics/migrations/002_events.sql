-- Migration: 002_events
-- Description: Partitioned events table with hot columns and indexes

-- Events table (partitioned by received_at)
CREATE TABLE _reactor_analytics.events (
  id              uuid NOT NULL,
  received_at     timestamptz NOT NULL,
  timestamp       timestamptz NOT NULL,
  org_id          uuid NOT NULL,
  project_id      uuid NOT NULL,
  event           text NOT NULL,
  anonymous_id    text NOT NULL,
  user_id         text,
  session_id      text,
  -- Hot columns for 80% of queries
  url             text,
  path            text,
  referrer_host   text,
  utm_source      text,
  country         text,
  device_type     text,
  ingest_ip_h24   text,
  library_name    text,
  library_version text,
  -- Everything else in jsonb
  properties      jsonb NOT NULL DEFAULT '{}'::jsonb,
  context         jsonb NOT NULL DEFAULT '{}'::jsonb,
  PRIMARY KEY (received_at, id)
) PARTITION BY RANGE (received_at);

-- Default partition catches mis-clocked clients
CREATE TABLE _reactor_analytics.events_default PARTITION OF _reactor_analytics.events DEFAULT;

-- Create partitions for current and next month
DO $$
DECLARE
  current_month_start DATE := date_trunc('month', NOW());
  next_month_start DATE := date_trunc('month', NOW() + interval '1 month');
  following_month_start DATE := date_trunc('month', NOW() + interval '2 months');
  current_partition TEXT;
  next_partition TEXT;
BEGIN
  -- Current month partition (e.g., events_2026_05)
  current_partition := 'events_' || to_char(current_month_start, 'YYYY_MM');
  EXECUTE format(
    'CREATE TABLE IF NOT EXISTS _reactor_analytics.%I PARTITION OF _reactor_analytics.events 
     FOR VALUES FROM (%L) TO (%L)',
    current_partition,
    current_month_start,
    next_month_start
  );

  -- Next month partition (e.g., events_2026_06)
  next_partition := 'events_' || to_char(next_month_start, 'YYYY_MM');
  EXECUTE format(
    'CREATE TABLE IF NOT EXISTS _reactor_analytics.%I PARTITION OF _reactor_analytics.events 
     FOR VALUES FROM (%L) TO (%L)',
    next_partition,
    next_month_start,
    following_month_start
  );
END $$;

-- BRIN index for time-based scans (efficient for append-only)
CREATE INDEX ON _reactor_analytics.events USING brin (received_at);

-- B-tree indexes for common query patterns
CREATE INDEX ON _reactor_analytics.events (project_id, received_at DESC);
CREATE INDEX ON _reactor_analytics.events (project_id, event, received_at DESC);
CREATE INDEX ON _reactor_analytics.events (project_id, user_id, received_at DESC)
  WHERE user_id IS NOT NULL;
CREATE INDEX ON _reactor_analytics.events (project_id, anonymous_id, received_at DESC);

-- Index for hot columns used in common filters
CREATE INDEX ON _reactor_analytics.events (project_id, path, received_at DESC)
  WHERE path IS NOT NULL;
CREATE INDEX ON _reactor_analytics.events (project_id, utm_source, received_at DESC)
  WHERE utm_source IS NOT NULL;
CREATE INDEX ON _reactor_analytics.events (project_id, country, received_at DESC)
  WHERE country IS NOT NULL;
