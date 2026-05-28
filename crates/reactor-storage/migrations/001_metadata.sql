-- Reactor Storage metadata schema
-- This schema is internal to reactor-storage and not accessible to user app roles.

CREATE SCHEMA IF NOT EXISTS _reactor_storage;

-- Buckets table
CREATE TABLE _reactor_storage.buckets (
    id            UUID PRIMARY KEY,
    org_id        UUID NOT NULL,
    slug          TEXT NOT NULL,                   -- e.g., 'avatars', 'documents'
    is_public     BOOLEAN NOT NULL DEFAULT FALSE,  -- public buckets allow anonymous read
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by    UUID,                            -- user_id who created
    UNIQUE (org_id, slug)
);

CREATE INDEX ON _reactor_storage.buckets (org_id);

-- Objects table (metadata only, blobs are in FS or S3)
CREATE TABLE _reactor_storage.objects (
    id            UUID PRIMARY KEY,
    bucket_id     UUID NOT NULL REFERENCES _reactor_storage.buckets(id) ON DELETE CASCADE,
    key           TEXT NOT NULL,                   -- object path within bucket
    content_type  TEXT,                            -- MIME type
    content_length BIGINT NOT NULL,
    etag          TEXT,                            -- content hash for cache validation
    metadata      JSONB NOT NULL DEFAULT '{}',     -- user-defined metadata
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by    UUID,                            -- user_id who uploaded
    UNIQUE (bucket_id, key)
);

CREATE INDEX ON _reactor_storage.objects (bucket_id);
CREATE INDEX ON _reactor_storage.objects (bucket_id, key);
