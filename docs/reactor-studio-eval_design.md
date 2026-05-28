# Reactor Studio Eval & Foundry Design

> Design document for the **evolutionary** half of Reactor Studio: the lesson system, the eval framework, the Foundry/Client split, the submission pipeline, and the auto-iteration bootstrap.

## Status: Draft
## Author: AI Assistant
## Date: 2026-05-19

---

## 1. Overview

### Relationship to `reactor-studio_design.md`

`reactor-studio_design.md` defines the **harness** — Tauri shell, agent loop, tools, six-phase Task pipeline, Reactor Cloud integration. This document is the companion that defines how that harness **gets better over time**: how it captures, validates, ranks, and ships behavioral improvements without (mostly) shipping new binaries.

The two docs share the same crate naming conventions and `.reactor/` storage layout. Where they overlap (e.g. `studio-skill`, `selfdev`), this doc references the existing design rather than redefining it.

### Problem Statement

A self-modifying agent harness is easy: jcode's `selfdev_*` tools already let the agent rebuild and reload itself. A self-*improving* harness is hard, because **modification without measurement is drift** — changes can degrade performance just as easily as improve it.

We need a structured loop where:

1. Failures and recoveries during real Tasks distill into reusable **lessons**.
2. Lessons are tiered, retrievable, and surfaced as **advisory context** to future Tasks (never authoritative).
3. Lessons earn promotion through **demonstrated utility** (cited use → success), not popularity.
4. The hardest promotions (T3, T4) are gated by an **eval framework** plus a **human review** step.
5. The system ships pre-tuned: a curated set of lessons and tested behaviors, baked in before the first user.

This doc describes the four roles, the tier ladder, the eval framework, the two editions (Client vs Foundry), the upstream submission pipeline, and the bootstrap protocol.

### Goals

1. **Self-improving via measurement**, not blind self-modification.
2. **Reactor-domain-specific**: tuned to building, migrating, improving, and deploying reactor.cloud apps.
3. **Federated**: every Client contributes signal; the Foundry curates and re-publishes.
4. **Privacy-first submissions**: opt-in, redacted, previewable, defense-in-depth on both ends.
5. **Eval IP stays private**: test corpus, fixtures, and rubrics never ship to Clients.
6. **Hot-path delivery for vetted lessons**: signed lesson packs reach Clients in hours, not weeks.
7. **Pre-user bootstrap**: the system reaches >=90% pass rate on L0–L4 evals before any human user is invited.

### Non-Goals (v0)

- Live fine-tuning of model weights (in-context learning only; weight updates considered for v2).
- Cross-client real-time gossip (lessons travel via the Foundry, not peer-to-peer).
- Per-user shared lesson libraries beyond Client-local + Foundry-vetted global.
- A public lesson marketplace.
- Auto-promotion to T4 (always human-gated).

---

## 2. The Four Roles

The eval/lesson loop is a tight choreography of four roles. Each is a separate sub-agent (or component) with its own reward signal so they can be iterated independently.

| Role | When it runs | Question it answers | Reward signal |
|---|---|---|---|
| **Critic** | mid-task, after a tool failure or step regression | "Is this fix attempt making progress, or are we looping?" | task progress, novelty of attempt, time budget |
| **Postmortem** | end of task (success or fail) | "What generalizable lesson is in this trace?" | extractability, specificity, novelty vs existing library |
| **Curator / Retriever** | start of any new task or phase, and on tool-call triggers | "Which staged lessons are relevant to *this* context?" | precision of surfacing |
| **Promoter** | periodically + on lesson-use events | "Has this lesson earned its next tier? Has any earned demotion?" | usage * success rate, statistical significance |

**Critic** wants speed (cheap fast model). **Postmortem** wants depth (power model). **Curator** wants accuracy (embedding quality matters more than reasoning). **Promoter** is mostly bookkeeping plus a counterfactual A/B harness.

---

## 3. The Loop (high-level)

```
Task is running
    |
    v
Step fails (tool error, plan diverges, test fails)
    |
    v
Critic: "loop or progress?"
    |
    +--> looping  --> stop, escalate to user / different agent
    |
    +--> progress --> recovery agent tries fix(es)
                          |
                          v
                  Task succeeds (with or without recovery)
                          |
                          v
                  Postmortem: produce lesson candidates
                          |
                          v
                  Scope classifier: project | global candidate
                          |
                          v
                  Stage at T1 (.reactor/lessons/staged/)
                          |
                          v
                  Auto-mint regression test from the failure trace
                          |
                          v
                  [Future task starts]
                          |
                          v
                  Curator surfaces top-K relevant T1+ lessons
                  as ADVISORY context to the agent
                          |
                          v
                  Agent may cite lessons via lesson_cite tool
                          |
                          v
                  Promoter updates ledger: cited + outcome
                          |
                          v
                  Tier transitions:
                    T1 --auto-->     T2 (cited + success >= 1)
                    T2 --auto-->     submission queue (Client)
                                  | counterfactual A/B (Foundry)
                    T3 --human-->   skill bundle / convention
                    T4 --human-->   shipped in lesson pack OR
                                    baked in via selfdev (Foundry only)
```

Two distinct sub-loops live inside this:

1. **Recovery sub-loop** (mid-task): Critic + recovery agent iterate to unblock the current Task. First priority is *finishing the current task*, not learning. Lessons are a side effect.
2. **Improvement sub-loop** (between tasks): Postmortem + Curator + Promoter run on top of completed tasks. This is where measurable improvement compounds.

Keeping these explicitly separate matters because they have different failure modes: the recovery loop can hill-climb into bad workarounds; the improvement loop can entrench bad lessons. Each gets its own safeguards.

---

## 4. Lesson System

A **Lesson** is a versioned, scoped, tiered artifact that modifies agent behavior without changing the harness binary. Lessons are how the system improves itself in the common case; only T4 promotions touch the binary, via the existing `selfdev` ladder.

### 4.1 Lesson Model

```rust
pub struct Lesson {
    pub id: LessonId,
    pub kind: LessonKind,
    pub tier: Tier,
    pub scope: Scope,
    pub title: String,
    pub body: String,                 // freeform markdown
    pub tags: Vec<String>,            // domain, library, error-class, etc.
    pub phases: Vec<PhaseName>,       // which Task phases this is relevant to
    pub triggers: Vec<Trigger>,       // structured retrieval predicates
    pub valid_for: Constraints,       // version ranges (reactor_cli, framework)
    pub embedding: Option<Vec<f32>>,  // for semantic retrieval
    pub origin: Origin,               // postmortem | synthetic | upstream | manual
    pub created: DateTime<Utc>,
    pub updated: DateTime<Utc>,
    pub citations: u64,               // total times cited
    pub successes: u64,               // citations that contributed to success
    pub failures: u64,                // citations that didn't help / hurt
}

pub enum LessonKind {
    PromptDelta { target: PromptTarget, snippet: String },
    Heuristic { when: String, prefer: String, avoid: Option<String> },
    SkillBundle { manifest_path: PathBuf },
    ToolProposal { spec: ToolSpec },         // T4-only path: requires selfdev_build
    AntiPattern { pattern: String, reason: String },
}

pub enum Tier { T0, T1, T2, T3, T4 }

pub enum Scope { Project, GlobalCandidate, Global }

pub enum Origin { Postmortem, Synthetic, Upstream, Manual }

pub struct Trigger {
    pub kind: TriggerKind, // tool_error_signature | phase_start | regex_in_message | ...
    pub value: String,
}
```

The five `LessonKind`s exist because lessons span a huge range of size and shape: from a one-line prompt addendum to a fully-specified new Rust tool. Each kind has its own T4 adoption path (see §4.6).

### 4.2 Tier Ladder

| Tier | Storage | Visibility | Auto-promotion criterion | Demotion criterion |
|---|---|---|---|---|
| T0 Active hypothesis | scratch buffer in active task | injected into current loop only | task succeeds -> T1 | task fails -> discarded |
| T1 Staged | `.reactor/lessons/staged/` | retriever surfaces only when relevance >= threshold | cited + contributed to success >= 1 -> T2 | cited + caused failure >= 1 -> discarded |
| T2 Validated | `.reactor/lessons/validated/` | always surfaced when domain-relevant; ranked higher | (Client) eligible for upstream submission; (Foundry) counterfactual A/B passes -> queue T3 | success_rate < 0.5 over >= 5 citations -> T1 |
| T3 Established | `.reactor/lessons/established/` (project) or `~/.config/reactor-studio/lessons/global/` (global) | always surfaced when domain-relevant; converted into a real skill bundle or convention edit | **human gate** (Foundry only auto-runs the queue; Client T3 = dormant pending Foundry adoption) | recurring demotion signals -> T2 |
| T4 Adopted | merged into `_shared/conventions.md`, default agent prompts, a new skill, or Rust code via `selfdev` | invisible — it is now *how the harness works* | **human gate** | manual rollback only |

**Client cap**: the Client edition's auto-promotion ladder tops out at T2. T3 entries on Clients are submitted upstream and held dormant; only the Foundry produces T3 and T4 changes. Vetted T3+ artifacts return to Clients via signed lesson packs (§6.4).

### 4.3 Scope Classifier

When the postmortem produces a candidate lesson, a small classifier agent assigns scope. Signals:

| Signal | Direction | Notes |
|---|---|---|
| Mentions only generic concepts (`reactor.cloud`, language names, framework idioms) | -> global candidate | strong global signal |
| Mentions specific project paths, file names, repo URLs, secret names | -> project | strong project signal |
| References a pinned library version | tag with version, classifiable as global | |
| Phrasing test: "rewrite this lesson without project-specific terms — does it still make sense?" | binary | rewriting succeeds -> global candidate |

Net positive score -> `GlobalCandidate`. Otherwise -> `Project`.

Scope is **mutable**. When the same lesson (by embedding similarity above a threshold) appears in N independent projects, the Foundry curator merges and migrates it to `Global`. A lesson can also be demoted from `Global` back to `GlobalCandidate` if it stops being useful.

### 4.4 Retrieval

The Curator runs at:

- start of each Task phase (filter by phase, domain)
- before each tool call when registered triggers match (filter by trigger pattern)
- on demand from the agent prompt (the agent may ask "are there lessons about X?")

Ranking score (illustrative, tunable):

```
score = 0.4 * semantic_similarity(query, lesson.embedding)
      + 0.3 * tag_overlap(query.tags, lesson.tags)
      + 0.2 * tier_weight(lesson.tier)            // T3 > T2 > T1
      + 0.1 * recent_success_rate(lesson)
```

Top-K (default 3) get injected into the agent context as **advisory** content:

```
=== Staged guidance (advisory; cite by id if you apply one) ===
[L42 / T2 / domain: auth] When integrating JWT auth on reactor.cloud,
the env var must be added before the route definition to avoid the
cold-start race. See also L88.
=== End advisory ===
```

The framing matters: the agent retains agency. Lessons are *available*, not *authoritative*. This avoids the cargo-cult failure mode where a wrong lesson silently poisons future runs.

**Conflict surfacing**: when two retrieved lessons contradict each other (detected by embedding distance + opposing recommendations), both are surfaced with an explicit nudge: "Lessons L42 and L88 disagree — pick one and explain why in your reasoning."

### 4.5 Citation & Ledger

The agent **must explicitly cite** any lesson it applies via the `lesson_cite` tool:

```
lesson_cite { lesson_id: "L42", reason: "applying advice on env var ordering" }
```

Uncited contributions get no credit. This forces attribution and keeps promotion math honest. (Postmortem will gently nudge the agent if it appears to have applied a retrieved lesson without citing it.)

Ledger format (`.reactor/lessons/ledger.jsonl`, append-only):

```json
{"ts":"2026-05-18T15:01:00Z","lesson":"L42","task":"task_abc","phase":"development","cited":true,"outcome":"success"}
```

The ledger is the single source of truth for promotion math. It is also exported (anonymized) as part of upstream submission's eval-context (§7.3).

### 4.6 T4 Adoption Paths

T4 promotion is the only point where a lesson stops being context and becomes part of the system itself. Each `LessonKind` adopts differently:

| LessonKind | T4 adoption path | Reversibility |
|---|---|---|
| `PromptDelta` | append to the target prompt or `_shared/conventions.md` | trivial (revert the edit) |
| `Heuristic` | add a structured entry to a heuristic registry (`.reactor/heuristics/index.yaml`) loaded by all agents | trivial (remove entry) |
| `SkillBundle` | install in `.reactor/skills/` (or globally) | trivial (uninstall) |
| `AntiPattern` | append to conventions; optionally add a lint rule to `studio-tools` | trivial |
| `ToolProposal` | requires `selfdev_build`: implement the tool, register it, rebuild the harness | requires re-deploy to revert |

Only `ToolProposal` touches the binary. All other T4 adoptions are pure file edits and can be rolled back by removing files or reverting commits.

### 4.7 Demotion

Symmetric to promotion. Triggers:

- T2 lesson with `successes / (successes + failures) < 0.5` over >= 5 cited runs -> back to T1
- T1 lesson uncited for 30 days *and* failures > 0 -> discarded
- Any global lesson regressing the eval suite during nightly re-validation -> Foundry retracts it via the lesson-pack channel; Clients remove it on next sync

This keeps the library from accumulating cruft and protects against silent regressions when the platform changes (reactor.cloud version bumps, library updates, etc.).

### 4.8 Storage Layout

```
.reactor/lessons/
├── staged/
│   └── L42.yaml
├── validated/
│   └── L18.yaml
├── established/
│   └── L07.yaml
├── submissions/
│   └── <submission_id>.yaml          # local receipts for upstream submissions
├── ledger.jsonl
└── index/
    ├── embeddings.bin
    └── tags.json

~/.config/reactor-studio/lessons/
└── global/
    ├── L007.yaml                     # vetted, signed lesson packs from Foundry
    ├── L018.yaml
    └── manifest.signed               # signature + version + applied_at
```

Project-scoped lessons live in `.reactor/`. Globally vetted lessons land in the user-config dir and are shared across all the user's projects.

---

## 5. Eval Framework (Foundry-only)

The eval framework is the loss function over the harness. It runs **only in the Foundry edition**; the Client never compiles the eval crates. This is both a cost decision (evals burn tokens) and an IP decision (the test corpus and rubrics are the "secret sauce" that drives quality).

### 5.1 Test Taxonomy

Eight levels, each subsuming the prior. Higher levels exercise more of the harness and cost more.

| Level | Name | What it exercises | Time/run | Frequency | Example |
|---|---|---|---|---|---|
| L0 | Atomic | one tool call | seconds | every iteration | create file with content |
| L1 | Sequential | 2-5 tool calls, no planning | <1 min | every iteration | init Node project + index.js |
| L2 | Templated scaffold | recipe knowledge for known shapes | 1-3 min | every iteration | init reactor.cloud TS app with /health |
| L3 | Modification | code reading + targeted editing | 3-8 min | per promotion check | add env var FOO, expose at /config |
| L4 | Full Task workflow | the 6-phase pipeline end-to-end | 10-20 min | nightly + pre-T3 | add JWT auth to fixture |
| L5 | Refactor | large coordinated edits | 20-60 min | weekly + pre-T3 | migrate REST -> tRPC |
| L6 | Debug / repair | broken project + critic loop | 10-30 min | nightly | fix this failing deploy |
| L7 | Open-ended build | full project from spec | 1-4 h | weekly + pre-T4 | build multi-tenant SaaS for X |

Cost tiers:

- **Smoke** (L0-L2) — runs on every iteration of the auto-loop.
- **Standard** (L3-L4) — runs per staged-lesson promotion check and nightly.
- **Full** (L5-L7) — runs weekly and before any T3->T4 promotion gate.

The full suite is never run on every iteration. Token economics make it impossible.

### 5.2 Test Shape

```yaml
id: l3.add-env-var-001
level: L3
domain: [env, reactor.cloud]
phases: [development]
fixture:
  kind: git
  repo: foundry/fixtures/reactor-app-skeleton
  ref: v0.1.0
instruction: |
  Add an environment variable `FOO` to this reactor.cloud app and expose
  it via a `/config` route that returns `{ foo: process.env.FOO }`.
budget:
  max_tool_calls: 30
  max_tokens: 50000
  max_wallclock_secs: 300
success:
  - { kind: file_exists, path: ".env.example", contains: "FOO=" }
  - { kind: regex_in, file: "src/routes/config.ts", pattern: "process\\.env\\.FOO" }
  - { kind: command_exit, cmd: "npm run typecheck", exit_code: 0 }
runs_required: 5
pass_threshold: 0.8
model_pin:
  provider: openrouter
  model: anthropic/claude-sonnet-4.6
  temperature: 0.2
```

### 5.3 Scorer Hierarchy

In priority order — always prefer the cheapest scorer that's adequate:

1. **Deterministic** — `file_exists`, `regex_in`, `command_exit`, `git_diff_matches`, `cargo_check`. Cheap, reproducible, no LLM in the loop.
2. **Structural** — AST-level checks via the existing `studio-tools/lsp` bridge. "Function with this signature exists", "no unused imports", "this type is exported".
3. **LLM judge** — separate model + rubric. Highest variance; only used for L4+ where deterministic scoring is infeasible. Always paired with at least one deterministic scorer to anchor the result.

Scorers within a test are AND'd together by default; OR groups are available via nested syntax.

### 5.4 Isolation & Parallelism

- Each run gets a **fresh `git worktree`** under `foundry/.runs/<run_id>/`. Cleaned on success, retained on failure for debugging.
- Tests within the same level run **in parallel** (configurable concurrency, default 4).
- Stochastic scoring: each test runs `runs_required` times; pass requires `pass_threshold` rate.
- **Model + version + temperature pinned per test** for reproducibility across iterations. A model swap is itself a change that requires re-baselining the suite.

### 5.5 Replay Mode

For iterating on harness internals (`studio-agent`, `studio-tools`, etc.) without re-spending tokens:

1. **Record**: a real run captures all LLM request/response pairs to `foundry/.replays/<test_id>/<run_seed>.jsonl`.
2. **Replay**: subsequent runs serve responses from the cassette instead of hitting the provider.
3. **Cassette miss**: if the harness sends a request not in the cassette (because it's behaving differently), the replay fails with a "cassette miss". That itself is signal — the harness change altered the request distribution.

Replay is **not** valid for evaluating prompt or lesson changes (those alter the request distribution by design). Use it only for harness-deterministic iteration.

### 5.6 Suite Lives in a Separate Private Repo

The test corpus, fixtures, and rubrics are the Foundry's most valuable IP. They live in a separate private repository (`reactor-foundry-suite/`) pulled in as a workspace dependency only by Foundry builds. Client builds cannot reference these crates at all — they aren't even in the dependency graph.

```
reactor-foundry-suite/             # PRIVATE, separate repo
├── tests/
│   ├── L0/ ... L7/
├── fixtures/                      # git submodules or sealed tarballs
├── rubrics/                       # LLM-judge rubrics
└── Cargo.toml                     # workspace dep, foundry-only
```

### 5.7 Auto-Mint of Regression Tests

Every novel failure surfaced anywhere — eval suite, synthetic project run, dogfood, user submission — gets minted as a permanent test at the appropriate level. The minting agent:

1. Reads the failure trace
2. Classifies its level (L0-L7) by tool-call depth, phase coverage, fixture state
3. Generates a deterministic scorer where possible (the failure's resolution becomes the success criterion)
4. Files it under `reactor-foundry-suite/tests/L<n>/auto-minted/<id>.yaml` with provenance tags

Auto-minted tests are reviewed periodically by maintainers; some get promoted to "canonical" and edited for clarity, others get pruned if they're flaky or duplicative.

---

## 6. Foundry / Client Split

Same harness, two editions. The Client edition runs on every user's machine. The Foundry edition runs internally and is the single source of truth for canonical improvements.

### 6.1 Editions at a Glance

| Capability | Client | Foundry |
|---|---|---|
| Full Tauri app, agent loop, tools, tasks, MCP | yes | yes |
| Selfdev (build/launch/reload) | yes (gated) | yes |
| Lesson tiers T0-T2 (auto) | yes | yes |
| Lesson tier T3 (human review) | submit upstream only | full pipeline |
| Lesson tier T4 (bake into release) | no | yes |
| Eval suite + fixtures + scorers | no | yes |
| Synthetic project generator | no | yes |
| Curator service (intake of submissions) | no | yes |
| Lesson pack publisher | no | yes |
| Receives signed lesson packs (OTA) | yes | yes (echo for testing) |
| Submits candidate lessons upstream | yes (opt-in) | yes (echo for testing) |

### 6.2 Crate Matrix

| Crate | Client builds | Foundry builds | Notes |
|---|---|---|---|
| `studio-agent`, `studio-tools`, `studio-providers`, `studio-protocol`, `studio-storage`, `studio-skill`, `studio-task`, `studio-memory`, `studio-plan`, `studio-compaction`, `studio-cloud` | yes | yes | shared core (per `reactor-studio_design.md`) |
| `studio-lessons` | yes | yes | shared; tier behavior driven by feature flag |
| `studio-postmortem` | yes | yes | shared |
| `studio-promotion` (`feature = "client"`) | yes | no | caps at T2 -> submission |
| `studio-promotion` (`feature = "foundry"`) | no | yes | full pipeline incl. counterfactual A/B |
| `studio-uplink` | yes | yes | submits candidate lessons upstream; Foundry uses for echo testing |
| `studio-eval` | no | yes | eval runner, scorers, fixture management, replay |
| `foundry-eval-suite` (separate private repo) | no | yes | the test corpus + rubrics + fixtures |
| `foundry-curator` | no | yes | submission intake service |
| `foundry-orchestrator` | no | yes | fleet management for synthetic-project workers |
| `foundry-publisher` | no | yes | signs and publishes lesson packs |

Cargo features at the workspace level:

- `client` (default) — builds Client edition
- `foundry` — builds Foundry edition; pulls in `studio-eval`, `foundry-*`, `reactor-foundry-suite`
- `eval` — implied by `foundry`; can be enabled standalone for harness-developer use of the eval runner without the rest of the Foundry plumbing

### 6.3 Workspace Layout

The repo gains a `foundry/` peer to `studio/`. Under the existing eviction model (`studio/` is destined for its own repo), the Foundry stays behind in this monorepo (or eventually moves to `reactor-foundry/` separately).

```
Reactor/
├── studio/                        # Client edition (per reactor-studio_design.md)
│   ├── apps/studio/
│   └── crates/studio-*
└── foundry/                       # Foundry edition (NEW)
    ├── apps/
    │   ├── console/               # Tauri or web maintainer dashboard
    │   │   ├── src/               # eval results, lesson queue, A/B status
    │   │   └── src-tauri/
    │   └── worker/                # headless: synthetic project loop, curator service
    │       └── src/main.rs
    ├── crates/
    │   ├── studio-eval/           # eval runner + scorers + replay
    │   ├── foundry-curator/       # submission intake, redaction, queue
    │   ├── foundry-orchestrator/  # fleet for synthetic projects + dogfood
    │   └── foundry-publisher/     # signed lesson pack publishing
    ├── fixtures/                  # public starter fixtures (the private suite is separate)
    └── .runs/                     # eval run worktrees (gitignored)
```

### 6.4 Lesson Pack Delivery (OTA)

Vetted T3+ lessons reach Clients via **signed lesson packs**, a delivery channel separate from app updates.

- Pack format: signed tarball of `LessonPack { manifest, lessons[], retractions[], signature }`.
- Manifest pins `min_studio_version` and `valid_for: { reactor_cli, framework, region }` constraints.
- Client checks for new packs daily (configurable; on-demand via "Check for updates"). Applied without restart — they go into `~/.config/reactor-studio/lessons/global/`.
- Signature verified against an embedded Foundry public key; tampered packs refused.
- **Retraction list**: any pack can include retraction entries (`retract: [L007, L042]`) that remove previously-published lessons. Clients honor these on next sync. This gives the Foundry a same-day kill switch for any lesson that turns out to regress in production.

This decouples lesson cadence (hours to days) from app release cadence (weeks).

### 6.5 Where Updates Go

| Update type | Channel | Cadence |
|---|---|---|
| New harness binary, new tools (Rust), bug fixes | App update (Tauri updater) | weeks |
| New vetted lesson, retraction, lesson edit | Signed lesson pack (OTA) | hours to days |
| New skill bundle (T4 from `SkillBundle` lesson) | Signed lesson pack — included as a bundled artifact | hours to days |
| New convention text (`_shared/conventions.md` edit) | Signed lesson pack — packaged as a `PromptDelta` or as a file diff | hours to days |
| New default agent prompt | App update (it's a baked-in resource) | weeks |
| New test in eval suite | Foundry-only; never ships | continuous |

The split keeps Clients safe (no surprise binary changes from auto-promotion) while letting good lessons reach users fast.

---

## 7. Submission Pipeline

How a candidate lesson travels from a user's Client to the Foundry's vetted global library.

### 7.1 Client-Side Flow

```
T2 lesson reaches submission-eligible threshold
    |
    v
Redactor pass:
  - secrets blocklist (regex: AWS, OpenAI, GitHub, Stripe, etc.)
  - high-entropy string detection (likely keys)
  - project identifier tokenization (paths -> <project>/<file>)
  - PII heuristics (email, phone, address-shaped)
  - code-sample scrub (replace business-logic constants with placeholders)
    |
    v
Preview UI (modal):
  - shows EXACT bytes that will leave the machine
  - inline edit / redact / cancel
  - per-submission, never blanket
  - choice persisted: "always", "never", "ask each time"
  - attribution: anonymous | handle
    |
    v
Sign + envelope
    |
    v
HTTPS POST -> foundry-curator endpoint
    |
    v
Receipt stored in .reactor/lessons/submissions/<id>.yaml
  status: queued | accepted | rejected | merged-into:<other>
  polled periodically; user notified on status change
```

**Default consent**: opt-in per submission with preview. No blanket auto-submit. The user can choose "always submit redacted" but it is off by default. Per-project opt-out always available. Global opt-out (never submit anything from this machine) available in Settings.

### 7.2 Foundry-Side Flow

```
HTTPS endpoint (foundry-curator)
    |
    v
Signature + replay-protection check
    |
    v
Quarantine inbox (foundry/.intake/<id>.yaml)
    |
    v
Re-redaction sanity scan (defense in depth)
  any leaked secret -> reject + alert client_id
    |
    v
Near-duplicate clustering
  embedding similarity vs existing library
  if similarity > 0.92 -> merge into existing as +1 evidence (boost promotion)
  else -> new candidate
    |
    v
Eval gate (run candidate against suite):
  must improve >= 1 test (statistically significant over N runs)
  must NOT regress any test (hard fail)
  results recorded with the candidate
    |
    v
Human review queue (Foundry console UI):
  approve as-is
  approve with edits
  reject (with reason; optional reply to submitter)
  merge into existing
    |
    v
Cohort A/B rollout:
  publish to 10% of fleet (random by client_id hash)
  observe for M days for regression signal
  if clean -> publish to 100%
    |
    v
Signed lesson pack published; submitter notified of acceptance
```

### 7.3 Wire Format

```json
{
  "version": 1,
  "client_id": "c_a1b2c3d4...",
  "studio_version": "0.4.2",
  "harness_revision": "0001f4a",
  "submitted_at": "2026-05-18T15:01:00Z",
  "lesson": { "...": "LessonModel" },
  "eval_context": {
    "phase": "development",
    "failure_signature": "TS2304:Cannot find name 'process'",
    "recovery_steps": 3,
    "trace_excerpt": "...redacted last 200 lines..."
  },
  "attribution": { "kind": "handle", "value": "@cdelconde" },
  "signature": "ed25519:..."
}
```

JSON for v1; protobuf considered later if volume warrants.

### 7.4 Privacy & Trust

| Concern | Mitigation |
|---|---|
| Secrets leakage | Multi-pass redaction (client + foundry); high-entropy heuristics; explicit blocklist; user preview gate |
| Adversarial submissions | Eval gate (must not regress any test); reputation scoring per `client_id`; rate limits |
| PII | Redactor flags email/phone/address shapes; rejected with feedback if found |
| Replay attacks | Signed envelopes with timestamps; nonce + replay window (24h) |
| Client_id linkability | Rotatable; can be regenerated per project or per submission; never derived from user identity unless attribution is set |
| Right to be forgotten | Curator supports retraction by `client_id` OR by `payload_hash` |
| Customer code in trace excerpts | Excerpts capped at N lines; redactor scrubs string literals more aggressively in traces than in lesson bodies |

### 7.5 Transport & Auth

The submission endpoint piggybacks on existing reactor.cloud auth infrastructure (assumption — to be confirmed in Open Questions). Anonymous submissions are allowed but rate-limited harder; signed-in submissions get higher rate limits and feedback delivery (status changes appear in the user's `.reactor/lessons/submissions/` directory and as a Studio notification).

The endpoint shape (illustrative):

```
POST   https://api.reactor.cloud/foundry/v1/submissions
GET    https://api.reactor.cloud/foundry/v1/submissions/<id>
GET    https://api.reactor.cloud/foundry/v1/lesson-packs/manifest      # client polls this
GET    https://api.reactor.cloud/foundry/v1/lesson-packs/<version>     # signed pack tarball
```

Foundry-side these route to the `foundry-curator` (intake) and `foundry-publisher` (pack delivery) services respectively.

### 7.6 Status Lifecycle (per submission)

```
queued        -> dedup -> {merged-into:<other>}
              -> dedup -> quarantine
quarantine    -> re-redaction -> {rejected:secret-leak}
              -> re-redaction -> eval-gate
eval-gate     -> {rejected:regression}
              -> {rejected:no-improvement}
              -> review-queue
review-queue  -> {rejected:<reason>}
              -> approved
approved      -> cohort-10
cohort-10     -> {rolled-back:<reason>}
              -> cohort-100
cohort-100    -> published
```

The submitter sees the same lifecycle in their local receipt file, with redacted reasons where appropriate.

---

## 8. Auto-Iteration & Bootstrap

The protocol that takes the harness from "promising idea" to "actually performant" before user beta.

### 8.1 The Loop

```
for iteration in 1..N:
    # baseline pass
    results_before = run_suite(level_filter = smoke + standard, parallel = true)
    failures = results_before.failures()

    # per-failure recovery + postmortem
    for failure in failures:
        fix = recovery_agent(failure)        # critic prevents loops
        if fix.succeeded:
            lesson = postmortem_agent(failure, fix)
            scope = scope_classifier(lesson)
            stage(lesson, tier = T1, scope)
        auto_mint_regression_test(failure)   # failure becomes a permanent eval

    # reevaluate with staged lessons available
    results_after = run_suite(level_filter = same)

    # promotion judging
    for lesson in T1_lessons:
        if cited and contributed_to_pass:
            promote(lesson, T2)
    for lesson in T2_lessons:
        if counterfactual_AB_shows_improvement:
            queue_for_human_review(T3)

    # rollback gate
    if results_after.pass_rate <= results_before.pass_rate:
        rollback_recent_lessons()
```

The Foundry runs this continuously across a fleet of synthetic-project workers (managed by `foundry-orchestrator`). The rollback gate is critical: it prevents the loop from hill-climbing into a worse local minimum.

### 8.2 Starter Corpus (L0-L4, ~30 tests)

Seeded by hand to bootstrap auto-iteration. Auto-minting from failures grows the corpus thereafter.

**L0 — fs primitives (5 tests)**
- create file with given content
- read existing file
- edit-replace single line
- delete file
- list directory

**L1 — multi-step plumbing (5 tests)**
- create folder with N referenced files
- run shell command and inspect stdout
- search-then-edit (grep -> patch)
- glob-then-batch-read
- find-then-rename across files

**L2 — reactor.cloud scaffolds (8 tests)**
- init blank reactor app
- init with `/health` route
- init with one DB table
- init with one Job
- init with auth stub
- init with file storage
- init with cron
- init with env var

**L3 — modifications on small fixture repos (8 tests)**
- add env var
- add a route
- add a DB column
- add a Job
- fix a deliberate type error
- rename a function across files
- add a missing import
- refactor a single function

**L4 — full Task pipeline (4 tests)**
- "add JWT auth"
- "add a CRUD resource"
- "add caching to one endpoint"
- "add a deploy preview workflow"

Fixtures live in `foundry/fixtures/` for the public starter set, and as git submodules in the private `reactor-foundry-suite/` for proprietary fixtures.

### 8.3 Gating Thresholds

| Step | Gate |
|---|---|
| Add L3 to standard tier | L0-L2 >= 95% pass rate |
| Add L4 to standard tier | L3 >= 90% pass rate |
| Add L5-L6 to full tier | L4 >= 80% pass rate |
| Open user private alpha | L0-L4 >= 85% pass rate |
| Open user public beta | L0-L4 >= 90% pass rate, submission privacy proven |
| Add L7 to full tier | L5-L6 >= 70% pass rate |

Pass rate is computed over the most recent full run of that level.

### 8.4 Foundry's Autonomous Side

Beyond the eval loop, the Foundry also runs:

- **Synthetic project generator**: foundry agents build real reactor.cloud apps from prompt templates, deploy them, exercise them. Generates failure modes that pure unit-style evals miss. Each synthetic project run is a candidate L4-L7 eval if it surfaces something interesting.
- **Real flagship apps (dogfooding)**: 2-3 real apps the team builds *on* reactor.cloud themselves, driven through Studio Foundry. Highest-quality lesson source because the failures are real. Whatever your reactor.cloud team is building anyway becomes the dogfood set.
- **Cohort replay**: when an interesting candidate lesson lands, replay it against historical synthetic-project runs (using replay-mode cassettes where possible) to see if it would have helped past failures. Cheap and informative.
- **Auto-mint regression tests**: every novel failure surfaced anywhere — eval suite, synthetic, dogfood, user submission — gets minted as a permanent test at the appropriate level (§5.7).

### 8.5 Bootstrap Timeline

This is the pre-user-beta phase. Expected duration: weeks to months depending on iteration cadence and budget.

| Phase | Activity | Exit |
|---|---|---|
| F0 — Eval skeleton | Stand up `studio-eval`, scorers, runner; implement L0-L2 starter; baseline pass rate | suite runs end-to-end |
| F1 — Loop online | Auto-iteration loop functional; `studio-postmortem`, scope classifier, `studio-promotion` (foundry profile) shipping lessons; replay mode functional | first lesson promoted T1->T2 by the loop |
| F2 — Higher levels | Add L3, then L4 to the standard tier; expand starter corpus; start dogfooding flagship apps | L0-L2 >= 95% |
| F3 — Submission pipeline | `studio-uplink`, redactor, preview UI ready (still no real Clients); `foundry-curator` ready; round-trip echo testing (Foundry submits to itself and verifies the full pipeline) | self-submission echo round-trip works end-to-end |
| F4 — Pack publishing | `foundry-publisher`; signed packs; client OTA fetcher in `studio-lessons` | pack -> client -> retrieval working in dogfood |
| F5 — Private alpha | First real Clients (small cohort); real submissions begin; cohort A/B rollout active; rollback proven | L0-L4 >= 85%, lesson pack channel proven over real traffic |
| F6 — Public beta | Open the door | L0-L4 >= 90%; submission privacy proven; rollback latency proven |

The Client-edition phased rollout (per `reactor-studio_design.md` Phase 0-6) runs in parallel. F0-F2 can begin as soon as Studio Phase 1 (agent core ported) is available. F3 depends on Studio Phase 5 (selfdev + diff view, which provides the redaction preview UI primitives). F5 is gated on Studio Phase 6 (polish) being beta-quality.

### 8.6 Cost Notes

The Foundry will burn meaningful tokens. Mitigations:

- **Replay mode** for harness-internal iteration (free).
- **Smoke-tier on every iteration**, standard nightly, full weekly.
- **Pinned model + temperature** per test — avoids accidental cost regressions from model upgrades.
- **Statistical pass thresholds** rather than runs-until-success — caps cost per test.
- **Cassette pruning**: replay cassettes get deleted after N days unless tagged as canonical.
- **Cheap critic, expensive postmortem**: the high-frequency critic uses a fast/cheap model; postmortem (low frequency) uses the power model.

A rough budget for Foundry F0-F2 should be assumed in the 4-5 figure monthly USD range during active iteration; this falls dramatically once pass rates plateau and the loop settles.

---

## 9. IPC & Service Surfaces

This section enumerates the new commands and events not already covered by `reactor-studio_design.md` §8.

### 9.1 Client (Studio) Tauri Commands (additive)

| Command | Args | Returns | Notes |
|---|---|---|---|
| `lessons.list` | `{ scope?, tier? }` | `Lesson[]` | for a Lessons sidebar / inspector view |
| `lessons.get` | `{ lessonId }` | `Lesson` | full body + ledger entries |
| `lessons.cite` | `{ lessonId, taskId, phase, reason }` | `void` | called by the agent's `lesson_cite` tool |
| `lessons.preview-submission` | `{ lessonId }` | `RedactedLesson` | runs redactor; returns exact bytes that would be sent |
| `lessons.submit` | `{ lessonId, attribution }` | `{ submissionId }` | sends to Foundry endpoint |
| `lessons.submission-status` | `{ submissionId }` | `SubmissionStatus` | polls receipt |
| `lessons.consent` | `{ scope, choice }` | `void` | per-project / global "always / never / ask" |
| `lessons.fetch-pack` | `{}` | `{ applied: Lesson[], retracted: LessonId[] }` | manual "Check for updates" |

### 9.2 Client Events (additive)

| Event | Payload | Notes |
|---|---|---|
| `lesson:retrieved` | `{ taskId, phase, lessons: Lesson[] }` | curator surfaced these for the agent |
| `lesson:cited` | `{ lessonId, taskId, phase }` | for UI "applied" badges |
| `lesson:promoted` | `{ lessonId, from, to }` | tier transition |
| `lesson:submission-status` | `{ submissionId, status }` | polled status changed |
| `lesson:pack-applied` | `{ added: LessonId[], retracted: LessonId[] }` | OTA pack just landed |

### 9.3 Foundry HTTP API (curator + publisher)

| Method | Path | Purpose |
|---|---|---|
| `POST` | `/foundry/v1/submissions` | Client submits a candidate lesson |
| `GET` | `/foundry/v1/submissions/<id>` | Client polls status |
| `GET` | `/foundry/v1/lesson-packs/manifest` | Client polls for pack updates |
| `GET` | `/foundry/v1/lesson-packs/<version>` | Client downloads signed pack tarball |
| `GET` | `/foundry/v1/health` | liveness |

Foundry-internal endpoints (console + worker only, not exposed):

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/internal/queue` | review queue |
| `POST` | `/internal/lesson/<id>/approve` | approve as-is |
| `POST` | `/internal/lesson/<id>/edit` | approve with edits |
| `POST` | `/internal/lesson/<id>/reject` | reject with reason |
| `POST` | `/internal/lesson/<id>/cohort` | manual cohort transition |
| `POST` | `/internal/eval/run` | enqueue an eval-suite run |

---

## 10. Open Questions

1. **Submission default consent**: opt-in per submission with preview (proposed default), or auto-submit-redacted off-by-default toggle? Privacy posture call.
2. **Submission transport**: piggyback on existing reactor.cloud auth + a `/foundry/v1/...` endpoint set (proposed default; minimal new infra), or stand up a dedicated foundry endpoint with its own auth model?
3. **Anonymous vs signed-in submissions**: allow both with rate-limit asymmetry (proposed), or require sign-in? Anonymous is friendlier; signed-in gives feedback delivery.
4. **Lesson pack signing key rotation**: bake a single public key into Clients, or ship an embedded list of pinned keys plus a rotation mechanism? Single key is simpler; rotation matters for long-lived deployments.
5. **Counterfactual A/B sample size**: how many runs are required to call a T2->T3 promotion statistically significant? Affects cost and cycle time; need to pick a default and tune.
6. **Reputation scoring per `client_id`**: yes/no in v0? Adds anti-abuse but also surveillance optics. (Suggest: simple rate limits + outlier detection in v0; scoring in v1.)
7. **Ledger retention**: keep `ledger.jsonl` forever, or rotate / compact periodically? Forever is simplest but unbounded.
8. **Embedding model**: which embedding model for retrieval? Self-hosted small model vs hosted API. Affects latency, cost, and Client-side dependency surface.
9. **Replay cassette format**: roll our own JSONL or adopt VCR-style cassettes (e.g. `vcr-cassette` crate)? Standard formats help if we ever open-source the eval framework.
10. **Synthetic project generator scope**: how broad? Just CRUD apps, or include data pipelines, ML serving, etc.? Affects the diversity of failure modes the Foundry sees pre-beta.
11. **Foundry console runtime**: Tauri app (consistent with Studio) or web (easier to give multiple maintainers access)? The worker is headless either way.
12. **Pre-T4 human review UX**: what does "approve with edits" look like? Markdown diff editor is the obvious answer; needs review against typical lesson sizes.

---

## 11. Glossary

- **Lesson** — a tiered, scoped behavioral artifact that modifies agent behavior without changing the harness binary (in tiers T0-T3) or with a reversible binary edit (T4 `ToolProposal`).
- **Tier** — a position on the lesson promotion ladder: T0 (active), T1 (staged), T2 (validated), T3 (established), T4 (adopted).
- **Scope** — `Project`, `GlobalCandidate`, or `Global`. Mutable; promoted by cross-project recurrence.
- **Citation** — explicit reference of a lesson by the agent via the `lesson_cite` tool. Required for promotion math.
- **Critic** — mid-task agent that judges progress vs looping.
- **Postmortem** — end-of-task agent that distills lesson candidates from the trace.
- **Curator** — runtime component that retrieves and ranks lessons for injection as advisory context.
- **Promoter** — bookkeeping component that walks the ledger and applies tier transitions.
- **Ledger** — append-only `(lesson, task, phase, cited, outcome)` log; source of truth for promotion math.
- **Client** — the user-facing edition of Reactor Studio. Caps lesson promotion at T2.
- **Foundry** — the maintainer-only edition. Runs the eval suite, the synthetic project loop, the curator service, and produces lesson packs.
- **Eval suite** — the test corpus + scorers + fixtures; lives in a separate private repo.
- **Eval level** — one of L0 through L7, ordered by cost and ambition.
- **Replay mode** — re-running a test with cached LLM responses to evaluate harness changes without spending tokens.
- **Lesson pack** — a signed, versioned bundle of lessons (and retractions) delivered to Clients OTA.
- **Auto-mint** — the practice of converting a novel failure into a permanent regression eval.
- **Counterfactual A/B** — running the eval suite with a candidate lesson visible vs hidden, to measure its true contribution.

---

*Document version: 0.1*
*Last updated: 2026-05-19*
