# Reactor CLI — Agent Guide

This document provides guidance for AI agents and automated tools using the Reactor CLI.

## Output Format

**Always use `--output json` for machine consumption.**

The CLI auto-detects non-TTY environments and outputs JSON by default, but explicitly setting the flag ensures consistent behavior:

```bash
reactor --output json functions list
```

## Response Structure

### Success

```json
{
  "ok": true,
  "data": { ... }
}
```

### Error

```json
{
  "ok": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable description",
    "hint": "Optional actionable suggestion"
  }
}
```

## Exit Codes

Exit codes are stable and documented for scripting:

| Code | Constant | Meaning |
|------|----------|---------|
| 0 | `Ok` | Success |
| 1 | `User` | Invalid arguments or user error |
| 2 | `Config` | Configuration or context error |
| 3 | `Auth` | Authentication failure |
| 4 | `Validation` | Schema or validation error |
| 5 | `Server` | Server-side error |
| 6 | `Network` | Connection or timeout error |

## Non-Interactive Mode

The CLI never prompts for input in non-TTY environments. Destructive operations require either:

1. The `--yes` flag
2. The `REACTOR_ASSUME_YES=1` environment variable

Without these, destructive commands fail with exit code 1 and error code `REQUIRES_CONFIRMATION`.

## Error Codes

Common error codes you may encounter:

| Code | Description |
|------|-------------|
| `USER_ERROR` | General user error |
| `MISSING_ARGUMENT` | Required argument not provided |
| `INVALID_ARGUMENT` | Argument has invalid value |
| `REQUIRES_CONFIRMATION` | Needs `--yes` in non-TTY |
| `CONFIG_ERROR` | Configuration file error |
| `CONTEXT_NOT_FOUND` | Named context doesn't exist |
| `MANIFEST_NOT_FOUND` | No reactor.toml found |
| `AUTH_REQUIRED` | Authentication token needed |
| `AUTH_FAILED` | Token invalid or expired |
| `PERMISSION_DENIED` | Insufficient permissions |
| `VALIDATION_ERROR` | Schema or format error |
| `SERVER_ERROR` | Server returned 5xx |
| `DEPLOYMENT_FAILED` | Deployment did not succeed |
| `PARTIAL_DEPLOYMENT` | Some capabilities failed |
| `NETWORK_ERROR` | Connection problem |
| `CONNECTION_REFUSED` | Server not reachable |
| `TIMEOUT` | Request timed out |

## Idempotent Operations

These operations are safe to retry:

- `reactor context list`
- `reactor context show`
- `reactor functions list`
- `reactor functions show`
- `reactor sites list`
- `reactor jobs list`
- `reactor doctor`
- `reactor version`
- `reactor whoami`
- `reactor data inspect`

## Common Workflows

### Deploy a project

```bash
# 1. Ensure we're in a project directory
reactor --output json project show || exit 2

# 2. Build the bundle
reactor --output json build || exit 4

# 3. Deploy
reactor --output json deploy --yes || exit 5
```

### Check server health

```bash
reactor --output json doctor | jq -e '.data.capabilities | to_entries | all(.value.status == "healthy")'
```

### Invoke a function

```bash
reactor --output json functions invoke my-function --data '{"key": "value"}' | jq '.data.response'
```

### Query database

```bash
reactor --output json data query --sql "SELECT id, name FROM users LIMIT 10" | jq '.data.rows'
```

### Manage environment variables

```bash
# List
reactor --output json functions env list my-function | jq '.data'

# Set
reactor --output json functions env set my-function API_KEY "secret123"

# Unset
reactor --output json functions env unset my-function OLD_KEY
```

## Authentication

### Token from environment

```bash
export REACTOR_TOKEN="rk_live_..."
reactor --output json whoami
```

### Token from flag

```bash
reactor --token "rk_live_..." --output json whoami
```

### Per-context token environment variables

Configure in `~/.reactor/config.toml`:

```toml
[contexts.production.auth]
kind = "token-env"
env = "REACTOR_PROD_TOKEN"
```

Then:

```bash
export REACTOR_PROD_TOKEN="rk_live_..."
reactor --context production --output json whoami
```

## Working with Multiple Contexts

```bash
# List available contexts
reactor --output json context list | jq '.data[].name'

# Use a specific context for one command
reactor --context production --output json functions list

# Set default context
reactor context use production
```

## Parsing Complex Responses

### Functions with deployments

```bash
reactor --output json functions show my-function | jq '{
  name: .data.name,
  runtime: .data.runtime,
  deployment: .data.current_deployment_id
}'
```

### Job runs

```bash
reactor --output json jobs runs my-job --limit 5 | jq '.data | map({id, status, started_at})'
```

### Table columns

```bash
reactor --output json data inspect users | jq '.data.columns | map({name, type: .data_type})'
```

## Best Practices

1. **Always use `--output json`** for parsing
2. **Check exit codes** before parsing output
3. **Use `--yes`** for automated deployments
4. **Set `REACTOR_ASSUME_YES=1`** in CI environments
5. **Handle partial deployments** (exit code 5 with `PARTIAL_DEPLOYMENT`)
6. **Retry on network errors** (exit code 6)
7. **Don't retry on auth errors** (exit code 3)

## Environment Variables

| Variable | Description |
|----------|-------------|
| `REACTOR_TOKEN` | Authentication token |
| `REACTOR_CONTEXT` | Default context name |
| `REACTOR_OUTPUT` | Output format (`json` or `human`) |
| `REACTOR_ASSUME_YES` | Skip confirmations when set to `1` |

## CI/CD Integration

```yaml
# GitHub Actions example
- name: Deploy to Reactor
  env:
    REACTOR_TOKEN: ${{ secrets.REACTOR_TOKEN }}
    REACTOR_ASSUME_YES: "1"
    REACTOR_OUTPUT: "json"
  run: |
    reactor build
    reactor deploy
```

## Version Compatibility

Check CLI and server version compatibility:

```bash
reactor --output json version | jq '{
  cli: .data.cli_version,
  server: .data.server_version,
  compatible: .data.compatible
}'
```
