-- ISR cache (Postgres backstop; primary cache in reactor-cache)

create table _reactor_sites.isr_cache (
  site_id                  uuid not null references _reactor_sites.sites(id) on delete cascade,
  path                     text not null,
  deployment_id            uuid not null references _reactor_sites.deployments(id) on delete cascade,
  body_storage_key         text not null,
  content_type             text,
  etag                     text,
  tags                     jsonb not null default '[]',
  revalidate_after_secs    bigint,
  last_revalidated_at      timestamptz not null default now(),
  created_at               timestamptz not null default now(),
  primary key (site_id, path)
);

create index idx_isr_cache_tags on _reactor_sites.isr_cache using gin (tags);
create index idx_isr_cache_revalidate on _reactor_sites.isr_cache (last_revalidated_at) where revalidate_after_secs is not null;
