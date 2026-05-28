-- Migration 001: Users and Identities
-- Creates the core user tables for reactor-auth

-- Enable citext extension for case-insensitive email/slug
CREATE EXTENSION IF NOT EXISTS citext;

-- Create the reactor_auth schema
CREATE SCHEMA IF NOT EXISTS reactor_auth;

-- Users table
CREATE TABLE reactor_auth.users (
    id              UUID PRIMARY KEY,
    email           CITEXT UNIQUE NOT NULL,
    email_verified  BOOLEAN NOT NULL DEFAULT FALSE,
    password_hash   TEXT,                               -- nullable for oauth-only users
    metadata        JSONB NOT NULL DEFAULT '{}'::JSONB,
    default_org_id  UUID,                               -- FK added after orgs table
    disabled_at     TIMESTAMPTZ,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Index for finding users by email
CREATE INDEX users_email_idx ON reactor_auth.users (email);

-- Identities table (OAuth provider linkages)
CREATE TABLE reactor_auth.identities (
    id              UUID PRIMARY KEY,
    user_id         UUID NOT NULL REFERENCES reactor_auth.users(id) ON DELETE CASCADE,
    provider        TEXT NOT NULL,                      -- 'google', 'github', 'email'
    provider_uid    TEXT NOT NULL,                      -- subject from IdP
    metadata        JSONB NOT NULL DEFAULT '{}'::JSONB,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider, provider_uid)
);

CREATE INDEX identities_user_id_idx ON reactor_auth.identities (user_id);
