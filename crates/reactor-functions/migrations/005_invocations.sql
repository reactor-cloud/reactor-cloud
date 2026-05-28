-- Invocations log (lightweight, per-invoke record)

CREATE TABLE _reactor_functions.invocations (
    id                      UUID PRIMARY KEY,
    deployment_id           UUID NOT NULL REFERENCES _reactor_functions.deployments(id) ON DELETE CASCADE,
    function_id             UUID NOT NULL,                      -- denormalised for query speed
    org_id                  UUID NOT NULL,
    actor_user_id           UUID,
    actor_apikey_id         UUID,
    request_id              TEXT NOT NULL,
    method                  TEXT NOT NULL,
    sub_path                TEXT NOT NULL,
    status_code             INTEGER NOT NULL,
    duration_ms             INTEGER NOT NULL,
    cold_start              BOOLEAN NOT NULL DEFAULT FALSE,
    bytes_in                BIGINT NOT NULL DEFAULT 0,
    bytes_out               BIGINT NOT NULL DEFAULT 0,
    error_code              TEXT,                               -- platform error code if any
    started_at              TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX ON _reactor_functions.invocations (function_id, started_at DESC);
CREATE INDEX ON _reactor_functions.invocations (org_id, started_at DESC);
CREATE INDEX ON _reactor_functions.invocations (deployment_id, started_at DESC);
CREATE INDEX ON _reactor_functions.invocations (status_code, started_at DESC) WHERE status_code >= 500;
