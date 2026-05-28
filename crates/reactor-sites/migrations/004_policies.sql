-- Per-site policies

create table _reactor_sites.policies (
  id                       uuid primary key,
  site_id                  uuid not null references _reactor_sites.sites(id) on delete cascade,
  name                     text not null,
  using_expr_json          jsonb,                    -- PolicyExpr; evaluated for serve-plane requests
  raw_text                 text not null,
  sha256                   bytea not null,
  created_at               timestamptz not null default now(),
  unique (site_id, name)
);

create index idx_policies_site_id on _reactor_sites.policies (site_id);
