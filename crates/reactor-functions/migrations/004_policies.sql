-- Invoke policies (per-function authorization rules)

CREATE TABLE _reactor_functions.policies (
    id                      UUID PRIMARY KEY,
    function_id             UUID NOT NULL REFERENCES _reactor_functions.functions(id) ON DELETE CASCADE,
    name                    TEXT NOT NULL,
    using_expr_json         JSONB,                              -- PolicyExpr; evaluated for invoke
    raw_text                TEXT NOT NULL,
    sha256                  BYTEA NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (function_id, name)
);

CREATE INDEX ON _reactor_functions.policies (function_id);
