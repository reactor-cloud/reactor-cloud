-- Gateway schema for routing tables and custom domains
-- This schema manages edge gateway routing configuration

CREATE SCHEMA IF NOT EXISTS reactor_gateway;

-- Routes table: maps hostnames to backend targets
CREATE TABLE reactor_gateway.routes (
    host TEXT PRIMARY KEY,
    project_id UUID NOT NULL,
    project_ref TEXT NOT NULL,
    backend_kind TEXT NOT NULL DEFAULT 'dedicated',  -- 'dedicated' | 'shared' (only 'dedicated' in Phase 2)
    backend_target TEXT NOT NULL,                    -- internal address, e.g. "reactor-cloud.internal:8000"
    tls_mode TEXT NOT NULL DEFAULT 'wildcard',       -- 'wildcard' | 'on_demand' | 'manual'
    enabled BOOLEAN NOT NULL DEFAULT true,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    CONSTRAINT routes_backend_kind_check CHECK (backend_kind IN ('dedicated', 'shared')),
    CONSTRAINT routes_tls_mode_check CHECK (tls_mode IN ('wildcard', 'on_demand', 'manual'))
);

-- Indexes for common queries
CREATE INDEX routes_project_id_idx ON reactor_gateway.routes(project_id);
CREATE INDEX routes_project_ref_idx ON reactor_gateway.routes(project_ref);
CREATE INDEX routes_enabled_idx ON reactor_gateway.routes(enabled) WHERE enabled = true;

-- Custom domains table: tracks custom domain verification and certificate status
CREATE TABLE reactor_gateway.custom_domains (
    host TEXT PRIMARY KEY,
    project_id UUID NOT NULL,
    verification_token TEXT NOT NULL,
    verified_at TIMESTAMPTZ,
    cert_status TEXT NOT NULL DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    
    CONSTRAINT custom_domains_cert_status_check CHECK (cert_status IN ('pending', 'provisioning', 'active', 'failed'))
);

-- Indexes for custom domain lookups
CREATE INDEX custom_domains_project_id_idx ON reactor_gateway.custom_domains(project_id);
CREATE INDEX custom_domains_verified_idx ON reactor_gateway.custom_domains(verified_at) WHERE verified_at IS NOT NULL;

-- LISTEN/NOTIFY trigger for route changes
-- This allows the gateway server to react to route changes in real-time
CREATE OR REPLACE FUNCTION reactor_gateway.notify_route_change() 
RETURNS TRIGGER AS $$
BEGIN
    -- Notify with the host that changed
    IF TG_OP = 'DELETE' THEN
        PERFORM pg_notify('reactor_gateway_routes', OLD.host);
        RETURN OLD;
    ELSE
        PERFORM pg_notify('reactor_gateway_routes', NEW.host);
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Trigger on routes table
CREATE TRIGGER routes_notify 
    AFTER INSERT OR UPDATE OR DELETE
    ON reactor_gateway.routes
    FOR EACH ROW 
    EXECUTE FUNCTION reactor_gateway.notify_route_change();

-- Trigger for custom domain changes (useful for cert status updates)
CREATE OR REPLACE FUNCTION reactor_gateway.notify_domain_change()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        PERFORM pg_notify('reactor_gateway_domains', OLD.host);
        RETURN OLD;
    ELSE
        PERFORM pg_notify('reactor_gateway_domains', NEW.host);
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER domains_notify
    AFTER INSERT OR UPDATE OR DELETE
    ON reactor_gateway.custom_domains
    FOR EACH ROW
    EXECUTE FUNCTION reactor_gateway.notify_domain_change();

-- Seed the initial reactor.cloud route
-- This points to the existing reactor-cloud.internal:8000 backend
-- The project_id and project_ref should match the existing reactor.cloud project
-- Using a placeholder UUID that should be updated during deployment
INSERT INTO reactor_gateway.routes (
    host,
    project_id,
    project_ref,
    backend_kind,
    backend_target,
    tls_mode,
    enabled
) VALUES (
    'reactor.cloud',
    '00000000-0000-0000-0000-000000000001'::uuid,  -- placeholder, update during deployment
    'reactorcloud00000000',                         -- placeholder 20-char ref
    'dedicated',
    'reactor-cloud.internal:8000',
    'wildcard',
    true
) ON CONFLICT (host) DO NOTHING;

-- Also add wildcard route for *.reactor.cloud subdomains
INSERT INTO reactor_gateway.routes (
    host,
    project_id,
    project_ref,
    backend_kind,
    backend_target,
    tls_mode,
    enabled
) VALUES (
    '*.reactor.cloud',
    '00000000-0000-0000-0000-000000000001'::uuid,
    'reactorcloud00000000',
    'dedicated',
    'reactor-cloud.internal:8000',
    'wildcard',
    true
) ON CONFLICT (host) DO NOTHING;

COMMENT ON SCHEMA reactor_gateway IS 'Edge gateway routing configuration';
COMMENT ON TABLE reactor_gateway.routes IS 'Maps hostnames to backend targets for the edge gateway';
COMMENT ON TABLE reactor_gateway.custom_domains IS 'Tracks custom domain verification and certificate status';
COMMENT ON COLUMN reactor_gateway.routes.backend_kind IS 'dedicated = single project instance, shared = pooled workers';
COMMENT ON COLUMN reactor_gateway.routes.tls_mode IS 'wildcard = use *.reactor.cloud cert, on_demand = provision via ACME, manual = bring your own cert';
COMMENT ON COLUMN reactor_gateway.custom_domains.verification_token IS 'DNS TXT record value for domain ownership verification';
