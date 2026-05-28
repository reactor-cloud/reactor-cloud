-- Audit events and invocations

-- Audit events
create table _reactor_sites.audit_events (
  id                       uuid primary key,
  ts                       timestamptz not null default now(),
  actor_user_id            uuid,
  actor_apikey_id          uuid,
  org_id                   uuid,
  site_id                  uuid,
  deployment_id            uuid,
  domain_id                uuid,
  event_type               text not null,
  details                  jsonb not null default '{}',
  request_id               text not null
);

create index idx_audit_events_org on _reactor_sites.audit_events (org_id, ts desc);
create index idx_audit_events_site on _reactor_sites.audit_events (site_id, ts desc);

-- Invocations (sampled serve-plane requests)
create table _reactor_sites.invocations (
  id                       uuid primary key,
  site_id                  uuid not null,
  deployment_id            uuid not null,
  org_id                   uuid not null,
  request_id               text not null,
  method                   text not null,
  path                     text not null,
  host                     text not null,
  route_kind               text not null,
  status_code              integer not null,
  duration_ms              integer not null,
  cache_status             text,                     -- HIT, MISS, STALE, BYPASS
  bytes_out                bigint not null default 0,
  created_at               timestamptz not null default now()
);

create index idx_invocations_site on _reactor_sites.invocations (site_id, created_at desc);
create index idx_invocations_org on _reactor_sites.invocations (org_id, created_at desc);
