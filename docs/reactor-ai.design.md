# `reactor-ai` — Design Doc

**Status:** Draft v0, May 2026
**Scope:** AI inference capability for the Reactor.cloud BaaS. Provides OpenAI-compatible LLM dispatch with provider abstraction and usage metering.
**Reader:** Whoever (human or agent) is about to build, extend, or consume this crate.

This document describes *contracts* — HTTP surface, provider dispatch, model registry, usage metering — not implementation. Code lands in follow-up PRs against this doc.

---

## 1. Goals

1. Expose an **OpenAI-compatible HTTP surface** for chat completions and embeddings (`/ai/v1/chat/completions`, `/ai/v1/embeddings`, `/ai/v1/models`).
2. Be **provider-portable**: support OpenRouter, Amazon Bedrock (with SigV4), Azure Foundry, and generic OpenAI-compatible APIs behind a single `ChatProvider` trait.
3. Provide a **model registry** with built-in defaults and per-project overlays, supporting routing aliases (e.g., `reasoning/cheapest`).
4. **Emit usage events** to `reactor-analytics` for token/cost tracking, keyed by user ID from `reactor-auth` JWT claims.
5. Leave **extension seams** for `reactor.cloud` to inject billing, quota enforcement, and region-pinned routing.

## 2. Non-goals (v0)

- **Credit ledger, billing, Stripe integration** — Cloud-only, consumes usage events from analytics.
- **Per-user quota enforcement, low-balance alerts** — Cloud-only policy plugin.
- **EU/US region pinning, cross-region proxy** — Cloud-only middleware.
- **Per-region API keys** — Cloud-only key scope.
- **Prompt caching** — Future enhancement via `reactor-cache`.
- **End-user database table** — User identity comes from `reactor-auth` JWT claims (`claims.sub`).

## 3. Crate layout

```
crates/
├── reactor-ai/                    # library — mirrors reactor-storage
│   ├── Cargo.toml                 # features: bedrock, openrouter, foundry, openai-compatible
│   ├── migrations/                # (empty for v0 — usage flows to reactor-analytics)
│   └── src/
│       ├── lib.rs                 # re-exports, ApiDoc, VERSION, router()
│       ├── config.rs              # AiConfig (provider creds, registry path/url)
│       ├── error.rs               # AiError
│       ├── state.rs               # AiState (registry, dispatch clients, extensions)
│       ├── router.rs              # axum Router::new(state) factory
│       ├── middleware.rs          # Auth middleware → AiCtx
│       │
│       ├── registry/
│       │   ├── mod.rs             # Registry, Alias, Model, Provider, ResolveStrategy
│       │   ├── defaults.toml      # shipped default models and aliases
│       │   └── overlay.rs         # merge project-level overrides
│       │
│       ├── dispatch/
│       │   ├── mod.rs             # ChatProvider trait + shared types
│       │   ├── bedrock.rs         # AWS Bedrock with SigV4 signing
│       │   ├── openrouter.rs      # OpenRouter API
│       │   ├── foundry.rs         # Azure Foundry
│       │   └── openai_compatible.rs  # Generic OpenAI-compatible (Together, Groq, etc.)
│       │
│       ├── routes/
│       │   ├── mod.rs
│       │   ├── health.rs          # GET /ai/v1/health
│       │   ├── chat.rs            # POST /ai/v1/chat/completions
│       │   ├── embeddings.rs      # POST /ai/v1/embeddings
│       │   └── models.rs          # GET /ai/v1/models
│       │
│       ├── store/
│       │   └── usage.rs           # UsageEvent → reactor-analytics
│       │
│       └── ext/
│           ├── mod.rs             # AiExtensions trait (quota, routing override)
│           └── noop.rs            # default no-op implementation
│
└── reactor-ai-server/             # standalone binary
    └── src/
        ├── main.rs                # env config + axum boot
        └── doctor.rs              # health checks (registry, provider reachability)
```

Conventions:
- `reactor-ai` depends on `reactor-core` (for `ReactorId`, `AuthClient`, `AuthCtx`) and optionally on `reactor-analytics` (for usage event emission).
- `reactor-ai` **never** depends on `reactor-auth`. Auth is consumed through the `AuthClient` trait.
- Provider adapters are feature-gated: `bedrock`, `openrouter`, `foundry`, `openai-compatible`.

---

## 4. Core types

### 4.1 `ChatProvider` trait

```rust
#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn chat_completion(
        &self,
        req: &ChatCompletionRequest,
        upstream_model: &str,
    ) -> Result<(ChatCompletionResponse, Duration), AiError>;

    async fn chat_completion_stream(
        &self,
        req: &ChatCompletionRequest,
        upstream_model: &str,
    ) -> Result<(Pin<Box<dyn Stream<Item = Result<ChatCompletionChunk, AiError>> + Send>>, Instant), AiError>;

    fn name(&self) -> &'static str;
}
```

### 4.2 `AiCtx` (request-local)

Constructed by middleware from JWT claims:

```rust
pub struct AiCtx {
    pub auth: Option<AuthCtx>,
    pub request_id: String,
}

impl AiCtx {
    pub fn user_id(&self) -> Option<String> {
        self.auth.as_ref().map(|a| a.claims.sub.clone())
    }
}
```

### 4.3 `Registry`

```rust
pub struct Registry {
    pub models: HashMap<String, Model>,
    pub aliases: HashMap<String, Alias>,
}

pub struct Model {
    pub id: String,
    pub provider: Provider,
    pub upstream_id: String,
    pub capabilities: Vec<Capability>,
    pub context_window: u32,
    pub input_price_per_mtok: Option<f64>,
    pub output_price_per_mtok: Option<f64>,
}

pub struct Alias {
    pub id: String,
    pub strategy: ResolveStrategy,
    pub targets: Vec<String>,
}

pub enum ResolveStrategy {
    First,
    Random,
    RoundRobin,
    Cheapest,
    Fastest,
}
```

---

## 5. HTTP surface (v0)

### 5.1 Health

```
GET    /ai/v1/health
       → 200 { "status": "ok", "version": "0.1.0", "providers": ["openrouter", "bedrock"] }
```

### 5.2 Chat completions

```
POST   /ai/v1/chat/completions
       Body: OpenAI-compatible ChatCompletionRequest
       Headers: Authorization: Bearer <jwt>
       → 200 (non-streaming) or text/event-stream (streaming)
       
Request body:
{
  "model": "gpt-4" | "reasoning/cheapest" | <alias>,
  "messages": [
    { "role": "system", "content": "..." },
    { "role": "user", "content": "..." }
  ],
  "temperature": 0.7,
  "max_tokens": 1024,
  "stream": false
}

Response (non-streaming):
{
  "id": "chatcmpl-...",
  "object": "chat.completion",
  "created": 1234567890,
  "model": "gpt-4",
  "choices": [{
    "index": 0,
    "message": { "role": "assistant", "content": "..." },
    "finish_reason": "stop"
  }],
  "usage": {
    "prompt_tokens": 10,
    "completion_tokens": 50,
    "total_tokens": 60
  }
}

Response (streaming):
data: {"id":"chatcmpl-...","object":"chat.completion.chunk","choices":[{"delta":{"content":"Hello"}}]}
data: {"id":"chatcmpl-...","object":"chat.completion.chunk","choices":[{"delta":{"content":" world"}}]}
data: [DONE]
```

### 5.3 Embeddings

```
POST   /ai/v1/embeddings
       Body: OpenAI-compatible EmbeddingRequest
       Headers: Authorization: Bearer <jwt>
       → 200

Request body:
{
  "model": "text-embedding-3-small",
  "input": "Hello world" | ["Hello", "World"],
  "encoding_format": "float"
}

Response:
{
  "object": "list",
  "data": [{
    "index": 0,
    "object": "embedding",
    "embedding": [0.1, 0.2, ...]
  }],
  "model": "text-embedding-3-small",
  "usage": { "prompt_tokens": 2, "total_tokens": 2 }
}
```

### 5.4 Models

```
GET    /ai/v1/models
       → 200 { "object": "list", "data": [{ model }, ...] }

Model object:
{
  "id": "gpt-4",
  "object": "model",
  "created": 1234567890,
  "owned_by": "openrouter"
}
```

### 5.5 Error envelope

```json
{
  "error": {
    "code": "model_not_found",
    "message": "Model 'gpt-5' not found in registry.",
    "status": 404,
    "request_id": "req_01HZ..."
  }
}
```

Error codes: `model_not_found`, `provider_error`, `rate_limited`, `invalid_request`, `unauthorized`, `quota_exceeded` (Cloud-only).

---

## 6. Provider dispatch

### 6.1 OpenRouter

- **Endpoint**: `https://openrouter.ai/api/v1/chat/completions`
- **Auth**: `Authorization: Bearer {OPENROUTER_API_KEY}`
- **Streaming**: SSE with `data: {...}` format
- **Model mapping**: Pass `model` field as-is

### 6.2 Amazon Bedrock

- **Endpoint**: Regional `https://bedrock-runtime.{region}.amazonaws.com`
- **Auth**: AWS SigV4 signing
- **Streaming**: Uses Bedrock's response streaming API
- **Model mapping**: `bedrock/{model}` → Bedrock model ID

### 6.3 Azure Foundry

- **Endpoint**: `https://{resource}.openai.azure.com/openai/deployments/{deployment}`
- **Auth**: `api-key: {AZURE_FOUNDRY_API_KEY}`
- **Streaming**: SSE with `data: {...}` format
- **Model mapping**: Deployment name from registry

### 6.4 OpenAI-compatible

- **Endpoint**: Configurable base URL
- **Auth**: `Authorization: Bearer {API_KEY}`
- **Examples**: Together AI, Groq, Fireworks, self-hosted vLLM

---

## 7. Model registry

### 7.1 Built-in defaults (`defaults.toml`)

```toml
[models.gpt-4]
provider = "openrouter"
upstream_id = "openai/gpt-4"
capabilities = ["chat", "reasoning"]
context_window = 128000
input_price_per_mtok = 10.0
output_price_per_mtok = 30.0

[models.claude-3-5-sonnet]
provider = "openrouter"
upstream_id = "anthropic/claude-3.5-sonnet"
capabilities = ["chat", "reasoning", "vision"]
context_window = 200000
input_price_per_mtok = 3.0
output_price_per_mtok = 15.0

[aliases.reasoning/cheapest]
strategy = "cheapest"
targets = ["gpt-4o-mini", "claude-3-haiku"]

[aliases.reasoning/best]
strategy = "first"
targets = ["gpt-4", "claude-3-5-sonnet"]
```

### 7.2 Project overlays

Projects can provide overlays via `reactor.toml`:

```toml
[ai]
registry_overlay = "./ai-models.toml"
# OR
registry_url = "https://example.com/models.toml"
```

Overlays merge with defaults: add new models, override existing ones, define custom aliases.

---

## 8. Usage metering

### 8.1 UsageEvent

```rust
pub struct UsageEvent {
    pub model_id: String,
    pub user_id: Option<String>,  // from AuthCtx.claims.sub
    pub tokens_in: u32,
    pub tokens_out: u32,
}
```

### 8.2 Flow

1. Chat/embedding request completes
2. Extract token counts from response
3. Emit `UsageEvent` via `AiExtensions::post_usage()`
4. Default impl logs; Cloud impl writes to analytics

### 8.3 Analytics integration

Usage events are converted to `reactor-analytics` `TrackEvent`:

```rust
pub const AI_USAGE_EVENT: &str = "ai.usage";

pub fn usage_event_to_properties(event: &UsageEvent) -> HashMap<String, Value> {
    [
        ("model_id", event.model_id.clone().into()),
        ("user_id", event.user_id.clone().into()),
        ("tokens_in", event.tokens_in.into()),
        ("tokens_out", event.tokens_out.into()),
    ].into_iter().collect()
}
```

---

## 9. Extension seam for reactor.cloud

### 9.1 `AiExtensions` trait

```rust
#[async_trait]
pub trait AiExtensions: Send + Sync {
    /// Pre-request hook: quota check, region validation
    async fn pre_request(&self, ctx: &RequestCtx) -> Result<(), AiError>;
    
    /// Post-usage hook: billing debit, alert triggers
    async fn post_usage(&self, event: &UsageEvent) -> Result<(), AiError>;
    
    /// Route override: return alternative upstream for region pinning
    fn route_override(&self, ctx: &RequestCtx) -> Option<String>;
}
```

### 9.2 Open-source default

`NoopExtensions` ships with the crate — all methods are no-ops. `reactor-server` (OSS) uses this.

### 9.3 Cloud override

`reactor.cloud` injects `CloudAiExtensions` that:
- Checks credit balance in `pre_request`
- Debits usage in `post_usage`
- Routes to regional endpoints based on data residency requirements

---

## 10. Auth integration

### 10.1 Middleware

```rust
async fn auth_middleware<B>(
    State(state): State<AiState>,
    headers: HeaderMap,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, AiError> {
    let token = headers.get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    let ctx = if let Some(token) = token {
        let auth_ctx = state.auth.resolve_ctx(token, None).await?;
        AiCtx::authenticated(auth_ctx, request_id)
    } else {
        AiCtx::anonymous(request_id)
    };

    req.extensions_mut().insert(ctx);
    Ok(next.run(req).await)
}
```

### 10.2 User metering

The `user_id` for metering comes from `AuthCtx.claims.sub`:
- Authenticated users: `user_id` = JWT subject claim
- API keys: `user_id` = None (per-key metering only)
- Anonymous: Rejected (401)

---

## 11. Configuration

`reactor-ai-server` reads from env (12-factor):

| Var | Required | Default | Notes |
|---|---|---|---|
| `REACTOR_AI_BIND` | no | `0.0.0.0:8004` | HTTP bind address |
| `REACTOR_AI_AUTH_URL` | yes | — | URL of reactor-auth-server |
| `REACTOR_AI_INTERNAL_SECRET` | yes | — | Shared secret for internal endpoints |
| `REACTOR_AI_REGISTRY_PATH` | no | — | Path to registry overlay TOML |
| `REACTOR_AI_REGISTRY_URL` | no | — | URL to fetch registry overlay |
| `OPENROUTER_API_KEY` | conditional | — | Required if openrouter provider enabled |
| `AWS_ACCESS_KEY_ID` | conditional | — | Required for Bedrock |
| `AWS_SECRET_ACCESS_KEY` | conditional | — | Required for Bedrock |
| `AWS_REGION` | no | `us-east-1` | AWS region for Bedrock |
| `AZURE_FOUNDRY_API_KEY` | conditional | — | Required for Foundry |
| `AZURE_FOUNDRY_ENDPOINT` | conditional | — | Required for Foundry |
| `REACTOR_LOG` | no | `info` | Tracing filter |

---

## 12. SDK integration

### 12.1 Rust client (`reactor-client`)

```rust
// Chat completion
let response = client.ai_chat_completion(request).await?;

// Streaming
let stream = client.ai_chat_stream(request).await?;
while let Some(chunk) = stream.next().await {
    print!("{}", chunk?.choices[0].delta.content.unwrap_or_default());
}

// Embeddings
let embeddings = client.ai_embed(request).await?;

// List models
let models = client.ai_models_list().await?;
```

### 12.2 JS SDK (`@reactor/ai`)

```typescript
const ai = createAiClient(ctx);

// Chat completion
const response = await ai.chatCompletion({ model: 'gpt-4', messages: [...] });

// Streaming
const stream = await ai.chatCompletionStream({ model: 'gpt-4', messages: [...] });
for await (const chunk of stream) {
  console.log(chunk.choices[0]?.delta?.content);
}

// Embeddings
const embeddings = await ai.embed({ model: 'text-embedding-3-small', input: 'Hello' });
```

### 12.3 Swift SDK (`ReactorAI`)

```swift
let ai = AIClient(ctx)

// Chat completion
let response = try await ai.chatCompletion(request)

// Streaming
let stream = ai.chatCompletionStream(request)
for try await chunk in stream {
    print(chunk.choices[0].delta.content ?? "")
}

// Simple chat
let reply = try await ai.chat(model: "gpt-4", prompt: "Hello!")
```

### 12.4 CLI (`reactor-cli`)

```bash
# List models
reactor ai models list

# Test a model
reactor ai test gpt-4 --prompt "Hello world"

# List aliases
reactor ai aliases list
```

---

## 13. Test surface

- **Unit**: Model resolution, alias strategies, registry merging, request/response serialization.
- **Integration**: Mock provider servers, auth middleware verification.
- **Provider conformance**: Each provider adapter tested against real API (gated by credentials).

---

## 14. Build order (v0 slice)

| # | Task | Outcome |
|---|---|---|
| 0 | Land this design doc | Reviewed contract |
| 1 | Scaffold `reactor-ai` + `reactor-ai-server` | `cargo check` clean |
| 2 | Implement `ChatProvider` trait + OpenRouter adapter | Basic chat works |
| 3 | Model registry with defaults.toml | Model resolution works |
| 4 | Chat completions route (streaming + non-streaming) | API endpoint works |
| 5 | Auth middleware + AiCtx | JWT verification works |
| 6 | Usage event emission | Analytics receives events |
| 7 | Embeddings route | Embedding generation works |
| 8 | Bedrock adapter | AWS provider works |
| 9 | Foundry adapter | Azure provider works |
| 10 | OpenAI-compatible adapter | Generic provider works |
| 11 | CLI commands | `reactor ai` works |
| 12 | SDKs (Rust, JS, Swift) | Client libraries complete |

---

## 15. Decision log

| Question | Decision | Rationale |
|---|---|---|
| **User identity** | JWT claims.sub | No separate end_users table; leverages existing reactor-auth users |
| **Extension model** | Trait with no-op default | Cloud can override without touching OSS crate |
| **Model registry** | TOML files with overlay merging | Human-readable, version-controllable, flexible |
| **Streaming protocol** | SSE | OpenAI-compatible, well-supported |
| **Provider abstraction** | Trait per provider | Clean separation, easy to add new providers |

---

## 16. Open questions (deferred)

1. **Prompt caching**: Integration with `reactor-cache` for expensive prompts.
2. **Function calling**: Full tool/function support across providers.
3. **Image generation**: Separate endpoint or unified with chat?
4. **Fine-tuned models**: Custom model registration and hosting.

---

*End of design doc. Land code against checklist §14 in order.*
