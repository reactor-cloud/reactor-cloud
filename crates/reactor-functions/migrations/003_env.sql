-- Per-function environment variables and secrets

CREATE TABLE _reactor_functions.env (
    function_id             UUID NOT NULL REFERENCES _reactor_functions.functions(id) ON DELETE CASCADE,
    key                     TEXT NOT NULL,
    value_plaintext         TEXT,                               -- non-secret values
    value_encrypted         BYTEA,                              -- AES-GCM encrypted with REACTOR_FUNCTIONS_DATA_KEY
    is_secret               BOOLEAN NOT NULL DEFAULT FALSE,
    last_updated_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (function_id, key),
    CONSTRAINT ck_env_value CHECK (
        (is_secret AND value_encrypted IS NOT NULL AND value_plaintext IS NULL)
        OR (NOT is_secret AND value_plaintext IS NOT NULL AND value_encrypted IS NULL)
    )
);

CREATE INDEX ON _reactor_functions.env (function_id);

-- Constraint: env keys must be uppercase alphanumeric with underscores
ALTER TABLE _reactor_functions.env
    ADD CONSTRAINT ck_env_key 
    CHECK (key ~ '^[A-Z][A-Z0-9_]{0,127}$');
