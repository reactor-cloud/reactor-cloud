-- Deployment routes and functions

-- Deployment routes (ordered route table)
create table _reactor_sites.deployment_routes (
  id                       uuid primary key,
  deployment_id            uuid not null references _reactor_sites.deployments(id) on delete cascade,
  pattern                  text not null,            -- path pattern, e.g. "/api/:path*", "/:slug"
  method_filter            text,                     -- null = any method; "GET,POST" = specific
  route_kind               text not null,            -- 'static' | 'function' | 'redirect' | 'prerender'
  target_ref               text not null,            -- storage key | function_id | redirect URL | prerender storage key
  cache_rules_json         jsonb not null default '{}',
  priority                 integer not null default 0,  -- higher = matched first
  created_at               timestamptz not null default now()
);

create index idx_deployment_routes_deployment on _reactor_sites.deployment_routes (deployment_id, priority desc);

-- Deployment functions (back-ref to reactor-functions)
create table _reactor_sites.deployment_functions (
  deployment_id            uuid not null references _reactor_sites.deployments(id) on delete cascade,
  function_id              uuid not null,            -- FK to _reactor_functions.functions conceptually
  role                     text not null,            -- 'ssr', 'api', 'isr-revalidate', etc.
  created_at               timestamptz not null default now(),
  primary key (deployment_id, function_id)
);

create index idx_deployment_functions_fn on _reactor_sites.deployment_functions (function_id);
