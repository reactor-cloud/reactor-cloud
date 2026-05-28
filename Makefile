# Reactor Makefile
# Local development convenience targets

.PHONY: db-up db-down db-reset migrate doctor server bundle smoke clean help

# Database
db-up:
	docker compose up -d postgres

db-down:
	docker compose down

db-reset:
	docker compose down -v
	docker compose up -d postgres
	@echo "Waiting for Postgres to be ready..."
	@sleep 3

# Server operations (run from dev/ so Reactor.toml is found)
migrate:
	cd dev && cargo run -q -p reactor-server -- migrate

doctor:
	cd dev && cargo run -q -p reactor-server -- doctor

server:
	cd dev && cargo run -q -p reactor-server

# Bundle building
bundle:
	cargo run -q -p reactor-dev-bundle-builder -- --src dev/fixtures/bundle-src --out dev/bundle.tar.zst

# End-to-end smoke test
smoke: db-up bundle
	dev/scripts/smoke.sh

# Cleanup
clean:
	rm -rf dev/.reactor dev/bundle.tar.zst

# JS SDK (sdks/js/)
sdk-install:
	cd sdks/js && pnpm install

sdk-build:
	cd sdks/js && pnpm build

sdk-test:
	cd sdks/js && pnpm test

sdk-openapi:
	cd sdks/js && pnpm openapi:generate

sdk-openapi-check:
	cd sdks/js && pnpm openapi:check

# Help
help:
	@echo "Reactor Makefile targets:"
	@echo "  db-up          - Start Postgres container"
	@echo "  db-down        - Stop containers"
	@echo "  db-reset       - Reset database (destroy volume and recreate)"
	@echo "  migrate        - Run database migrations"
	@echo "  doctor         - Run server doctor checks"
	@echo "  server         - Start reactor-server"
	@echo "  bundle         - Build dev fixture bundle"
	@echo "  smoke          - Run full e2e smoke test"
	@echo "  clean          - Remove dev artifacts"
	@echo ""
	@echo "JS SDK targets:"
	@echo "  sdk-install    - Install JS SDK dependencies"
	@echo "  sdk-build      - Build all JS SDK packages"
	@echo "  sdk-test       - Run JS SDK tests"
	@echo "  sdk-openapi    - Generate TS types from OpenAPI"
	@echo "  sdk-openapi-check - Verify types are up to date"
