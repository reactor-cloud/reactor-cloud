-- Sites metadata schema

create schema if not exists _reactor_sites;

-- Enable citext extension for case-insensitive site names
create extension if not exists citext;

-- Sites table
create table _reactor_sites.sites (
  id                       uuid primary key,
  org_id                   uuid not null,
  name                     citext not null,
  framework                text not null,            -- 'static' | 'hono' | 'nextjs' | ...
  current_deployment_id    uuid,                     -- FK; null until first promote
  created_at               timestamptz not null default now(),
  updated_at               timestamptz not null default now(),
  unique (org_id, name)
);

create index idx_sites_org_id on _reactor_sites.sites (org_id);

-- Deployments table
create table _reactor_sites.deployments (
  id                       uuid primary key,
  site_id                  uuid not null references _reactor_sites.sites(id) on delete cascade,
  version                  bigint not null,          -- monotonic per site
  manifest_json            jsonb not null,
  status                   text not null default 'pending',  -- pending, ready, failed, destroyed
  status_detail            text,
  static_asset_count       integer not null default 0,
  static_asset_bytes       bigint not null default 0,
  deployed_at              timestamptz not null default now(),
  deployed_by_user_id      uuid,
  unique (site_id, version)
);

create index idx_deployments_site_id on _reactor_sites.deployments (site_id, deployed_at desc);
create index idx_deployments_status on _reactor_sites.deployments (status) where status in ('pending', 'failed');

-- Add FK from sites to deployments (deferred to avoid circular dependency)
alter table _reactor_sites.sites
  add constraint fk_current_deployment
  foreign key (current_deployment_id) references _reactor_sites.deployments(id) on delete set null;
