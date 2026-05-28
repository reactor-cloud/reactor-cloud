-- Reactor Functions metadata schema
-- This schema is internal to reactor-functions and not accessible to user app roles.

CREATE SCHEMA IF NOT EXISTS _reactor_functions;

-- Enable citext extension for case-insensitive function names
CREATE EXTENSION IF NOT EXISTS citext;

-- Functions table
CREATE TABLE _reactor_functions.functions (
    id                      UUID PRIMARY KEY,
    org_id                  UUID NOT NULL,
    name                    CITEXT NOT NULL,
    description             TEXT,
    runtime                 TEXT NOT NULL,                      -- 'wasm' | 'bun' | 'lambda'
    current_deployment_id   UUID,                               -- FK; null until first promote
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (org_id, name)
);

CREATE INDEX ON _reactor_functions.functions (org_id);

-- Constraint: function names must be lowercase alphanumeric with hyphens, 3-63 chars
ALTER TABLE _reactor_functions.functions
    ADD CONSTRAINT ck_function_name 
    CHECK (name ~ '^[a-z][a-z0-9-]{1,61}[a-z0-9]$');

-- Constraint: runtime must be a known value
ALTER TABLE _reactor_functions.functions
    ADD CONSTRAINT ck_function_runtime 
    CHECK (runtime IN ('wasm', 'bun', 'lambda'));
