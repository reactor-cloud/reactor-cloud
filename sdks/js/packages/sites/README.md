# @reactor/sites

Sites client for Reactor. Manage static sites and deployments.

## Installation

```bash
npm install @reactor/sites @reactor/shared
```

Or use the unified client:

```bash
npm install @reactor/client
```

## Quick Start

```typescript
import { SitesClient } from '@reactor/sites';

const sites = new SitesClient(ctx);

// List sites
const { data: siteList } = await sites.list();

// Get site details
const { data: site } = await sites.get(siteId);

// List deployments
const { data: deployments } = await sites.deployments.list(siteId);

// Promote deployment to production
await sites.deployments.promote(siteId, deploymentId);

// Rollback to previous deployment
await sites.deployments.rollback(siteId);
```

## Domain Management

```typescript
// List domains
const { data: domains } = await sites.domains.list(siteId);

// Add custom domain
await sites.domains.create(siteId, {
  domain: 'app.example.com',
});

// Verify domain
const { data: verification } = await sites.domains.verify(siteId, domainId);

// Delete domain
await sites.domains.delete(siteId, domainId);
```

## Deployments

```typescript
// List deployments
const { data: deployments } = await sites.deployments.list(siteId, {
  limit: 10,
});

// Get deployment details
const { data: deployment } = await sites.deployments.get(siteId, deploymentId);

// Promote to production
await sites.deployments.promote(siteId, deploymentId);

// Rollback
await sites.deployments.rollback(siteId);
```

## Documentation

- [Sites Guide](https://reactor.cloud/docs/sites)
- [Custom Domains](https://reactor.cloud/docs/sites#domains)
- [API Reference](https://reactor.cloud/docs)

## License

MIT
