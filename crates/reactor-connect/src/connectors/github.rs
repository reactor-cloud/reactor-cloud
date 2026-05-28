//! GitHub connector implementation.
//!
//! GitHub uses OAuth2 or PAT authentication. Includes a read-only issues
//! stream as a plumbing canary for stream support. Write actions are
//! synthesized for dry-run.

use crate::descriptor::{
    ActionDescriptor, AuthDescriptor, AuthField, AuthKind, ConnectorCapabilities,
    ConnectorDescriptor, DryRunSupport, RateLimitDescriptor, SideEffectKind, StreamDescriptor,
    SyncMode,
};
use crate::error::ConnectError;
use crate::protocol::{ConnectionStatus, DiscoveredCatalog};
use crate::runtime::native::NativeConnector;
use crate::runtime::{ActionOpts, RuntimeKind};
use async_trait::async_trait;
use serde_json::json;

/// GitHub connector.
pub struct GitHubConnector {
    http: reqwest::Client,
}

impl Default for GitHubConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl GitHubConnector {
    /// Create a new GitHub connector.
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    fn token_from_config(config: &serde_json::Value) -> Result<String, ConnectError> {
        config
            .get("access_token")
            .or_else(|| config.get("token"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ConnectError::InvalidInput("access_token required".to_string()))
    }

    async fn api_get(
        &self,
        token: &str,
        path: &str,
    ) -> Result<serde_json::Value, ConnectError> {
        let url = format!("https://api.github.com{}", path);

        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "reactor-connect/1.0")
            .send()
            .await
            .map_err(|e| ConnectError::Internal(format!("request failed: {}", e)))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ConnectError::Internal(format!("json parse failed: {}", e)))?;

        if status.is_success() {
            Ok(body)
        } else {
            let msg = body
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            Err(ConnectError::ActionFailed {
                code: format!("http_{}", status.as_u16()),
                cause: msg.to_string(),
                suggested_fix: None,
            })
        }
    }

    async fn api_post(
        &self,
        token: &str,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ConnectError> {
        let url = format!("https://api.github.com{}", path);

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "reactor-connect/1.0")
            .json(body)
            .send()
            .await
            .map_err(|e| ConnectError::Internal(format!("request failed: {}", e)))?;

        let status = resp.status();
        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ConnectError::Internal(format!("json parse failed: {}", e)))?;

        if status.is_success() || status == reqwest::StatusCode::CREATED {
            Ok(result)
        } else {
            let msg = result
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            Err(ConnectError::ActionFailed {
                code: format!("http_{}", status.as_u16()),
                cause: msg.to_string(),
                suggested_fix: None,
            })
        }
    }

    async fn api_patch(
        &self,
        token: &str,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, ConnectError> {
        let url = format!("https://api.github.com{}", path);

        let resp = self
            .http
            .patch(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("User-Agent", "reactor-connect/1.0")
            .json(body)
            .send()
            .await
            .map_err(|e| ConnectError::Internal(format!("request failed: {}", e)))?;

        let status = resp.status();
        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ConnectError::Internal(format!("json parse failed: {}", e)))?;

        if status.is_success() {
            Ok(result)
        } else {
            let msg = result
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            Err(ConnectError::ActionFailed {
                code: format!("http_{}", status.as_u16()),
                cause: msg.to_string(),
                suggested_fix: None,
            })
        }
    }

    fn get_owner_repo<'a>(
        input: &'a serde_json::Value,
        config: &'a serde_json::Value,
    ) -> Result<(&'a str, &'a str), ConnectError> {
        let owner = input
            .get("owner")
            .or_else(|| config.get("owner"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ConnectError::InvalidInput("owner required".to_string()))?;

        let repo = input
            .get("repo")
            .or_else(|| config.get("repo"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| ConnectError::InvalidInput("repo required".to_string()))?;

        Ok((owner, repo))
    }

    async fn execute_action(
        &self,
        token: &str,
        config: &serde_json::Value,
        action: &str,
        input: &serde_json::Value,
        dry_run: bool,
    ) -> Result<serde_json::Value, ConnectError> {
        match action {
            "createIssue" => {
                let (owner, repo) = Self::get_owner_repo(input, config)?;

                if dry_run {
                    return Ok(json!({
                        "id": 0,
                        "number": 0,
                        "title": input.get("title"),
                        "state": "open",
                        "html_url": format!("https://github.com/{}/{}/issues/0", owner, repo),
                        "_dry_run": true
                    }));
                }

                let path = format!("/repos/{}/{}/issues", owner, repo);
                let body = json!({
                    "title": input.get("title"),
                    "body": input.get("body"),
                    "labels": input.get("labels"),
                    "assignees": input.get("assignees")
                });

                self.api_post(token, &path, &body).await
            }
            "updateIssue" => {
                let (owner, repo) = Self::get_owner_repo(input, config)?;
                let issue_number = input
                    .get("issue_number")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| {
                        ConnectError::InvalidInput("issue_number required".to_string())
                    })?;

                if dry_run {
                    return Ok(json!({
                        "id": 0,
                        "number": issue_number,
                        "title": input.get("title"),
                        "_dry_run": true
                    }));
                }

                let path = format!("/repos/{}/{}/issues/{}", owner, repo, issue_number);
                let body = json!({
                    "title": input.get("title"),
                    "body": input.get("body"),
                    "state": input.get("state"),
                    "labels": input.get("labels"),
                    "assignees": input.get("assignees")
                });

                self.api_patch(token, &path, &body).await
            }
            "getIssue" => {
                let (owner, repo) = Self::get_owner_repo(input, config)?;
                let issue_number = input
                    .get("issue_number")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| {
                        ConnectError::InvalidInput("issue_number required".to_string())
                    })?;

                let path = format!("/repos/{}/{}/issues/{}", owner, repo, issue_number);
                self.api_get(token, &path).await
            }
            "listIssues" => {
                let (owner, repo) = Self::get_owner_repo(input, config)?;
                let mut path = format!("/repos/{}/{}/issues", owner, repo);

                let mut query_parts = Vec::new();
                if let Some(state) = input.get("state").and_then(|v| v.as_str()) {
                    query_parts.push(format!("state={}", state));
                }
                if let Some(sort) = input.get("sort").and_then(|v| v.as_str()) {
                    query_parts.push(format!("sort={}", sort));
                }
                if let Some(dir) = input.get("direction").and_then(|v| v.as_str()) {
                    query_parts.push(format!("direction={}", dir));
                }
                if let Some(per_page) = input.get("per_page").and_then(|v| v.as_i64()) {
                    query_parts.push(format!("per_page={}", per_page));
                }
                if let Some(page) = input.get("page").and_then(|v| v.as_i64()) {
                    query_parts.push(format!("page={}", page));
                }

                if !query_parts.is_empty() {
                    path = format!("{}?{}", path, query_parts.join("&"));
                }

                self.api_get(token, &path).await
            }
            "createComment" => {
                let (owner, repo) = Self::get_owner_repo(input, config)?;
                let issue_number = input
                    .get("issue_number")
                    .and_then(|v| v.as_i64())
                    .ok_or_else(|| {
                        ConnectError::InvalidInput("issue_number required".to_string())
                    })?;

                if dry_run {
                    return Ok(json!({
                        "id": 0,
                        "body": input.get("body"),
                        "_dry_run": true
                    }));
                }

                let path = format!(
                    "/repos/{}/{}/issues/{}/comments",
                    owner, repo, issue_number
                );

                let body = json!({
                    "body": input.get("body")
                });

                self.api_post(token, &path, &body).await
            }
            _ => Err(ConnectError::ActionNotFound(action.to_string())),
        }
    }
}

#[async_trait]
impl NativeConnector for GitHubConnector {
    fn descriptor(&self) -> ConnectorDescriptor {
        ConnectorDescriptor {
            type_id: "github".to_string(),
            display_name: "GitHub".to_string(),
            version: "1.0.0".to_string(),
            runtime: RuntimeKind::Native,
            auth: AuthDescriptor {
                kind: AuthKind::OAuth2 {
                    authorize_url: "https://github.com/login/oauth/authorize".to_string(),
                    token_url: "https://github.com/login/oauth/access_token".to_string(),
                    scopes: vec!["repo".to_string(), "read:user".to_string()],
                    pkce: false,
                    reactor_proxy: false,
                },
                fields: vec![
                    AuthField {
                        name: "client_id".to_string(),
                        label: "Client ID".to_string(),
                        sensitive: false,
                        required: true,
                        description: Some("GitHub OAuth App Client ID".to_string()),
                    },
                    AuthField {
                        name: "client_secret".to_string(),
                        label: "Client Secret".to_string(),
                        sensitive: true,
                        required: true,
                        description: Some("GitHub OAuth App Client Secret".to_string()),
                    },
                ],
                test: None,
            },
            streams: vec![StreamDescriptor {
                name: "issues".to_string(),
                json_schema: json!({
                    "type": "object",
                    "properties": {
                        "id": { "type": "integer" },
                        "number": { "type": "integer" },
                        "title": { "type": "string" },
                        "state": { "type": "string" },
                        "body": { "type": "string" },
                        "user": {
                            "type": "object",
                            "properties": {
                                "login": { "type": "string" },
                                "id": { "type": "integer" }
                            }
                        },
                        "labels": { "type": "array" },
                        "assignees": { "type": "array" },
                        "created_at": { "type": "string", "format": "date-time" },
                        "updated_at": { "type": "string", "format": "date-time" }
                    }
                }),
                supported_modes: vec![SyncMode::FullRefresh, SyncMode::IncrementalAppend],
                cursor_field: Some(vec!["updated_at".to_string()]),
                primary_key: Some(vec![vec!["id".to_string()]]),
                supports_outbound: false,
                source_defined: false,
            }],
            actions: vec![
                ActionDescriptor {
                    name: "createIssue".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["title"],
                        "properties": {
                            "owner": { "type": "string" },
                            "repo": { "type": "string" },
                            "title": { "type": "string" },
                            "body": { "type": "string" },
                            "labels": { "type": "array", "items": { "type": "string" } },
                            "assignees": { "type": "array", "items": { "type": "string" } }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer" },
                            "number": { "type": "integer" },
                            "title": { "type": "string" },
                            "html_url": { "type": "string" }
                        }
                    }),
                    side_effects: SideEffectKind::Mutates,
                    dry_run: DryRunSupport::Synthesized,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "updateIssue".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["issue_number"],
                        "properties": {
                            "owner": { "type": "string" },
                            "repo": { "type": "string" },
                            "issue_number": { "type": "integer" },
                            "title": { "type": "string" },
                            "body": { "type": "string" },
                            "state": { "type": "string", "enum": ["open", "closed"] },
                            "labels": { "type": "array", "items": { "type": "string" } },
                            "assignees": { "type": "array", "items": { "type": "string" } }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer" },
                            "number": { "type": "integer" },
                            "title": { "type": "string" }
                        }
                    }),
                    side_effects: SideEffectKind::Mutates,
                    dry_run: DryRunSupport::Synthesized,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "getIssue".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["issue_number"],
                        "properties": {
                            "owner": { "type": "string" },
                            "repo": { "type": "string" },
                            "issue_number": { "type": "integer" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer" },
                            "number": { "type": "integer" },
                            "title": { "type": "string" },
                            "state": { "type": "string" },
                            "body": { "type": "string" }
                        }
                    }),
                    side_effects: SideEffectKind::Reads,
                    dry_run: DryRunSupport::Native,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "listIssues".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "owner": { "type": "string" },
                            "repo": { "type": "string" },
                            "state": { "type": "string", "enum": ["open", "closed", "all"], "default": "open" },
                            "sort": { "type": "string", "enum": ["created", "updated", "comments"] },
                            "direction": { "type": "string", "enum": ["asc", "desc"] },
                            "per_page": { "type": "integer", "minimum": 1, "maximum": 100, "default": 30 },
                            "page": { "type": "integer", "minimum": 1 }
                        }
                    }),
                    output_schema: json!({
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "integer" },
                                "number": { "type": "integer" },
                                "title": { "type": "string" },
                                "state": { "type": "string" }
                            }
                        }
                    }),
                    side_effects: SideEffectKind::Reads,
                    dry_run: DryRunSupport::Native,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "createComment".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["issue_number", "body"],
                        "properties": {
                            "owner": { "type": "string" },
                            "repo": { "type": "string" },
                            "issue_number": { "type": "integer" },
                            "body": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "id": { "type": "integer" },
                            "body": { "type": "string" },
                            "html_url": { "type": "string" }
                        }
                    }),
                    side_effects: SideEffectKind::Sends,
                    dry_run: DryRunSupport::Synthesized,
                    idempotency: None,
                },
            ],
            webhooks: vec![],
            capabilities: ConnectorCapabilities {
                sandbox_mode: false,
                vendor_test_mode: false,
                cdc: false,
                incremental: true,
                schema_discovery: false,
            },
            rate_limits: Some(RateLimitDescriptor {
                requests_per_hour: Some(5000),
                requests_per_second: None,
                requests_per_minute: None,
                requests_per_day: None,
                concurrent_requests: Some(100),
            }),
            doc_url: Some("https://docs.github.com/en/rest".to_string()),
        }
    }

    async fn check(&self, config: &serde_json::Value) -> Result<ConnectionStatus, ConnectError> {
        let token = Self::token_from_config(config)?;

        match self.api_get(&token, "/user").await {
            Ok(user) => {
                let login = user
                    .get("login")
                    .and_then(|l| l.as_str())
                    .unwrap_or("unknown");
                tracing::info!(user = login, "GitHub auth verified");
                Ok(ConnectionStatus::succeeded())
            }
            Err(ConnectError::ActionFailed { cause, .. }) => Ok(ConnectionStatus::failed(format!(
                "Authentication failed: {}. Verify your token has required scopes (repo).",
                cause
            ))),
            Err(e) => Err(e),
        }
    }

    async fn discover(
        &self,
        _config: &serde_json::Value,
    ) -> Result<DiscoveredCatalog, ConnectError> {
        // Return the issues stream from the descriptor
        let descriptor = self.descriptor();
        Ok(DiscoveredCatalog {
            streams: descriptor.streams,
        })
    }

    async fn invoke_action(
        &self,
        config: &serde_json::Value,
        action: &str,
        input: &serde_json::Value,
        opts: &ActionOpts,
    ) -> Result<serde_json::Value, ConnectError> {
        let token = Self::token_from_config(config)?;
        self.execute_action(&token, config, action, input, opts.dry_run)
            .await
    }
}
