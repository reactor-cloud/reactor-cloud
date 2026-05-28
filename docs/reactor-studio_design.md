# Reactor Studio Design

> Design document for `studio/` — the Reactor desktop developer interface (Tauri + React + Rust agent harness)

## Status: Draft
## Author: AI Assistant
## Date: 2026-05-18

---

## 1. Overview

### Problem Statement

Reactor today is a backend platform (`reactor-server`, `reactor-cli`, capability servers, `reactor.cloud` control plane). Developers building **on** Reactor have nowhere to *live* day-to-day: they jump between an editor (Cursor/VS Code), a browser tab for `reactor.cloud`, terminals for `reactor-cli`, and ad-hoc AI chats with no persistent project memory or workflow structure.

**Reactor Studio** is a desktop application — Tauri (Rust + WebView2/wry) — that becomes the developer's home for any project built on Reactor. It marries:

- A **chat-and-tabs UI** lifted from `Awesome` (the four-pane layout: agent rail / conversation rail / main tabbed pane / files rail).
- A **Rust agent harness** lifted from `1jehuang/jcode` (agent loop, primitive tools, providers, memory, plan/skill subsystem, self-modifying `selfdev` tools).
- A **Task workflow** new to Reactor: every feature request is a multi-phase pipeline (Alignment → Planning → Development → Testing → UAT → Deployment) with one conversation per phase, locked/unlocked progressively.
- A **Reactor Cloud control surface**: the cloud dashboard embedded as a tab; the primary control surface is the agent which has access to the full `reactor-cli`.

### Goals

1. **One window = one project.** Opening any local folder converts it into a Reactor project by scaffolding a `.reactor/` directory. No project switcher.
2. **Closely mirror Cursor's feel** for chat, tabs, file browser, diff review.
3. **Closely mirror Awesome's UX shell**: agent rail, conversations sidebar, tabbed main pane, files/plugins right rail.
4. **Closely mirror Cursor's agent capability**: full filesystem, shell, browser, code edit, diff/apply, MCP tools.
5. **Keep jcode's selfdev capability**: the agent can modify and reload its own harness from inside Studio.
6. **Task-driven development**: features and changes flow through a predefined six-phase pipeline with phase-scoped conversations.
7. **First-class Reactor Cloud integration**: deployment status, logs, metrics, env vars are queryable from the agent and viewable in a dedicated tab.
8. **Reuse-first**: minimize net-new code by lifting ~70% from Awesome (React renderer) and ~70% from jcode (Rust harness).

### Non-Goals (v0)

- Cross-project navigation, project gallery, multi-workspace sync.
- Plugin marketplace (we ship a static set of agents/tools; user-editable on disk).
- Team collaboration / shared sessions (single user, local-first).
- Mobile/web build (desktop only).
- Spreadsheet/presentation views (Awesome has them; not relevant here).
- Self-hosting the LLM (we route through provider gateway / user's API keys).

### Out of scope, but pre-wired

- Cloud sync of `.reactor/` state — schema designed so it could ship to Reactor Cloud later.
- Multi-developer Task handoff — phase state machine designed to allow it.
- Built-in code review for human reviewers (the diff view supports it but no PR integration in v0).

---

## 2. Repository Layout

Studio lives at the repo root in `studio/` and will eventually be evicted to its own repo.

```
Reactor/
├── studio/
│   ├── package.json                  # workspace root (pnpm)
│   ├── pnpm-workspace.yaml
│   ├── tsconfig.base.json
│   ├── tauri.conf.json
│   ├── README.md
│   │
│   ├── apps/
│   │   └── studio/                   # the Tauri app
│   │       ├── src/                  # React renderer (port of Awesome's src/renderer)
│   │       │   ├── App.tsx
│   │       │   ├── main.tsx
│   │       │   ├── components/
│   │       │   │   ├── layout/       # TitleBar, ChatPanel, MainPane, FileBrowserPanel
│   │       │   │   ├── chat/         # AgentBar, ConversationList, ChatView, MessageList, ...
│   │       │   │   ├── tasks/        # NEW: TaskRail, TaskList, TaskView, PhaseStepper
│   │       │   │   └── ui/           # primitives
│   │       │   ├── views/            # tab view registry (Awesome's pattern)
│   │       │   │   ├── code-editor/  # Monaco diff/editor
│   │       │   │   ├── markdown/
│   │       │   │   ├── document/     # TipTap
│   │       │   │   ├── browser/
│   │       │   │   ├── conversation/ # pop-out chat into tab
│   │       │   │   ├── diff/         # NEW: agent-proposed change review
│   │       │   │   ├── reactor-cloud/# NEW: cloud dashboard tab
│   │       │   │   ├── terminal/
│   │       │   │   ├── settings/
│   │       │   │   └── new-tab/
│   │       │   ├── hooks/
│   │       │   ├── lib/
│   │       │   │   ├── ipc.ts        # @tauri-apps/api wrapper
│   │       │   │   ├── reactor.ts    # @reactor/client SDK wrapper
│   │       │   │   └── utils.ts
│   │       │   └── data/
│   │       │       ├── agents.ts     # default Reactor agents
│   │       │       └── task-template.ts
│   │       ├── public/
│   │       ├── index.html
│   │       ├── vite.config.ts
│   │       ├── tailwind.config.js
│   │       └── src-tauri/
│   │           ├── Cargo.toml
│   │           ├── tauri.conf.json
│   │           ├── build.rs
│   │           ├── icons/
│   │           └── src/
│   │               ├── main.rs              # Tauri entry; mounts services
│   │               ├── ipc/                 # Tauri command/event surface
│   │               │   ├── agent.rs
│   │               │   ├── task.rs
│   │               │   ├── workspace.rs
│   │               │   ├── files.rs
│   │               │   └── cloud.rs
│   │               └── lib.rs
│   │
│   └── crates/                       # Rust crates owned by Studio
│       ├── studio-agent/             # port of jcode-agent-runtime + src/agent
│       ├── studio-tools/             # port of jcode-tool-core + concrete tools
│       │   ├── fs/                   # read, write, edit, multiedit, apply_patch
│       │   ├── search/               # grep, glob, agentgrep, codesearch
│       │   ├── shell/                # bash
│       │   ├── browser/              # webview-driven browser tool
│       │   ├── lsp/                  # language server bridge
│       │   ├── mcp/                  # MCP client
│       │   ├── reactor/              # NEW: reactor-cli wrapper, cloud client tool
│       │   ├── task/                 # task/batch/background tools
│       │   ├── selfdev/              # port of src/tool/selfdev — harness rebuild
│       │   └── todo/
│       ├── studio-protocol/          # port of jcode-protocol + message-types + session-types
│       ├── studio-providers/         # port of jcode-provider-* (openai, openrouter, gemini, gateway)
│       ├── studio-memory/            # port of jcode-memory + memory_agent + memory_graph
│       ├── studio-plan/              # port of jcode-plan; extended for Task phases
│       ├── studio-skill/             # port of jcode-skill
│       ├── studio-compaction/        # port of jcode-compaction-core
│       ├── studio-storage/           # port of jcode-storage — targets `.reactor/`
│       ├── studio-task/              # NEW: Task state machine over studio-plan
│       └── studio-cloud/             # NEW: reactor.cloud client (uses @reactor/client over FFI? or direct HTTP)
```

The Studio Rust crates live inside `studio/` (not at the repo's top-level `crates/`) so the eventual eviction to its own repo is a clean `git mv studio/ ../reactor-studio/`.

---

## 3. Application Model

### One Window = One Project

- Launching Studio with no project opens a small "Open Folder" window (port of Awesome's `WorkspaceScreen`, stripped down).
- Selecting a folder:
  1. Scaffolds `.reactor/` if absent (idempotent; harmless on already-initialized folders).
  2. Spawns a new Tauri window bound to that folder.
  3. Closes the launcher window.
- Each window has its own Rust agent runtime instance, its own `.reactor/` storage, its own task state, its own conversations.
- No project list, no "recent projects" inside a window. The OS handles window management.

`File → Open Folder…` from the menu opens **another** window. Closing the last project window quits the app (configurable).

### `.reactor/` Project State Directory

```
<project-root>/
├── .reactor/
│   ├── config.toml                   # studio settings for this project
│   ├── agents/                       # agent definitions (yaml + prompt.md)
│   │   ├── _shared/
│   │   │   ├── project-profile.md
│   │   │   └── conventions.md
│   │   ├── planner/
│   │   │   ├── agent.yaml
│   │   │   └── prompt.md
│   │   ├── coder/
│   │   ├── reviewer/
│   │   ├── tester/
│   │   ├── deployer/
│   │   └── researcher/
│   ├── tasks/
│   │   ├── <task-id>/
│   │   │   ├── task.yaml             # title, state, owner agent, created/updated
│   │   │   ├── phases/
│   │   │   │   ├── 01-alignment/
│   │   │   │   │   ├── conversation.jsonl
│   │   │   │   │   ├── status.json   # active | completed | locked
│   │   │   │   │   └── artifacts/
│   │   │   │   ├── 02-planning/
│   │   │   │   │   └── plan.md
│   │   │   │   ├── 03-development/
│   │   │   │   │   └── changes/      # patches, diffs
│   │   │   │   ├── 04-testing/
│   │   │   │   │   └── reports/
│   │   │   │   ├── 05-uat/
│   │   │   │   └── 06-deployment/
│   │   │   │       └── receipts/
│   │   │   └── progress.md           # rolling summary
│   │   └── index.json                # task list metadata
│   ├── conversations/                # ad-hoc chats not bound to a task
│   │   └── <conversation-id>.jsonl
│   ├── memory/                       # agent memory (jcode-memory format)
│   │   ├── graph.json
│   │   └── notes/
│   ├── skills/                       # user-installed skill bundles
│   ├── index/                        # workspace embedding index (optional)
│   │   ├── embeddings.bin
│   │   └── metadata.json
│   ├── credentials/                  # encrypted vault (OS keychain-backed key)
│   │   └── vault.enc
│   ├── snapshots/                    # selfdev + replay snapshots
│   └── cache/
└── (user's project files)
```

All of this is plain files. The user can `git add .reactor/` if they want history of agent conversations, or `.gitignore` it (we ship a recommended `.gitignore` template that ignores credentials/cache/index but keeps tasks and plans).

### `.reactor/config.toml` (per-project)

```toml
[project]
name = "my-app"
created = "2026-05-18T14:00:00Z"

[agents]
default = "planner"

[providers]
default = "openrouter"

[providers.openrouter]
# api key sourced from vault

[cloud]
project_id = "rc_abc123"          # optional; set when linked to reactor.cloud
endpoint = "https://api.reactor.cloud"

[tasks]
phases = [
  "alignment",
  "planning",
  "development",
  "testing",
  "uat",
  "deployment",
]

[index]
enabled = true
```

A **global** Studio config lives at `~/.config/reactor-studio/config.toml` for cross-project preferences (theme, default models, keychain settings).

---

## 4. UI Architecture

### Four-Pane Layout (port from Awesome)

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Reactor Studio — <project-name>                          [−] [□] [×]    │
├──────┬────────────────┬───────────────────────────────────────┬──────────┤
│      │                │  [Tab 1] [Tab 2] [Tab 3] [+]          │          │
│  🦋  │  Conversations │───────────────────────────────────────│  Files   │
│      │  with Planner  │                                       │          │
│  🤖  │                │                                       │  ▼ src/  │
│      │  ● Pipeline    │           Main Tab Content            │    ...   │
│  📐  │  ○ Setup chat  │     (Markdown / Browser /             │  ▼ ...   │
│      │                │      Cloud Dashboard / Code /         │          │
│  💻  │  Tasks (3)     │      Diff / Conversation)             │  Plugins │
│      │  ● Add auth ⏳ │                                       │          │
│  ✅  │  ✓ Init schema │                                       │          │
│      │  ○ Fix CORS    │                                       │          │
│  🚀  │                │                                       │          │
│      │────────────────│                                       │          │
│  📚  │  [Chat input]  │                                       │          │
│      │                │                                       │          │
│  +   │                │                                       │          │
└──────┴────────────────┴───────────────────────────────────────┴──────────┘
   ^         ^                          ^                            ^
   |         |                          |                            |
AgentBar  ChatPanel                  MainPane                  FileBrowserPanel
+TaskRail (selected→TaskList         (Tauri-managed             (Awesome
          ↳ TaskView)                 tabbed views)              FileBrowserPanel)
```

### AgentBar (leftmost, ~56px)

Direct port of `Awesome/src/renderer/components/chat/AgentBar.tsx`.

- Logo at top.
- Vertical list of agent avatars (default Reactor agents + user-added).
- **A `Tasks` entry** at the bottom of the agent list (visually distinguished with a list icon, not an avatar) that selects the Task rail mode instead of an agent.
- `+` at the bottom to add an agent.

### ChatPanel (sidebar, ~380px, resizable)

Port of `Awesome/src/renderer/components/layout/ChatPanel.tsx`. The sidebar has **four** modes (extends Awesome's three):

| AgentBar selection | Sidebar mode | Content |
|---|---|---|
| An agent | `conversations` | `ConversationList` filtered to that agent (ad-hoc chats) |
| An agent + a conversation | `chat` | `ChatView` for the conversation |
| `Tasks` entry | `tasks` (NEW) | `TaskList`: all tasks for the project with state badges |
| `Tasks` + a task | `task` (NEW) | `TaskView`: phase stepper + selected phase's conversation |
| `+` Add agent | `agent-picker` | `AgentPicker` to enable a stored agent |

### TaskList (NEW)

```
┌─────────────────────────────────────┐
│ Tasks                          + New│
├─────────────────────────────────────┤
│ ● Add auth flow                     │
│   Development · 2h ago              │
├─────────────────────────────────────┤
│ ○ Initialize schema                 │
│   ✓ Deployed · 3d ago               │
├─────────────────────────────────────┤
│ ○ Fix CORS on /api/upload           │
│   Alignment · 10m ago               │
└─────────────────────────────────────┘
```

- Title + current phase badge + relative timestamp.
- Click to open the `task` view.
- `+ New` opens an Alignment conversation immediately with the user's title prompt.

### TaskView (NEW — the centerpiece)

```
┌──────────────────────────────────────────────────────────┐
│ ← Tasks    Add auth flow                          ⋯      │
├──────────────────────────────────────────────────────────┤
│  ✓ Alignment   →  ● Planning  →  ○ Dev  →  ○ Test  →  …  │
│  (readonly)       (active)       (locked)                │
├──────────────────────────────────────────────────────────┤
│                                                          │
│           [ Active phase: Planning ]                     │
│                                                          │
│           Conversation messages stream here              │
│           (same ChatView component, scoped to            │
│            this phase's conversation.jsonl)              │
│                                                          │
│           [ Move to Development → ]                      │
│                                                          │
├──────────────────────────────────────────────────────────┤
│  [Chat input — disabled if phase locked/completed]       │
└──────────────────────────────────────────────────────────┘
```

- **PhaseStepper** at top: horizontal list of the six phases with state icons (`✓` completed, `●` active, `○` upcoming, `🔒` locked).
- Clicking a completed phase shows it in **readonly** mode (its conversation, its artifacts).
- The active phase has a live `ChatView` (the same component used everywhere) bound to `tasks/<id>/phases/<n>-<name>/conversation.jsonl`.
- Each phase has a phase-specific "advance" affordance:
  - **Alignment → Planning**: agent decides when alignment is complete (tool call `task_advance`) — the UI shows a button that becomes active when the agent emits readiness. Manual override is available.
  - **Planning → Development**: requires `plan.md` to exist and be approved by the user.
  - **Development → Testing**: requires changes to have been committed (or a manual confirmation).
  - **Testing → UAT**: requires test reports.
  - **UAT → Deployment**: requires user approval.
  - **Deployment → done**: requires a deploy receipt (from `studio-cloud`).
- Past phases are **readonly** but inspectable; the user can re-open prior conversations to see decisions.
- The right sidebar (Files) can pin per-phase artifacts (the plan, the diff, the test report).

### MainPane (center, flexible, tabbed)

Port of `Awesome/src/renderer/views/` framework wholesale: `registry.ts`, `TabBar`, `ViewContainer`, `EmptyState`, `useViews()`, `TabPersistence`.

**Default view types in v0:**

| View | Source | Notes |
|---|---|---|
| `new-tab` | Awesome direct port | Start screen with quick actions |
| `markdown` | Awesome direct port | Rendering + edit |
| `document` | Awesome direct port | TipTap rich-text |
| `code-editor` | Awesome direct port | Monaco; extended with diff mode |
| `browser` | Awesome adapted | Tauri WebView via `tauri-plugin-webview` |
| `diff` | **NEW** | Side-by-side / unified diff for agent-proposed changes; per-hunk accept/reject |
| `reactor-cloud` | **NEW** | Dashboard for the linked Reactor Cloud project |
| `conversation` | Awesome direct port | Pop-out chat into a tab |
| `terminal` | Awesome direct port | xterm.js + Tauri shell |
| `settings` | Awesome direct port + Reactor-specific sections | Account, providers, agents, cloud link, keys |

Views can be opened by the agent via the `view_open` tool (port from jcode).

### FileBrowserPanel (right, ~250px, collapsible)

Direct port of `Awesome/src/renderer/components/layout/FileBrowserPanel.tsx`. Two modes:

- **Files** (default): tree view of the project root with right-click context menu.
- **Plugins** (NEW tab in the panel header): list of installed skills/MCP servers/tools with enable/disable toggles.

---

## 5. Agent Loop & Tools (Rust side)

### Architecture

```
┌───────────────────────────────────────────────────────────────────────┐
│                       React Renderer (WebView)                        │
│  Awesome chat components, view registry, TaskView, file tree          │
└────────────────────────────────┬──────────────────────────────────────┘
                                 │ Tauri IPC (commands + events)
                                 │ protocol: studio-protocol shapes
                                 ▼
┌───────────────────────────────────────────────────────────────────────┐
│                       Tauri Main Process (Rust)                       │
│                                                                       │
│  ┌──────────────────────┐   ┌────────────────────┐   ┌──────────────┐ │
│  │  studio-agent        │   │  studio-task       │   │ studio-cloud │ │
│  │  (jcode agent loop)  │◀──│  (phase machine)   │──▶│ reactor.cloud│ │
│  └──────────┬───────────┘   └────────────────────┘   │ client       │ │
│             │                                        └──────────────┘ │
│             ▼                                                         │
│  ┌──────────────────────┐   ┌────────────────────┐   ┌──────────────┐ │
│  │  studio-tools        │   │  studio-providers  │   │ studio-mem.  │ │
│  │  fs/search/shell/    │   │  openai/openrouter │   │              │ │
│  │  browser/lsp/mcp/    │   │  gemini/gateway    │   │ studio-plan  │ │
│  │  reactor/selfdev/    │   └────────────────────┘   │              │ │
│  │  task/todo           │                            │ studio-skill │ │
│  └──────────────────────┘                            └──────────────┘ │
│                                                                       │
│  ┌──────────────────────────────────────────────────────────────────┐ │
│  │  studio-storage  ──▶  <project>/.reactor/                        │ │
│  └──────────────────────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────┘
```

### Crates (all under `studio/crates/`)

| Crate | Source | Role |
|---|---|---|
| `studio-protocol` | port of `jcode-protocol` + message/session/task types | Wire shapes shared with renderer (serde+TS bindings via `specta`) |
| `studio-agent` | port of `jcode-agent-runtime` + `src/agent` | Core loop: context → LLM → stream → tool call → loop |
| `studio-tools` | port of `jcode-tool-core` + tool impls under `src/tool/` | Tool registry, primitives, executor |
| `studio-providers` | port of `jcode-provider-*` + `src/gateway.rs` | LLM provider abstraction; OpenAI, OpenRouter, Gemini, AI Gateway |
| `studio-memory` | port of `jcode-memory-types` + `src/memory*` | Persistent agent memory + graph |
| `studio-plan` | port of `jcode-plan` + `src/plan.rs` | Planning subsystem (`plan.md` generation) |
| `studio-skill` | port of `jcode-skill` + `src/skill.rs` + `src/tool/skill.rs` | Skill loading & execution |
| `studio-compaction` | port of `jcode-compaction-core` + `src/compaction.rs` | Context window compaction |
| `studio-storage` | port of `jcode-storage` + `src/storage/` | Targets `.reactor/`; conversation/task/memory persistence |
| `studio-task` | **NEW** | Task state machine (6 phases) sitting on top of `studio-plan` + `studio-storage` |
| `studio-cloud` | **NEW** | Reactor Cloud API client; wraps `reactor-cli` for the agent tool |

### Tools (initial set)

Direct ports from jcode's `src/tool/`:

- **Filesystem**: `file_read`, `file_write`, `file_edit`, `multiedit`, `apply_patch`, `ls`, `glob`, `open`
- **Search**: `grep`, `agentgrep`, `codesearch`, `conversation_search`, `session_search`
- **Shell**: `bash` (with sandboxing/permission prompts)
- **Browser**: `browser_navigate`, `browser_action` (via Tauri WebView)
- **Code intelligence**: `lsp` (language server bridge)
- **MCP**: `mcp` (MCP client; reuses `studio-skill` for static schemas)
- **Web**: `webfetch`, `websearch`
- **Memory**: `memory_*` family
- **Task management**: `task`, `todo`, `batch`, `bg` (background execution)
- **Communication**: `communicate` (between agents/subagents)
- **Selfdev** (jcode-specific, keep): `selfdev_build`, `selfdev_launch`, `selfdev_reload`, `selfdev_status` — agent can rebuild and reload the harness from inside a session

**New Reactor-specific tools (`studio-tools/reactor`):**

- `reactor_cli` — wraps the `reactor` CLI binary (already in `crates/reactor-cli`); the agent has full access to deploy, env, logs, db, storage, etc.
- `reactor_cloud_status` — structured project status (deployments, branches, recent jobs)
- `reactor_cloud_deploy` — triggers a deploy with a checked-in `Reactor.toml`
- `reactor_cloud_logs` — tails logs from a deployment
- `task_advance` — agent-callable: marks the current phase ready to advance (UI surfaces the button)
- `task_artifact_write` — writes to `tasks/<id>/phases/<n>/artifacts/`

### Agent Definitions (default set, shipped in app bundle and copied to `.reactor/agents/` on init)

| Agent | Role | Tools | Model preference |
|---|---|---|---|
| `planner` | Default orchestrator; runs the Task pipeline | all + `task_advance` + `delegate` | power |
| `coder` | Writes code, edits files, runs tests | fs, search, shell, lsp, mcp | power |
| `reviewer` | Reviews diffs, flags issues | fs, search, lsp, `diff` view | fast |
| `tester` | Runs tests, generates test reports | shell, fs | fast |
| `deployer` | Deploys via reactor_cli, monitors logs | reactor_*, shell | fast |
| `researcher` | Web research, docs, exploration | webfetch, websearch, browser, fs | fast |

All editable by the user: they're plain `agent.yaml` + `prompt.md` files in `.reactor/agents/`.

### Streaming Protocol

Identical to jcode's:

```rust
enum StreamChunk {
    Thinking { content: String },
    Text { content: String },
    ToolCall { id: String, name: String, params: Value },
    ToolResult { id: String, result: ToolResult },
    Error { message: String },
    Done,
}
```

Emitted from Rust via `tauri::Window::emit("agent:chunk", ...)` and consumed by Awesome's `MessageList` / `ToolCallDisplay` / `StreamingMarkdown` components.

### Self-Modifying Harness (selfdev)

Lifted verbatim from `jcode/src/tool/selfdev/`:

- `selfdev_status` — reports build state, last reload, current revision
- `selfdev_build` — schedules a rebuild of the `studio-*` crates in a worker
- `selfdev_launch` — launches a new instance of the harness against new artifacts
- `selfdev_reload` — hot-swaps providers/tools that don't require a binary restart

Reactor-specific addition: selfdev changes are gated behind a confirmation modal in the UI by default, with a per-project setting to auto-approve.

---

## 6. Task System (NEW)

### State Machine

```
Alignment ──readiness──▶ Planning ──plan_approved──▶ Development
                                                          │
                                                  changes_committed
                                                          ▼
       Deployment ◀──uat_approved── UAT ◀──tests_passed── Testing
            │
       deploy_ok
            ▼
          Done
```

Each transition is captured as an event in `tasks/<id>/task.yaml`:

```yaml
id: task_2026-05-18_add-auth
title: Add authentication flow
state: development
created: 2026-05-18T14:00:00Z
phases:
  - name: alignment
    status: completed
    started: 2026-05-18T14:00:00Z
    completed: 2026-05-18T14:23:00Z
    summary: "Agreed on email/password + magic link, no OAuth in v1"
  - name: planning
    status: completed
    started: 2026-05-18T14:23:00Z
    completed: 2026-05-18T15:01:00Z
    artifact: phases/02-planning/plan.md
  - name: development
    status: active
    started: 2026-05-18T15:01:00Z
  - name: testing
    status: locked
  - name: uat
    status: locked
  - name: deployment
    status: locked
```

### Phase Conversations

- Each phase has **one** conversation by default (`phases/<n>-<name>/conversation.jsonl`).
- The conversation is bound to a primary agent for that phase (configurable; defaults: Planner for alignment+planning, Coder for development, Tester for testing, user for UAT, Deployer for deployment).
- Conversation messages reference the parent task via metadata so the agent can read prior-phase summaries.

### Locking Semantics

- **Locked** future phases: invisible chat input; conversation does not exist yet.
- **Active** phase: chat input enabled, agent running.
- **Completed** phase: chat input disabled; conversation messages rendered readonly; "View artifacts" link.
- On phase advance:
  1. Current phase status → `completed`, write summary.
  2. Next phase status → `active`, initialize conversation with system message containing prior-phase summaries.
  3. Emit `task:phase-changed` event so renderer can refresh.

### Rust Layer (`studio-task`)

```rust
pub struct Task {
    pub id: TaskId,
    pub title: String,
    pub state: TaskState,
    pub phases: Vec<Phase>,
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
}

pub enum TaskState {
    Alignment, Planning, Development, Testing, Uat, Deployment, Done, Abandoned,
}

pub struct Phase {
    pub name: PhaseName,
    pub status: PhaseStatus,
    pub started: Option<DateTime<Utc>>,
    pub completed: Option<DateTime<Utc>>,
    pub summary: Option<String>,
    pub artifacts: Vec<PathBuf>,
}

pub enum PhaseStatus { Locked, Active, Completed }

pub trait TaskStore {
    fn create(&self, title: &str) -> Result<TaskId>;
    fn get(&self, id: &TaskId) -> Result<Task>;
    fn list(&self) -> Result<Vec<TaskSummary>>;
    fn advance(&self, id: &TaskId, summary: &str) -> Result<Task>;
    fn append_message(&self, id: &TaskId, phase: PhaseName, msg: Message) -> Result<()>;
}
```

The Task state machine sits on top of `studio-plan` (which provides plan-generation primitives) and `studio-storage` (which provides JSONL append and YAML persistence).

---

## 7. Reactor Cloud Integration

### Linking

- Studio can be in two states per project: **unlinked** (purely local) or **linked** (bound to a `reactor.cloud` project).
- Linking writes `[cloud] project_id = "..."` to `.reactor/config.toml` and stores credentials in the vault.
- Linking is triggered from the Reactor Cloud tab or via `reactor_cli link` invoked by the agent.

### Reactor Cloud Tab

Native React view (not embedded webview), talking to `reactor.cloud` via `@reactor/client` SDK (the existing JS SDK design at `docs/reactor-js-sdk_design.md`).

```
┌──────────────────────────────────────────────────────────────────┐
│  Reactor Cloud — my-app                            [Open in web] │
├──────────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐   │
│  │ Production       │  │ Preview          │  │ Branches         │   │
│  │ ✓ Healthy        │  │ ✓ Healthy        │  │ main + 2 PRs     │   │
│  │ v0.34.2 · 2h ago │  │ v0.35.0 · 12m    │  │                  │   │
│  └─────────────────┘  └─────────────────┘  └─────────────────┘   │
│                                                                  │
│  Recent deployments                                              │
│  ───────────────────────────────────────────────────────────     │
│  ✓ feat/auth · v0.35.0 · 12m ago      [Logs] [Promote]           │
│  ✓ main      · v0.34.2 · 2h ago       [Logs]                     │
│  ✗ chore/cors · v0.33.0 · 1d ago      [Logs]                     │
│                                                                  │
│  Jobs · Storage · Functions · Database — small status cards      │
└──────────────────────────────────────────────────────────────────┘
```

- The view has lightweight controls (logs, promote, rollback, env-var quick view).
- **The primary control surface is the agent**, which has the full `reactor_cli` tool. Anything you could do with the CLI, the agent can do.
- "Open in web" punches out to the full `reactor.cloud` Astro site for deeper management.

### Deployer Agent

The default `deployer` agent has a prompt that knows:

- The project's `Reactor.toml` and how to read/edit it.
- The lifecycle of a deploy (build → upload → promote → smoke-test).
- How to consult `studio-cloud` for status/logs.
- How to react to deploy failures (read logs, suggest fix, optionally open a new task).

---

## 8. IPC Surface (Tauri commands + events)

### Commands (renderer → main)

| Command | Args | Returns | Notes |
|---|---|---|---|
| `workspace.open` | `{ path }` | `{ projectId }` | Scaffolds `.reactor/` if needed |
| `workspace.state` | — | `WorkspaceState` | Restored on window load |
| `agent.list` | — | `Agent[]` | Reads from `.reactor/agents/` |
| `agent.send` | `{ agentId, conversationId, message }` | `void` | Streams via events |
| `agent.cancel` | `{ conversationId }` | `void` | |
| `conversation.list` | `{ agentId }` | `ConversationSummary[]` | |
| `conversation.messages` | `{ conversationId }` | `Message[]` | |
| `task.list` | — | `TaskSummary[]` | |
| `task.create` | `{ title }` | `{ taskId }` | Starts Alignment phase |
| `task.get` | `{ taskId }` | `Task` | |
| `task.advance` | `{ taskId, summary? }` | `Task` | Manual override |
| `task.send` | `{ taskId, phase, message }` | `void` | Phase-scoped send |
| `task.phase-messages` | `{ taskId, phase }` | `Message[]` | |
| `tool.approve` | `{ toolCallId, approved }` | `void` | For sandboxed tool calls |
| `file.read/write/list/watch` | path-y args | | Mirrors Awesome's `fileAPI` |
| `view.open/close/switch/save` | tab-y args | | Mirrors Awesome's `viewsAPI` |
| `cloud.status` | — | `CloudStatus` | |
| `cloud.deploy` | `{ branch, env }` | `{ deployId }` | |
| `cloud.logs.tail` | `{ deployId }` | `void` | Streams via events |
| `selfdev.status` | — | `SelfdevStatus` | |
| `selfdev.build/launch/reload` | — | `void` | Confirmation-gated |

### Events (main → renderer)

| Event | Payload | Notes |
|---|---|---|
| `agent:chunk` | `{ conversationId, chunk: StreamChunk }` | Same shape as jcode's protocol |
| `agent:tool-call` | `{ conversationId, call }` | For granular display |
| `agent:tool-result` | `{ conversationId, result }` | |
| `agent:complete` | `{ conversationId }` | |
| `agent:error` | `{ conversationId, error }` | |
| `task:phase-changed` | `{ taskId, from, to }` | Drives PhaseStepper update |
| `task:advance-ready` | `{ taskId }` | Lights up the "Move to next phase" button |
| `file:changed` | `{ path }` | From workspace watcher |
| `cloud:status-changed` | `CloudStatus` | Polled / streamed |
| `cloud:log-line` | `{ deployId, line }` | For Cloud tab tail |
| `selfdev:build-progress` | `{ stage, message }` | |

Types are generated from Rust to TypeScript via `specta` so the renderer is type-safe against the protocol.

---

## 9. Authentication & Credentials

### Provider Credentials

- Stored in `.reactor/credentials/vault.enc`.
- Encryption key sourced from OS keychain (macOS Keychain / Windows Credential Manager / Secret Service on Linux).
- User can paste OpenRouter / OpenAI / Anthropic / Gemini keys in Settings → Providers.

### Reactor Cloud Auth

- Two modes: device-code flow (recommended) or PAT.
- Token stored in the vault.
- `studio-cloud` refreshes silently using the JS SDK's auth strategies adapted for Rust.

---

## 10. Reuse Map (concrete file mapping)

### From `Awesome/src/renderer/` → `studio/apps/studio/src/`

| Awesome path | Studio path | Action |
|---|---|---|
| `App.tsx` | `App.tsx` | Port; replace Electron IPC handler with Tauri; add Tasks rail wiring |
| `components/layout/{TitleBar,ChatPanel,MainPane,FileBrowserPanel}.tsx` | same | Direct port |
| `components/layout/AuditPanel.tsx` | drop | Awesome-specific |
| `components/chat/*` (30 files) | `components/chat/*` | Direct port; some renames |
| `components/ui/*` | `components/ui/*` | Direct port |
| `components/Welcome/*`, `components/Workspace/*` | `components/Welcome/*` | Adapt to Reactor branding + "Open Folder" only |
| `components/Awesome/`, `components/Super/`, `components/Skills/`, `components/Connections/` | drop | Awesome-specific |
| `components/Settings/`, `components/Updates/` | port | Adapt to Reactor |
| `components/TabPersistence.tsx` | direct | |
| `views/registry.ts`, `views/types.ts`, `views/index.ts`, `views/hooks/*`, `views/components/*` | direct | |
| `views/{code-editor,markdown,document,browser,terminal,settings,new-tab,conversation,documentation,pdf,trace}/` | direct port | `browser` needs Tauri WebView adapter |
| `views/{spreadsheet,presentation}/` | drop | Not relevant for Studio v0 |
| `hooks/{useResizable,useTheme,useChatContext,useConversations,useAgents,useChat,useFileBrowser,useFileClipboard,useContextUsage,useTrace,useWindowState,useWorkspace,useMCP,useSkillRegistry}.ts` | direct port | |
| `hooks/{useAuth,useCredentials,useConnections,useAwesome,useWorkspaceSync,useUpdater}.ts` | rewrite | Awesome→Reactor backends |
| `lib/{utils,monaco-config}.ts` | direct | |
| `lib/{awe-api,supabase,config}.ts` | rewrite | Reactor Cloud SDK wrapper |
| `data/agents.ts` | rewrite | Reactor default agents |
| `types/global.ts` | rewrite | Tauri window types instead of Electron |
| `index.css`, `tailwind.config.js`, `postcss.config.js` | direct | |

### From `jcode` (Rust) → `studio/crates/`

| jcode path | Studio crate | Action |
|---|---|---|
| `crates/jcode-agent-runtime` + `src/agent/` | `studio-agent` | Direct port; rename |
| `crates/jcode-tool-core` + `crates/jcode-tool-types` + `src/tool/` | `studio-tools` | Direct port; add `reactor/` submodule |
| `crates/jcode-protocol` + `crates/jcode-message-types` + `crates/jcode-session-types` + `src/protocol/` | `studio-protocol` | Direct port; export TS bindings via specta |
| `crates/jcode-provider-*` + `crates/jcode-provider-catalog` + `src/gateway.rs` + `src/provider/` | `studio-providers` | Direct port |
| `crates/jcode-memory-types` + `src/memory*` | `studio-memory` | Direct port |
| `crates/jcode-plan` + `src/plan.rs` | `studio-plan` | Direct port; extend for Task phases |
| `crates/jcode-skill` + `src/skill.rs` + `src/tool/skill.rs` | `studio-skill` | Direct port |
| `crates/jcode-compaction-core` + `src/compaction.rs` | `studio-compaction` | Direct port |
| `crates/jcode-storage` + `src/storage/` | `studio-storage` | Direct port; target `.reactor/` paths |
| `src/tool/selfdev/` | `studio-tools/selfdev` | Direct port; UI confirmation gate |
| `crates/jcode-task-types` + `src/tool/task.rs` + `src/tool/batch.rs` + `src/tool/bg.rs` | `studio-task` | Adapt; build 6-phase state machine on top |
| `crates/jcode-config-types` + `src/config/` + `src/auth/` | folded into `studio-agent` + `apps/studio/src-tauri/src/ipc/` | Adapt |
| All `crates/jcode-tui-*`, `crates/jcode-desktop`, `src/tui/` | drop | TUI; replaced by React |
| `crates/jcode-mobile-*`, `ios/`, `src/mobile_*` | drop | Out of scope |
| `crates/jcode-notify-email`, `src/{telegram,gmail,dictation,login_qr}.rs`, `telemetry-worker/` | drop | Out of scope |
| `crates/jcode-import-core`, `src/import*` | reconsider in v2 | Useful but not v0 |
| `crates/jcode-ambient-types` + `src/ambient*`, `crates/jcode-overnight-core` + `src/overnight.rs` | reconsider in v2 | Powerful but adds surface |
| `src/transport/` | adapt | Replace stdio framing with Tauri events |
| `src/server.rs`, `src/sidecar.rs` | drop | Replaced by Tauri main |
| `crates/jcode-update-core` | replace | Use Tauri updater |

### New crates / modules

| Path | Role |
|---|---|
| `studio/crates/studio-task` | 6-phase Task state machine |
| `studio/crates/studio-cloud` | Reactor Cloud client (HTTP/SDK wrapper) |
| `studio/apps/studio/src-tauri/src/ipc/*.rs` | Tauri command handlers + event emitters |
| `studio/apps/studio/src/components/tasks/*` | `TaskRail`, `TaskList`, `TaskView`, `PhaseStepper`, `PhaseConversation` |
| `studio/apps/studio/src/views/diff/*` | Diff review view (Monaco diff editor + hunk accept/reject) |
| `studio/apps/studio/src/views/reactor-cloud/*` | Cloud dashboard view |

---

## 11. Phased Rollout

### Phase 0 — Skeleton (week 1)

- Create `studio/` workspace with pnpm + Tauri scaffolding.
- Empty Tauri app shell with the layout (AgentBar / ChatPanel / MainPane / FileBrowserPanel) stubbed.
- `.reactor/` scaffolding on folder open; folder picker.
- Port Awesome's `views/registry.ts` and `views/components/*` so tabs work.

**Exit criteria**: launch app, open a folder, see the four-pane shell with the file tree populated; open a markdown file in a tab.

### Phase 1 — Port jcode agent core (weeks 2–3)

- Lift `jcode-agent-runtime`, `jcode-tool-core`, `jcode-protocol`, `jcode-providers`, `jcode-storage`, `jcode-memory` into `studio/crates/`.
- Implement Tauri IPC bridge: `agent.send` + `agent:chunk` events.
- Port Awesome's `ChatView`, `MessageList`, `StreamingMarkdown`, `ToolCallDisplay`, `PromptInput`.
- Port a minimal tool set: `file_read`, `file_write`, `file_edit`, `bash`, `grep`, `glob`.
- One default agent (`coder`) loaded from a baked-in YAML.

**Exit criteria**: have a real conversation with `coder` that reads/writes files in the project.

### Phase 2 — Multi-agent + conversations (week 4)

- Port `AgentBar`, `AgentPicker`, `ConversationList`.
- Load agents from `.reactor/agents/`.
- Persistent conversations per agent in `.reactor/conversations/`.
- Window state persistence (selected agent, active conversation).

**Exit criteria**: switch between Planner / Coder / Researcher with persisted conversations.

### Phase 3 — Task system (weeks 5–6)

- Build `studio-task` crate (6-phase state machine + storage).
- Build TaskRail / TaskList / TaskView / PhaseStepper components.
- Wire `task_advance` tool + `task:phase-changed` events.
- Default Planner agent prompt that drives the Alignment phase.
- Plan agent generates `plan.md` during Planning phase.

**Exit criteria**: create a task "Add hello endpoint", drive it through Alignment → Planning → Development with the agent generating code, and see the phases lock as they advance.

### Phase 4 — Cloud integration + Deployer (week 7)

- Build `studio-cloud` crate wrapping `reactor-cli` and Reactor Cloud HTTP.
- Implement `reactor_cli` tool family.
- Build Reactor Cloud dashboard view.
- Deployer agent prompts + deploy phase wiring.

**Exit criteria**: linked project deploys via the Deployment phase of a task, with status visible in the Cloud tab.

### Phase 5 — Browser, diff, selfdev (week 8)

- Port `views/browser` with Tauri WebView adapter.
- Build `views/diff` with Monaco diff + hunk-level accept/reject (agent uses `apply_patch` after approval).
- Port `selfdev` tools with confirmation gating.

**Exit criteria**: agent can browse the web in a tab, propose code changes via a diff view, and (gated) rebuild itself.

### Phase 6 — Polish & extract (weeks 9–10)

- Port remaining views (terminal, document, PDF, settings).
- MCP client integration via `studio-skill`.
- Updater (Tauri updater).
- Packaging for macOS / Windows / Linux.
- Move `studio/` to its own repo.

---

## 12. Open Questions

1. **`reactor-cli` invocation**: Spawn the binary as a subprocess, or compile it as a Rust library (`reactor-client` is already a crate) and call directly? Subprocess is more honest but slower; library is faster but ties Studio's build to the rest of the workspace.
2. **Diff acceptance flow**: Does the agent always go through a diff-review UI before writing files, or only when the agent flags a change as risky? Cursor-like: write directly with an undo trail vs. always-review.
3. **Per-window vs. per-project agent runtime**: Today this design assumes one Rust runtime per window. If a user opens the same folder twice, what happens? (Suggest: refuse second window.)
4. **Task phase customization**: Six phases are fixed by `.reactor/config.toml`, but should users be able to reorder, skip, or add custom phases per task type? (Suggest: skipping yes via "fast-track" affordance, reordering no in v0.)
5. **Memory scope**: Agent memory (`.reactor/memory/`) is project-scoped. Should there also be a global, cross-project memory? jcode supports both.
6. **Conversation transport for streaming**: Tauri events have a max payload — should we chunk long messages, or stream via a dedicated socket?
7. **Skill/MCP discovery**: Bundle a default set of MCP servers, or rely on user-added? (Suggest: a small default set: filesystem, web, git.)
8. **Vault encryption**: OS keychain only, or also support a passphrase fallback for environments without a keychain? (Suggest: keychain v0, passphrase v1.)
9. **Tauri version**: Tauri 2.x supports more platforms and has a nicer plugin story; Tauri 1.x is more mature. (Suggest: Tauri 2.)
10. **License posture for the jcode port**: jcode is licensed (check `LICENSE`); confirm compatibility before lifting code wholesale.

---

## 13. Appendix: Glossary

- **Project** — a local folder with a `.reactor/` directory.
- **Window** — one Tauri window bound to exactly one project.
- **Agent** — an entity defined by `agent.yaml` + `prompt.md` that participates in conversations.
- **Conversation** — a message thread between the user and an agent (ad-hoc or phase-bound).
- **Task** — a six-phase pipeline modeling a feature/change request; each phase has its own conversation.
- **Phase** — one of `alignment`, `planning`, `development`, `testing`, `uat`, `deployment`.
- **Tool** — a Rust function exposed to the agent's tool-calling loop.
- **View** — a tab type in the MainPane (markdown, browser, cloud dashboard, diff, etc.).
- **Skill** — a bundled set of prompts + tools loadable at runtime (MCP-style).
- **Selfdev** — jcode's mechanism for the agent to rebuild/reload its own harness.

---

*Document version: 0.1*
*Last updated: 2026-05-18*
