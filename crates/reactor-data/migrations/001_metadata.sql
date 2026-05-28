-- Reactor Data metadata schema
-- This schema is internal to reactor-data and not accessible to user app roles.

CREATE SCHEMA IF NOT EXISTS _reactor_data;

-- Migration history for user-defined migrations (not this file)
CREATE TABLE _reactor_data.migrations (
    version       TEXT PRIMARY KEY,           -- e.g., '001_init'
    source_sha256 BYTEA NOT NULL,             -- sha256 of raw migration source
    applied_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    applied_by    TEXT                        -- user/process that ran the migration
);

-- Table introspection cache (refreshed on migration apply)
CREATE TABLE _reactor_data.tables (
    table_schema  TEXT NOT NULL,
    table_name    TEXT NOT NULL,
    columns_json  JSONB NOT NULL,             -- [{ name, type, nullable, default }, ...]
    primary_key   TEXT[] NOT NULL,
    foreign_keys  JSONB NOT NULL,             -- [{ name, columns, ref_table, ref_columns, on_delete }, ...]
    indexes       JSONB NOT NULL,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (table_schema, table_name)
);

CREATE INDEX ON _reactor_data.tables (table_schema);
