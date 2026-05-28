# Reactor.cloud

**The AI-first backend platform.** Identity, data, storage, functions, jobs,
sites, and an LLM gateway — behind a single CLI, a single SDK, and a single
project file. The same project runs on your laptop, on one VPS, on your own
cloud, or on Reactor.cloud — without code changes.

Built in Rust. Designed for agents.

## The Eight Capabilities

| # | Capability | What it does |
|---|------------|--------------|
| 1 | **Identity** | Auth, users, orgs, roles, MFA, OAuth, JWT issuance |
| 2 | **Data** | Typed tables, queries, mutations, RLS, realtime subscriptions |
| 3 | **Storage** | Blob upload/download, signed URLs, lifecycle |
| 4 | **Functions** | One-shot serverless functions (HTTP handlers) |
| 5 | **Jobs** | Durable, retryable, scheduled background work |
| 6 | **Sites** | Static hosting and full app hosting (Next.js, etc.) |
| 7 | **Gateway** | LLM routing, metering, observability |
| 8 | **Connect** | Third-party API connectors, data sync, webhooks |

Each capability is a stable HTTP surface, a typed SDK, a CLI verb, and a Rust
trait with swappable adapters. Pick a deployment grade; the rest is invisible.

## Quick Start

```bash
# Install the CLI
brew install reactor-cloud/tap/reactor
# or: cargo install reactor-cli

# Create a new project
reactor init my-app
cd my-app

# Start local development
reactor dev

# Deploy to Reactor.cloud
reactor deploy
```

## Documentation

- [Quickstart Guide](https://docs.reactor.cloud/quickstart)
- [CLI Reference](https://docs.reactor.cloud/cli)
- [SDK Reference](https://docs.reactor.cloud/sdk)
- [Self-Hosting Guide](https://docs.reactor.cloud/self-hosting)

## Project Structure

```
reactor-cloud/
├── crates/           # Rust workspace
│   ├── reactor-core/        # Shared types, traits, config
│   ├── reactor-auth/        # Identity capability
│   ├── reactor-data/        # Data capability
│   ├── reactor-storage/     # Storage capability
│   ├── reactor-functions/   # Functions capability
│   ├── reactor-jobs/        # Jobs capability
│   ├── reactor-sites/       # Sites capability
│   ├── reactor-gateway/     # LLM Gateway
│   ├── reactor-connect/     # Connect capability
│   ├── reactor-server/      # Main server binary
│   └── reactor-cli/         # CLI binary
├── sdks/
│   ├── js/                  # TypeScript/JavaScript SDK
│   └── swift/               # Swift SDK
├── studio/                  # Visual development environment
├── examples/                # Example projects
└── docs/                    # Design documents
```

## Licensing

This project uses a dual-license model:

- **Apache 2.0** for the framework, all capability crates, SDKs, and Studio
- **BUSL 1.1** for `reactor-cloud-api` and `reactor-ops` (multi-tenant control plane)

See [LICENSING.md](LICENSING.md) for details.

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines.

All contributions require a Developer Certificate of Origin (DCO) sign-off.

## Community

- [GitHub Discussions](https://github.com/reactor-cloud/reactor-cloud/discussions)
- [Discord](https://reactor.cloud/discord)

## Built by [AtomicoLabs](https://atomicolabs.com)
