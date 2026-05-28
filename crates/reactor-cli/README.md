# Reactor CLI

Command-line interface for managing Reactor servers and projects.

## Installation

```bash
cargo install reactor-cli
```

Or build from source:

```bash
cargo build --release --package reactor-cli
```

## Quick Start

### Initialize a new project

```bash
reactor init my-app
cd my-app
```

This creates:
- `reactor.toml` — project manifest
- `.reactorignore` — files to exclude from deployments
- `functions/` — sample edge function
- `migrations/` — sample database migration

### Set up a context

A **context** is a named connection to a Reactor server:

```bash
# Add a remote server
reactor context add production --endpoint https://api.example.com --org acme

# Add a local development server
reactor context add local --endpoint http://localhost:8080

# Set the default context
reactor context use local
```

### Authenticate

```bash
reactor login
# Follow prompts to enter your API token
```

### Start local development

```bash
reactor dev
# Starts a local Reactor server with ephemeral Postgres
```

### Deploy

```bash
# Build the deployment bundle
reactor build

# Deploy to the configured server
reactor deploy
```

## Commands

### Project Management

| Command | Description |
|---------|-------------|
| `reactor init <name>` | Initialize a new project |
| `reactor project show` | Show project manifest and paths |
| `reactor build` | Build a deployment bundle |
| `reactor deploy` | Deploy to a Reactor server |

### Context Management

| Command | Description |
|---------|-------------|
| `reactor context list` | List all contexts |
| `reactor context add <name>` | Add a new context |
| `reactor context use <name>` | Set the default context |
| `reactor context show [name]` | Show context details |
| `reactor context remove <name>` | Remove a context |

### Authentication

| Command | Description |
|---------|-------------|
| `reactor login` | Authenticate with a server |
| `reactor logout` | Remove stored credentials |
| `reactor whoami` | Show current user info |

### Functions

| Command | Description |
|---------|-------------|
| `reactor functions list` | List functions |
| `reactor functions show <name>` | Show function details |
| `reactor functions invoke <name>` | Invoke a function |
| `reactor functions logs <name>` | View function logs |
| `reactor functions env list <name>` | List environment variables |
| `reactor functions env set <name> <key> <value>` | Set an environment variable |

### Sites

| Command | Description |
|---------|-------------|
| `reactor sites list` | List sites |
| `reactor sites show <name>` | Show site details |
| `reactor sites rollback <name>` | Rollback to previous deployment |
| `reactor sites domains list <name>` | List custom domains |
| `reactor sites revalidate <name> --path <path>` | Revalidate ISR cache |

### Jobs

| Command | Description |
|---------|-------------|
| `reactor jobs list` | List jobs |
| `reactor jobs show <name>` | Show job details |
| `reactor jobs trigger <name>` | Manually trigger a job |
| `reactor jobs runs <name>` | List job runs |
| `reactor jobs dlq list` | List dead letter queue entries |

### Data

| Command | Description |
|---------|-------------|
| `reactor data migrate` | Run database migrations |
| `reactor data inspect <table>` | Inspect table schema |
| `reactor data query --sql <sql>` | Execute a read-only query |

### Server Administration

| Command | Description |
|---------|-------------|
| `reactor doctor` | Run diagnostic checks |
| `reactor version` | Show CLI and server versions |
| `reactor migrate` | Run all pending migrations |

### Local Development

| Command | Description |
|---------|-------------|
| `reactor dev` | Start local server (foreground) |
| `reactor up` | Start local server (background) |
| `reactor down` | Stop local server |
| `reactor status` | Show local server status |

## Global Flags

| Flag | Env | Description |
|------|-----|-------------|
| `--context, -c` | `REACTOR_CONTEXT` | Context to use |
| `--manifest, -m` | — | Path to reactor.toml |
| `--output, -o` | `REACTOR_OUTPUT` | Output format (human, json) |
| `--yes` | `REACTOR_ASSUME_YES` | Skip confirmation prompts |
| `--verbose, -v` | — | Enable verbose output |
| `--token` | `REACTOR_TOKEN` | Override authentication token |

## Output Format

By default, output is human-readable when connected to a terminal, and JSON when piped or scripted.

Force a specific format with `--output`:

```bash
# Always JSON
reactor functions list --output json

# Always human-readable
reactor functions list --output human
```

### JSON Output Contract

Success responses:

```json
{
  "ok": true,
  "data": { ... }
}
```

Error responses:

```json
{
  "ok": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable message",
    "hint": "Optional suggestion"
  }
}
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | User error (invalid arguments, missing flags) |
| 2 | Configuration error (invalid context, missing manifest) |
| 3 | Authentication error (invalid token, permission denied) |
| 4 | Validation error (schema violation, bundle error) |
| 5 | Server error (5xx response, deployment failed) |
| 6 | Network error (connection refused, timeout) |

## Token Precedence

Authentication tokens are resolved in this order:

1. `--token` flag
2. `REACTOR_TOKEN` environment variable
3. Context-specific environment variable
4. OS keychain (if configured)

## Configuration Files

### `~/.reactor/config.toml`

Global CLI configuration:

```toml
default = "production"

[contexts.production]
endpoint = "https://api.example.com"
org = "acme"

[contexts.production.auth]
kind = "keychain"
service = "reactor"
account = "production"

[contexts.local]
endpoint = "http://localhost:8080"

[contexts.local.auth]
kind = "token-env"
env = "REACTOR_LOCAL_TOKEN"
```

### `reactor.toml`

Project manifest:

```toml
project_id = "my-app"
name = "My Application"
default_context = "local"

[functions]
path = "functions"

[sites]
path = "sites"

[data]
migrations_path = "migrations"

[jobs]
path = "jobs"
```

## License

MIT
