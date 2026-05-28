-- Storage policies (object-level access control)
CREATE TABLE _reactor_storage.policies (
    id            BIGSERIAL PRIMARY KEY,
    bucket_id     UUID NOT NULL REFERENCES _reactor_storage.buckets(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,                   -- e.g., 'owner_read', 'public_images'
    scopes        TEXT[] NOT NULL,                 -- {'read','write'}
    using_ast     JSONB,                           -- serialized PolicyExpr for USING clause
    check_ast     JSONB,                           -- serialized PolicyExpr for CHECK clause
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by    UUID,
    UNIQUE (bucket_id, name)
);

CREATE INDEX ON _reactor_storage.policies (bucket_id);
