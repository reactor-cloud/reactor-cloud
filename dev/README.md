# Reactor Local Development

```bash
docker compose up -d postgres   # start Postgres on :5434
make migrate                    # apply all schemas
make smoke                      # run full e2e test
```

## Quickstart

```bash
# One-time setup
cp .env.example .env

# Start Postgres and run tests
make db-up
make smoke
```

## Configuration

Default Postgres port is **5434** to avoid conflicts with other services.
Edit `.env` to customize:

```bash
POSTGRES_PORT=5434      # host port mapping
POSTGRES_USER=reactor
POSTGRES_PASSWORD=reactor
POSTGRES_DB=reactor
```

## Available Targets

See [../Makefile](../Makefile) for all available targets:
- `db-up` / `db-down` / `db-reset` — Postgres lifecycle
- `migrate` — Run all capability migrations
- `doctor` — Health checks
- `server` — Start reactor-server
- `bundle` — Build dev fixture bundle
- `smoke` — Full e2e smoke test
- `clean` — Remove dev artifacts

## Known Issues

**Migration conflicts**: The current migration system uses a shared
`_sqlx_migrations` table for all capabilities. If you encounter migration
errors after running other Reactor instances on the same database, run
`make db-reset` to start fresh.
