-- Compiled policies (one row per policy declaration)
CREATE TABLE _reactor_data.policies (
    id            BIGSERIAL PRIMARY KEY,
    schema_name   TEXT NOT NULL,              -- 'public', 'app', etc.
    table_name    TEXT NOT NULL,
    name          TEXT NOT NULL,              -- 'todos_tenant'
    scopes        TEXT[] NOT NULL,            -- {'select','update','delete'}
    using_ast     JSONB,                      -- serialized PolicyExpr for USING clause
    check_ast     JSONB,                      -- serialized PolicyExpr for CHECK clause
    migration_name TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (schema_name, table_name, name)
);

CREATE INDEX ON _reactor_data.policies (schema_name, table_name);

-- Registered RPC functions
CREATE TABLE _reactor_data.rpc_functions (
    id            BIGSERIAL PRIMARY KEY,
    schema_name   TEXT NOT NULL,
    name          TEXT NOT NULL,
    params        JSONB NOT NULL DEFAULT '[]', -- [{ name, sql_type, has_default, position }, ...]
    return_type   TEXT NOT NULL,               -- 'void', 'record', 'setof record', specific type
    returns_set   BOOLEAN NOT NULL DEFAULT FALSE,
    body          TEXT NOT NULL,               -- SQL body of the function
    security      TEXT NOT NULL DEFAULT 'definer', -- 'definer' | 'invoker'
    migration_name TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (schema_name, name)
);

CREATE INDEX ON _reactor_data.rpc_functions (schema_name, name);
