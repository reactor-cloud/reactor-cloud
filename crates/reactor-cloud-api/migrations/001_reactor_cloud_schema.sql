-- Cloud control plane schema for multi-tenant project management
-- This schema is global (not per-tenant) and manages the control plane state

CREATE SCHEMA IF NOT EXISTS reactor_cloud;

-- Projects table: tracks all tenant projects
CREATE TABLE reactor_cloud.projects (
    id            UUID PRIMARY KEY,                      -- ProjectId
    ref           TEXT NOT NULL UNIQUE,                  -- ProjectRef (20 chars)
    name          TEXT NOT NULL,
    owner_user_id UUID NOT NULL,
    backend_kind  TEXT NOT NULL DEFAULT 'dedicated',     -- 'dedicated' | 'shared'
    status        TEXT NOT NULL DEFAULT 'provisioning',  -- project lifecycle state
    region        TEXT NOT NULL DEFAULT 'iad',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT projects_status_check CHECK (status IN ('provisioning','active','suspended','deleting','failed')),
    CONSTRAINT projects_backend_kind_check CHECK (backend_kind IN ('dedicated', 'shared'))
);

CREATE INDEX projects_owner_idx ON reactor_cloud.projects(owner_user_id);
CREATE INDEX projects_status_idx ON reactor_cloud.projects(status);
CREATE INDEX projects_ref_idx ON reactor_cloud.projects(ref);

-- Project members: tracks user membership in projects
CREATE TABLE reactor_cloud.project_members (
    project_id UUID NOT NULL REFERENCES reactor_cloud.projects(id) ON DELETE CASCADE,
    user_id    UUID NOT NULL,
    role       TEXT NOT NULL,                            -- 'owner' | 'admin' | 'member'
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, user_id),
    CONSTRAINT members_role_check CHECK (role IN ('owner','admin','member'))
);

CREATE INDEX members_user_idx ON reactor_cloud.project_members(user_id);

-- Project API keys: tracks issued anon/service keys
CREATE TABLE reactor_cloud.project_keys (
    id          UUID PRIMARY KEY,
    project_id  UUID NOT NULL REFERENCES reactor_cloud.projects(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL,                           -- 'anon' | 'service' | 'jwt-signing'
    vault_ref   TEXT NOT NULL,                           -- e.g. tenant/<id>/keys/anon
    revoked_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT keys_kind_check CHECK (kind IN ('anon','service','jwt-signing'))
);

CREATE INDEX keys_project_kind_idx ON reactor_cloud.project_keys(project_id, kind);
CREATE INDEX keys_active_idx ON reactor_cloud.project_keys(project_id) WHERE revoked_at IS NULL;

-- Audit log: append-only log of all control plane actions
CREATE TABLE reactor_cloud.audit_log (
    id         BIGSERIAL PRIMARY KEY,
    project_id UUID,  -- null for global events
    actor      TEXT NOT NULL,
    action     TEXT NOT NULL,
    metadata   JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX audit_project_created_idx ON reactor_cloud.audit_log(project_id, created_at DESC);
CREATE INDEX audit_action_idx ON reactor_cloud.audit_log(action);
CREATE INDEX audit_created_idx ON reactor_cloud.audit_log(created_at DESC);

-- Trigger to update updated_at on projects
CREATE OR REPLACE FUNCTION reactor_cloud.update_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER projects_updated_at
    BEFORE UPDATE ON reactor_cloud.projects
    FOR EACH ROW
    EXECUTE FUNCTION reactor_cloud.update_updated_at();

-- Comments
COMMENT ON SCHEMA reactor_cloud IS 'Cloud control plane for multi-tenant project management';
COMMENT ON TABLE reactor_cloud.projects IS 'All tenant projects managed by the control plane';
COMMENT ON TABLE reactor_cloud.project_members IS 'User membership in projects';
COMMENT ON TABLE reactor_cloud.project_keys IS 'API keys issued for projects';
COMMENT ON TABLE reactor_cloud.audit_log IS 'Append-only audit log of control plane actions';
COMMENT ON COLUMN reactor_cloud.projects.ref IS 'URL-safe 20-char project reference, used in subdomains';
COMMENT ON COLUMN reactor_cloud.projects.status IS 'Lifecycle state: provisioning -> active -> suspended -> deleting';
COMMENT ON COLUMN reactor_cloud.project_keys.vault_ref IS 'Path to key material in vault (tenant/<id>/keys/<kind>)';
