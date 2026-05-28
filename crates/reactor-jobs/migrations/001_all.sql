-- Reactor Jobs schema migrations
-- All tables in a single file for initial setup

CREATE SCHEMA IF NOT EXISTS _reactor_jobs;

-- 1. Jobs (metadata overlay on functions)
CREATE TABLE IF NOT EXISTS _reactor_jobs.jobs (
    id                       uuid PRIMARY KEY,
    org_id                   uuid NOT NULL,
    name                     text NOT NULL,
    function_name            text NOT NULL,
    description              text,
    retry_max_attempts       integer NOT NULL DEFAULT 3,
    retry_backoff            text NOT NULL DEFAULT 'exponential',
    retry_initial_delay_ms   integer NOT NULL DEFAULT 1000,
    retry_max_delay_ms       integer NOT NULL DEFAULT 60000,
    max_concurrency          integer NOT NULL DEFAULT 10,
    timeout_ms               integer NOT NULL DEFAULT 600000,
    created_at               timestamptz NOT NULL DEFAULT now(),
    updated_at               timestamptz NOT NULL DEFAULT now(),
    UNIQUE (org_id, name)
);

CREATE INDEX IF NOT EXISTS idx_jobs_org ON _reactor_jobs.jobs (org_id);

-- 2. Triggers
CREATE TABLE IF NOT EXISTS _reactor_jobs.triggers (
    id                       uuid PRIMARY KEY,
    job_id                   uuid NOT NULL REFERENCES _reactor_jobs.jobs(id) ON DELETE CASCADE,
    kind                     text NOT NULL,
    config_json              jsonb NOT NULL DEFAULT '{}',
    webhook_token            text UNIQUE,
    enabled                  boolean NOT NULL DEFAULT true,
    last_triggered_at        timestamptz,
    next_trigger_at          timestamptz,
    created_at               timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_triggers_job ON _reactor_jobs.triggers (job_id);
CREATE INDEX IF NOT EXISTS idx_triggers_cron ON _reactor_jobs.triggers (kind, next_trigger_at) 
    WHERE kind = 'cron' AND enabled;
CREATE INDEX IF NOT EXISTS idx_triggers_event ON _reactor_jobs.triggers (kind, config_json) 
    WHERE kind = 'event' AND enabled;

-- 3. Runs
CREATE TABLE IF NOT EXISTS _reactor_jobs.runs (
    id                       uuid PRIMARY KEY,
    job_id                   uuid NOT NULL REFERENCES _reactor_jobs.jobs(id) ON DELETE CASCADE,
    org_id                   uuid NOT NULL,
    trigger_id               uuid REFERENCES _reactor_jobs.triggers(id) ON DELETE SET NULL,
    trigger_kind             text NOT NULL,
    status                   text NOT NULL DEFAULT 'pending',
    payload_json             jsonb NOT NULL DEFAULT '{}',
    attempt                  integer NOT NULL DEFAULT 1,
    max_attempts             integer NOT NULL,
    started_at               timestamptz,
    finished_at              timestamptz,
    wakeup_at                timestamptz,
    error_code               text,
    error_message            text,
    created_at               timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_runs_job ON _reactor_jobs.runs (job_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_runs_org ON _reactor_jobs.runs (org_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_runs_status ON _reactor_jobs.runs (status) 
    WHERE status IN ('pending', 'running', 'sleeping', 'queued');
CREATE INDEX IF NOT EXISTS idx_runs_wakeup ON _reactor_jobs.runs (wakeup_at) 
    WHERE status = 'sleeping';

-- 4. Steps
CREATE TABLE IF NOT EXISTS _reactor_jobs.steps (
    id                       uuid PRIMARY KEY,
    run_id                   uuid NOT NULL REFERENCES _reactor_jobs.runs(id) ON DELETE CASCADE,
    name                     text NOT NULL,
    status                   text NOT NULL DEFAULT 'pending',
    input_json               jsonb,
    output_json              jsonb,
    attempt                  integer NOT NULL DEFAULT 1,
    started_at               timestamptz,
    finished_at              timestamptz,
    error_message            text,
    UNIQUE (run_id, name)
);

CREATE INDEX IF NOT EXISTS idx_steps_run ON _reactor_jobs.steps (run_id);

-- 5. State (per-run KV)
CREATE TABLE IF NOT EXISTS _reactor_jobs.state (
    run_id                   uuid NOT NULL REFERENCES _reactor_jobs.runs(id) ON DELETE CASCADE,
    key                      text NOT NULL,
    value_json               jsonb NOT NULL,
    updated_at               timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (run_id, key)
);

-- 6. Events (internal pub-sub)
CREATE TABLE IF NOT EXISTS _reactor_jobs.events (
    id                       uuid PRIMARY KEY,
    org_id                   uuid NOT NULL,
    topic                    text NOT NULL,
    payload_json             jsonb NOT NULL DEFAULT '{}',
    emitted_by_run_id        uuid REFERENCES _reactor_jobs.runs(id) ON DELETE SET NULL,
    consumed_by_run_id       uuid REFERENCES _reactor_jobs.runs(id) ON DELETE SET NULL,
    created_at               timestamptz NOT NULL DEFAULT now(),
    consumed_at              timestamptz
);

CREATE INDEX IF NOT EXISTS idx_events_pending ON _reactor_jobs.events (org_id, topic, created_at) 
    WHERE consumed_at IS NULL;

-- 7. DLQ (dead letter queue)
CREATE TABLE IF NOT EXISTS _reactor_jobs.dlq (
    id                       uuid PRIMARY KEY,
    run_id                   uuid NOT NULL,
    job_id                   uuid NOT NULL REFERENCES _reactor_jobs.jobs(id) ON DELETE CASCADE,
    org_id                   uuid NOT NULL,
    payload_json             jsonb NOT NULL,
    error_code               text,
    error_message            text,
    attempt                  integer NOT NULL,
    created_at               timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_dlq_job ON _reactor_jobs.dlq (job_id, created_at DESC);

-- 8. Audit events
CREATE TABLE IF NOT EXISTS _reactor_jobs.audit_events (
    id                       uuid PRIMARY KEY,
    ts                       timestamptz NOT NULL DEFAULT now(),
    actor_user_id            uuid,
    actor_apikey_id          uuid,
    org_id                   uuid,
    job_id                   uuid,
    run_id                   uuid,
    event_type               text NOT NULL,
    details                  jsonb NOT NULL DEFAULT '{}',
    request_id               text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_org ON _reactor_jobs.audit_events (org_id, ts DESC);
CREATE INDEX IF NOT EXISTS idx_audit_job ON _reactor_jobs.audit_events (job_id, ts DESC);
