-- Deployments table (one row per (function, version))

CREATE TABLE _reactor_functions.deployments (
    id                      UUID PRIMARY KEY,
    function_id             UUID NOT NULL REFERENCES _reactor_functions.functions(id) ON DELETE CASCADE,
    version                 BIGINT NOT NULL,                    -- monotonic per function
    bundle_bucket           TEXT NOT NULL,                      -- always '_reactor_functions'
    bundle_object_key       TEXT NOT NULL,                      -- "{function_name}/{version}.zip"
    bundle_sha256           BYTEA NOT NULL,
    bundle_size             BIGINT NOT NULL,
    manifest_json           JSONB NOT NULL,                     -- full validated manifest
    status                  TEXT NOT NULL,                      -- 'pending' | 'ready' | 'failed' | 'destroyed'
    status_detail           TEXT,                               -- error message on failed
    runtime_ref             TEXT,                               -- adapter-specific (Lambda ARN, etc.)
    deployed_at             TIMESTAMPTZ NOT NULL DEFAULT now(),
    deployed_by_user_id     UUID,
    UNIQUE (function_id, version)
);

CREATE INDEX ON _reactor_functions.deployments (function_id, deployed_at DESC);
CREATE INDEX ON _reactor_functions.deployments (status) WHERE status IN ('pending', 'failed');

-- Constraint: status must be a known value
ALTER TABLE _reactor_functions.deployments
    ADD CONSTRAINT ck_deployment_status 
    CHECK (status IN ('pending', 'ready', 'failed', 'destroyed'));

-- Add FK from functions.current_deployment_id to deployments.id
ALTER TABLE _reactor_functions.functions
    ADD CONSTRAINT fk_current_deployment
    FOREIGN KEY (current_deployment_id) REFERENCES _reactor_functions.deployments(id) ON DELETE SET NULL;
