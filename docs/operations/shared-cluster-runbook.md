# Shared Cluster Operations Runbook

This runbook covers operational procedures for the Phase 4 shared cluster deployment, including database management, NATS operations, connection pooling, and quota enforcement.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Database Operations](#database-operations)
3. [NATS Operations](#nats-operations)
4. [Supavisor/Connection Pooling](#supavisorconnection-pooling)
5. [Quota Management](#quota-management)
6. [Incident Response](#incident-response)
7. [Monitoring & Alerts](#monitoring--alerts)

---

## Architecture Overview

```
                    ┌─────────────────────────────┐
                    │     Caddy Edge Gateway      │
                    │   (Rate limiting, TLS)      │
                    └──────────────┬──────────────┘
                                   │
              ┌────────────────────┼────────────────────┐
              │                    │                    │
              ▼                    ▼                    ▼
     ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐
     │  reactor-shared │  │  reactor-shared │  │  reactor-shared │
     │  (stateless)    │  │  (stateless)    │  │  (stateless)    │
     └────────┬────────┘  └────────┬────────┘  └────────┬────────┘
              │                    │                    │
    ┌─────────┴─────────┬──────────┴──────────┬────────┴─────────┐
    │                   │                     │                  │
    ▼                   ▼                     ▼                  ▼
┌───────────┐    ┌────────────┐       ┌─────────────┐    ┌──────────┐
│ Supavisor │    │    NATS    │       │  Shared PG  │    │ OpenBao  │
│ (pooler)  │    │  Cluster   │       │  (tenant_*) │    │ Cluster  │
└───────────┘    └────────────┘       └─────────────┘    └──────────┘
```

### Key Components

- **reactor-shared**: Stateless application servers (3+ instances)
- **Supavisor**: PostgreSQL connection pooler (transaction mode)
- **NATS Cluster**: Realtime messaging (3-node JetStream)
- **Shared PostgreSQL**: Single Postgres instance hosting tenant_* databases
- **OpenBao**: Secrets management (3-node HA)

---

## Database Operations

### Tenant Database Naming

Each tenant gets their own database: `tenant_<project_ref>`

```sql
-- List all tenant databases
SELECT datname FROM pg_database WHERE datname LIKE 'tenant_%';

-- Count tenant databases
SELECT COUNT(*) as tenant_count FROM pg_database WHERE datname LIKE 'tenant_%';
```

### Backup Strategy

#### Full Backup (pg_dumpall)

Use for disaster recovery and cluster migration:

```bash
# Full cluster backup (all tenant databases)
pg_dumpall -h shared-pg.internal -U admin > /backup/full_$(date +%Y%m%d).sql

# Compress
gzip /backup/full_$(date +%Y%m%d).sql
```

#### Per-Tenant Backup (pg_dump)

Use for tenant migration or selective restore:

```bash
# Backup single tenant
pg_dump -h shared-pg.internal -U admin -d tenant_abc123xyz > /backup/tenant_abc123xyz.sql

# Backup all tenants (parallel)
for db in $(psql -h shared-pg.internal -U admin -t -c "SELECT datname FROM pg_database WHERE datname LIKE 'tenant_%'"); do
  pg_dump -h shared-pg.internal -U admin -d "$db" | gzip > "/backup/${db}_$(date +%Y%m%d).sql.gz" &
done
wait
```

#### Point-in-Time Recovery (PITR)

For Postgres instances with WAL archiving:

```bash
# Archive WAL to S3
archive_command = 'aws s3 cp %p s3://reactor-pg-wal/%f'

# Restore to specific time
recovery_target_time = '2026-05-23 12:00:00 UTC'
restore_command = 'aws s3 cp s3://reactor-pg-wal/%f %p'
```

### Tenant Cleanup

Remove inactive tenant databases:

```bash
# Check last activity for a tenant
SELECT last_activity FROM pg_stat_database WHERE datname = 'tenant_abc123xyz';

# Drop tenant database (DANGEROUS - verify first!)
DROP DATABASE IF EXISTS tenant_abc123xyz;
DROP ROLE IF EXISTS tenant_abc123xyz;

# Remove tenant route
DELETE FROM routes WHERE project_ref = 'abc123xyz';
```

### Schema Migrations

Migrations run per-tenant database. For shared cluster:

```bash
# Apply migrations to all tenant databases
for db in $(psql -h shared-pg.internal -U admin -t -c "SELECT datname FROM pg_database WHERE datname LIKE 'tenant_%'"); do
  echo "Migrating $db..."
  DATABASE_URL="postgres://admin:***@shared-pg.internal/$db" reactor-server migrate
done
```

### Database Connection Limits

Each tenant role has connection limits enforced by PostgreSQL:

```sql
-- Check connection limits per role
SELECT rolname, rolconnlimit FROM pg_roles WHERE rolname LIKE 'tenant_%';

-- Adjust connection limit
ALTER ROLE tenant_abc123xyz CONNECTION LIMIT 10;
```

---

## NATS Operations

### Cluster Health

```bash
# Check cluster health
nats server check connection --server nats://nats-0.internal:4222

# List all servers
nats server list

# Check JetStream status
nats server report jetstream
```

### Stream Management

Reactor uses JetStream for durable realtime:

```bash
# List streams
nats stream list

# Check stream info
nats stream info REACTOR_REALTIME

# Purge old messages (keep last 24h)
nats stream purge REACTOR_REALTIME --keep 86400s

# Consumer lag
nats consumer report REACTOR_REALTIME
```

### Topic Inspection

Monitor tenant-scoped topics:

```bash
# Subscribe to a tenant's data changes (debug)
nats sub "reactor.abc123xyz.data.>"

# Check message rate per tenant
nats sub --count 100 "reactor.*.data.*.insert" | awk -F'.' '{print $2}' | sort | uniq -c
```

### Partition Recovery

If NATS cluster loses quorum:

1. **Check node status**:
   ```bash
   nats server list
   ```

2. **Force leader election** (if stuck):
   ```bash
   nats server request jetstream.cluster.step_down
   ```

3. **Verify message delivery**:
   ```bash
   # Check for unacked messages
   nats consumer info REACTOR_REALTIME reactor-shared-consumer
   ```

### NATS Maintenance

```bash
# Graceful restart (one node at a time)
flyctl machines restart nats-0 --wait-timeout 60s
sleep 30  # Wait for re-election
flyctl machines restart nats-1 --wait-timeout 60s
# etc.

# Check for slow consumers
nats consumer pending REACTOR_REALTIME reactor-shared-consumer
```

---

## Supavisor/Connection Pooling

### Health Check

```bash
# Check Supavisor status
curl -s http://supavisor.internal:4000/health

# Connection stats
curl -s http://supavisor.internal:4000/metrics | grep supavisor_db_
```

### Connection Pool Metrics

Key metrics to monitor:

- `supavisor_db_pool_size` - Current pool size
- `supavisor_db_pool_available` - Available connections
- `supavisor_db_pool_waiting` - Queries waiting for connection
- `supavisor_db_pool_timeout` - Connection timeout count

### Troubleshooting Pool Exhaustion

1. **Identify the tenant**:
   ```bash
   # Check which tenants have most connections
   psql -c "SELECT datname, count(*) FROM pg_stat_activity WHERE datname LIKE 'tenant_%' GROUP BY datname ORDER BY count DESC LIMIT 10"
   ```

2. **Kill long-running queries**:
   ```sql
   -- Find long queries (>60s)
   SELECT pid, datname, now() - query_start as duration, query
   FROM pg_stat_activity
   WHERE datname LIKE 'tenant_%'
     AND state = 'active'
     AND now() - query_start > interval '60 seconds';

   -- Kill specific query
   SELECT pg_terminate_backend(pid);
   ```

3. **Adjust per-tenant pool size**:
   Update `Reactor.toml`:
   ```toml
   [cloud.shared_pool]
   per_tenant_pool_size = 3  # Reduce from 5
   ```

### Supavisor Configuration

```toml
# supavisor.toml
[pool]
mode = "transaction"
default_pool_size = 5
min_pool_size = 1
reserve_pool_timeout = 5
statement_timeout = 30

[client]
max_connections = 10000
idle_timeout = 60
```

---

## Quota Management

### Current Quota Limits (Free Tier)

| Resource | Limit |
|----------|-------|
| Requests/minute | 1,000 |
| Concurrent functions | 10 |
| Database connections | 5 |
| Storage | 1 GB |
| Bandwidth/month | 5 GB |

### Checking Quota Status

```bash
# Admin API endpoint
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
  http://localhost:8000/_admin/quotas/tenant_abc123xyz
```

### Responding to Quota Breaches

1. **Identify the breach type**:
   - Check Prometheus: `reactor_quota_exceeded_total{tenant="abc123xyz",resource="requests"}`

2. **For request rate breaches**:
   - Verify legitimate traffic (not abuse)
   - Check for runaway client
   - Consider temporary rate increase or upgrade path

3. **For storage breaches**:
   ```sql
   -- Check storage usage
   SELECT pg_size_pretty(pg_database_size('tenant_abc123xyz'));
   ```

4. **Manual quota override** (temporary):
   ```bash
   # Via admin API
   curl -X POST -H "Authorization: Bearer $ADMIN_TOKEN" \
     -d '{"requests_per_minute": 2000}' \
     http://localhost:8000/_admin/quotas/tenant_abc123xyz/override
   ```

### Monthly Bandwidth Reset

Bandwidth counters reset on the 1st of each month:

```bash
# Manual reset (if needed)
curl -X POST -H "Authorization: Bearer $ADMIN_TOKEN" \
  http://localhost:8000/_admin/quotas/reset-bandwidth
```

---

## Incident Response

### High Latency (P50 > 100ms)

1. **Check database load**:
   ```bash
   psql -c "SELECT count(*) FROM pg_stat_activity WHERE state = 'active'"
   ```

2. **Check NATS lag**:
   ```bash
   nats consumer report REACTOR_REALTIME
   ```

3. **Check tenant cache**:
   ```bash
   curl http://localhost:8000/_admin/cache/stats
   ```

4. **Mitigation**:
   - Scale up reactor-shared instances
   - Increase Supavisor pool size
   - Evict idle tenants from cache

### Database Connection Exhaustion

1. **Check connection count**:
   ```sql
   SELECT count(*) FROM pg_stat_activity;
   SELECT max_connections FROM pg_settings WHERE name = 'max_connections';
   ```

2. **Identify problematic tenants**:
   ```sql
   SELECT datname, count(*), state
   FROM pg_stat_activity
   WHERE datname LIKE 'tenant_%'
   GROUP BY datname, state
   ORDER BY count DESC;
   ```

3. **Emergency mitigation**:
   ```sql
   -- Kill all connections for a tenant
   SELECT pg_terminate_backend(pid)
   FROM pg_stat_activity
   WHERE datname = 'tenant_abc123xyz';
   ```

### NATS Cluster Failure

1. **Check cluster state**:
   ```bash
   nats server report
   ```

2. **If single node down**:
   - Cluster should self-heal
   - Monitor consumer lag

3. **If quorum lost**:
   - Restore from remaining nodes
   - May need manual stream recovery:
     ```bash
     nats stream restore REACTOR_REALTIME < stream_backup.json
     ```

### Tenant Data Isolation Breach (Critical)

If tenant data isolation is suspected:

1. **IMMEDIATELY** disable the affected tenant:
   ```sql
   UPDATE routes SET enabled = false WHERE project_ref = 'abc123xyz';
   ```

2. **Audit logs**:
   ```bash
   grep "abc123xyz" /var/log/reactor/*.log | head -100
   ```

3. **Check for SQL injection**:
   ```sql
   SELECT query FROM pg_stat_statements WHERE query LIKE '%abc123xyz%';
   ```

4. **Escalate** to security team if breach confirmed.

---

## Monitoring & Alerts

### Key Metrics

| Metric | Warning | Critical |
|--------|---------|----------|
| `reactor_tenant_cache_active` | >4000 | >4500 |
| `reactor_quota_exceeded_total` rate | >10/min | >50/min |
| `supavisor_db_pool_waiting` | >10 | >50 |
| `nats_consumer_pending` | >1000 | >10000 |
| `reactor_request_latency_p99` | >200ms | >500ms |

### Alert Rules (Prometheus)

```yaml
groups:
- name: reactor-shared-cluster
  rules:
  - alert: TenantCacheNearCapacity
    expr: reactor_tenant_cache_active > 4500
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: "Tenant cache near capacity"
      
  - alert: HighQuotaExceeded
    expr: rate(reactor_quota_exceeded_total[5m]) > 0.5
    for: 10m
    labels:
      severity: warning
    annotations:
      summary: "High quota exceeded rate"
      
  - alert: NATSConsumerLag
    expr: nats_consumer_pending > 10000
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: "NATS consumer has high pending messages"
```

### Dashboards

Grafana dashboards for shared cluster:

- **Tenant Overview**: Active tenants, cache hit rate, evictions
- **Quota Dashboard**: Quota usage by tier, breach counts
- **Database Health**: Connections, query latency, pool utilization
- **NATS Health**: Message rates, consumer lag, cluster state

---

## Maintenance Windows

### Weekly Maintenance

- Sunday 04:00 UTC: PostgreSQL VACUUM ANALYZE
- Sunday 05:00 UTC: Clear expired quota counters

### Monthly Maintenance

- 1st of month: Reset bandwidth counters
- 1st of month: Rotate NATS credentials
- 1st of month: Review tenant database sizes

### Quarterly Maintenance

- PostgreSQL minor version upgrades
- NATS version upgrades
- Supavisor version upgrades
- Certificate rotation

---

## Emergency Contacts

| Role | Contact |
|------|---------|
| On-call Engineer | PagerDuty: reactor-oncall |
| Database Admin | #db-team Slack |
| Security | security@reactor.cloud |
| Cloud Infrastructure | #infra Slack |
