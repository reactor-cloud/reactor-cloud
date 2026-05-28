-- Audit events (admin actions only — invocations live in _reactor_functions.invocations)

CREATE TABLE _reactor_functions.audit_events (
    id                      UUID PRIMARY KEY,
    ts                      TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor_user_id           UUID,
    actor_apikey_id         UUID,
    org_id                  UUID,
    function_id             UUID,
    deployment_id           UUID,
    event_type              TEXT NOT NULL,                      -- 'function.create', 'deployment.create', 'deployment.promote', etc.
    details                 JSONB NOT NULL DEFAULT '{}'::JSONB,
    request_id              TEXT NOT NULL
);

CREATE INDEX ON _reactor_functions.audit_events (org_id, ts DESC);
CREATE INDEX ON _reactor_functions.audit_events (function_id, ts DESC);

-- Constraint: event_type must be a known prefix
ALTER TABLE _reactor_functions.audit_events
    ADD CONSTRAINT ck_audit_event_type 
    CHECK (event_type ~ '^(function\.|deployment\.|env\.|policy\.)[a-z_]+$');
