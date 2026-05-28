# Reactor Connect YAML Manifests

This directory contains Airbyte Low-Code CDK compatible YAML manifests for various connectors.

## License

All manifests in this directory are licensed under the MIT License, following the Airbyte Low-Code CDK patterns.

**Important**: These manifests define connector configurations. Users must provide their own:
- OAuth client IDs and secrets
- API keys and tokens
- Account credentials

Reactor does not provide any API keys or credentials for third-party services.

## Supported Connectors

| Connector | Auth Type | Streams | Status |
|-----------|-----------|---------|--------|
| HubSpot | OAuth2 | contacts, companies, deals | Ready |
| Pipedrive | API Key | deals, persons, organizations, activities | Ready |
| Mailchimp | API Key (Basic) | lists, campaigns, automations | Ready |
| Intercom | Bearer Token | contacts, conversations, companies | Ready |
| Zendesk | API Token (Basic) | tickets, users, organizations | Ready |

## Adding New Manifests

1. Create a new YAML file following the Airbyte Low-Code CDK spec
2. Include a license header: `# License: MIT (Airbyte Low-Code CDK compatible)`
3. Run the license audit script: `./scripts/audit_manifest_licenses.sh`
4. Add the connector to the table above

## Authentication Patterns

### OAuth2
```yaml
definitions:
  oauth_authenticator:
    type: OAuthAuthenticator
    client_id: "{{ config['client_id'] }}"
    client_secret: "{{ config['client_secret'] }}"
    token_refresh_endpoint: "https://..."
    refresh_token: "{{ config['refresh_token'] }}"
```

### API Key
```yaml
definitions:
  api_key_authenticator:
    type: ApiKeyAuthenticator
    header: api_token
    api_token: "{{ config['api_token'] }}"
```

### Bearer Token
```yaml
definitions:
  bearer_authenticator:
    type: BearerAuthenticator
    api_token: "{{ config['access_token'] }}"
```

### Basic Auth
```yaml
definitions:
  basic_authenticator:
    type: BasicHttpAuthenticator
    username: "{{ config['username'] }}"
    password: "{{ config['password'] }}"
```

## Pagination Patterns

### Cursor-based
```yaml
pagination_strategy:
  type: CursorPagination
  cursor_value: "{{ response.next_cursor }}"
  stop_condition: "{{ response.next_cursor is not defined }}"
```

### Offset-based
```yaml
pagination_strategy:
  type: OffsetIncrement
  page_size: 100
```

### Page-based
```yaml
pagination_strategy:
  type: PageIncrement
  page_size: 100
```
