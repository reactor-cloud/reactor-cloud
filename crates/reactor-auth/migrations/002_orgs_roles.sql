-- Migration 002: Organizations, Roles, and Memberships
-- Creates the multi-tenancy spine for reactor-auth

-- Organizations (tenancy boundary)
CREATE TABLE reactor_auth.orgs (
    id              UUID PRIMARY KEY,
    slug            CITEXT UNIQUE NOT NULL,             -- url-safe, user-chosen
    name            TEXT NOT NULL,
    metadata        JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX orgs_slug_idx ON reactor_auth.orgs (slug);

-- Add FK from users to orgs for default_org_id
ALTER TABLE reactor_auth.users
    ADD CONSTRAINT users_default_org_fk
    FOREIGN KEY (default_org_id) REFERENCES reactor_auth.orgs(id) ON DELETE SET NULL;

-- Roles (per-org, project-defined)
CREATE TABLE reactor_auth.roles (
    id              UUID PRIMARY KEY,
    org_id          UUID NOT NULL REFERENCES reactor_auth.orgs(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,                      -- 'owner', 'admin', 'member', or custom
    description     TEXT,
    is_system       BOOLEAN NOT NULL DEFAULT FALSE,     -- system roles cannot be deleted
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (org_id, name)
);

CREATE INDEX roles_org_id_idx ON reactor_auth.roles (org_id);

-- Role permissions
CREATE TABLE reactor_auth.role_permissions (
    role_id         UUID NOT NULL REFERENCES reactor_auth.roles(id) ON DELETE CASCADE,
    permission      TEXT NOT NULL,                      -- 'data:todos:read', '*'
    PRIMARY KEY (role_id, permission)
);

-- Memberships (user <-> org relationship)
CREATE TABLE reactor_auth.memberships (
    user_id         UUID NOT NULL REFERENCES reactor_auth.users(id) ON DELETE CASCADE,
    org_id          UUID NOT NULL REFERENCES reactor_auth.orgs(id) ON DELETE CASCADE,
    role_id         UUID NOT NULL REFERENCES reactor_auth.roles(id) ON DELETE RESTRICT,
    joined_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (user_id, org_id)
);

CREATE INDEX memberships_org_id_idx ON reactor_auth.memberships (org_id);
CREATE INDEX memberships_user_id_idx ON reactor_auth.memberships (user_id);
