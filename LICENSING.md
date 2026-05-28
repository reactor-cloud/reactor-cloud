# Licensing

Reactor.cloud is primarily licensed under the Apache License 2.0. However, certain components are licensed under the Business Source License 1.1 (BUSL 1.1).

## License Summary

| Component | License | Notes |
|-----------|---------|-------|
| All crates except below | Apache 2.0 | Free for any use |
| `crates/reactor-cloud-api` | BUSL 1.1 | Multi-tenant control plane |
| `crates/reactor-ops` | BUSL 1.1 | Fleet orchestration |

## Apache 2.0 Components

The following crates are licensed under Apache 2.0 and may be used freely for any purpose:

- `reactor-core` - Shared types, errors, config
- `reactor-policy` - Policy engine
- `reactor-auth` / `reactor-auth-server` - Identity and authentication
- `reactor-data` / `reactor-data-server` - Data layer (PostgREST-style)
- `reactor-storage` / `reactor-storage-server` - Blob storage
- `reactor-functions` / `reactor-functions-server` - Serverless functions
- `reactor-jobs` / `reactor-jobs-server` - Durable background jobs
- `reactor-connect` / `reactor-connect-server` - Third-party connectors
- `reactor-sites` / `reactor-sites-server` - Static and app hosting
- `reactor-analytics` / `reactor-analytics-server` - Analytics
- `reactor-cache` - Caching layer
- `reactor-gateway` / `reactor-gateway-server` - LLM gateway
- `reactor-vault` - Secrets management
- `reactor-deploy-bundle` - Deployment bundling
- `reactor-server` - Main server binary
- `reactor-cli` - Command-line interface
- `reactor-client` - Client library
- All SDKs (`sdks/js`, `sdks/swift`)
- Studio (`studio/`)

## BUSL 1.1 Components

The following crates are licensed under BUSL 1.1:

- `reactor-cloud-api` - Multi-tenant control plane for Reactor.cloud hosted service
- `reactor-ops` - Fleet orchestration and management

### BUSL 1.1 Terms

- **Change Date**: May 28, 2029
- **Change License**: Apache License 2.0
- **Additional Use Grant**: You may make production use of the Licensed Work, provided that you do not offer the Licensed Work to third parties as a hosted or managed service that provides users with substantially the same functionality as Reactor.cloud.

After the Change Date, these components will automatically become available under Apache 2.0.

## Questions

For licensing questions or commercial licensing arrangements, contact licensing@reactor.cloud.
