# Reactor Studio

Desktop developer interface for Reactor. Built with Tauri 2 + React + Rust.

## Overview

Reactor Studio is the developer's home for any project built on Reactor. It combines:

- A **four-pane UI** (agent rail / conversation sidebar / main tabbed pane / files rail)
- A **Rust agent harness** for AI-powered development workflows
- A **Task system** with six-phase pipelines (Alignment → Planning → Development → Testing → UAT → Deployment)
- **Reactor Cloud integration** for deployment status and management

## Development

### Prerequisites

- Node.js >= 20
- pnpm >= 9
- Rust (latest stable)
- Tauri CLI: `cargo install tauri-cli`

### Setup

```bash
cd studio
pnpm install
pnpm dev
```

### Build

```bash
pnpm build
```

## Project Structure

```
studio/
├── apps/
│   └── studio/           # Tauri + React app
│       ├── src/          # React renderer
│       └── src-tauri/    # Rust backend
└── crates/               # Shared Rust crates (Phase 1+)
    ├── studio-agent/
    ├── studio-protocol/
    ├── studio-providers/
    ├── studio-storage/
    ├── studio-task/
    └── studio-tools/
```

## Architecture

See [docs/reactor-studio_design.md](../docs/reactor-studio_design.md) for the full design document.
