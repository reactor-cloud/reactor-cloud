-- Multipart uploads tracking
CREATE TABLE _reactor_storage.multipart_uploads (
    id            UUID PRIMARY KEY,                 -- upload_id returned to client
    bucket_id     UUID NOT NULL REFERENCES _reactor_storage.buckets(id) ON DELETE CASCADE,
    key           TEXT NOT NULL,                    -- target object key
    content_type  TEXT,
    metadata      JSONB NOT NULL DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by    UUID,
    expires_at    TIMESTAMPTZ                       -- auto-cleanup incomplete uploads
);

CREATE INDEX ON _reactor_storage.multipart_uploads (bucket_id);
CREATE INDEX ON _reactor_storage.multipart_uploads (expires_at);

-- Individual parts of multipart uploads
CREATE TABLE _reactor_storage.multipart_parts (
    upload_id     UUID NOT NULL REFERENCES _reactor_storage.multipart_uploads(id) ON DELETE CASCADE,
    part_number   INTEGER NOT NULL,
    etag          TEXT NOT NULL,                    -- content hash for this part
    size          BIGINT NOT NULL,
    uploaded_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (upload_id, part_number)
);
