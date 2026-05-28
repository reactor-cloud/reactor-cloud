-- Custom domains

create table _reactor_sites.domains (
  id                       uuid primary key,
  site_id                  uuid not null references _reactor_sites.sites(id) on delete cascade,
  host                     text not null unique,     -- e.g. "app.example.com"
  status                   text not null default 'pending',  -- pending, verified, active, failed
  verification_token       text not null,            -- for DNS TXT or HTTP challenge
  verification_method      text not null default 'dns',  -- 'dns' | 'http'
  tls_cert_ref             text,                     -- reference to cert in storage (G2) or CDN (G3)
  tls_expires_at           timestamptz,
  verified_at              timestamptz,
  created_at               timestamptz not null default now()
);

create index idx_domains_site_id on _reactor_sites.domains (site_id);
create index idx_domains_tls_expires on _reactor_sites.domains (tls_expires_at) where status = 'active';
