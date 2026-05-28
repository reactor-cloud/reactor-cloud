//! Slack connector implementation.
//!
//! Slack uses OAuth2 authentication and is action-heavy (posting messages,
//! managing channels, etc.). Write actions are synthesized for dry-run.

use crate::descriptor::{
    ActionDescriptor, AuthDescriptor, AuthField, AuthKind, ConnectorCapabilities,
    ConnectorDescriptor, DryRunSupport, RateLimitDescriptor, SideEffectKind,
};
use crate::error::ConnectError;
use crate::protocol::ConnectionStatus;
use crate::runtime::native::NativeConnector;
use crate::runtime::{ActionOpts, RuntimeKind};
use async_trait::async_trait;
use serde_json::json;

/// Slack connector.
pub struct SlackConnector {
    http: reqwest::Client,
}

impl Default for SlackConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl SlackConnector {
    /// Create a new Slack connector.
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    fn token_from_config(config: &serde_json::Value) -> Result<String, ConnectError> {
        config
            .get("access_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ConnectError::InvalidInput("access_token required".to_string()))
    }

    async fn call_api(
        &self,
        method: &str,
        token: &str,
        body: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value, ConnectError> {
        let url = format!("https://slack.com/api/{}", method);

        let mut req = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json; charset=utf-8");

        if let Some(body) = body {
            req = req.json(body);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| ConnectError::Internal(format!("request failed: {}", e)))?;

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ConnectError::Internal(format!("json parse failed: {}", e)))?;

        let ok = result.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);

        if ok {
            Ok(result)
        } else {
            let error = result
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unknown_error");
            Err(ConnectError::ActionFailed {
                code: error.to_string(),
                cause: format!("Slack API error: {}", error),
                suggested_fix: None,
            })
        }
    }

    fn action_to_method(action: &str) -> String {
        match action {
            "postMessage" => "chat.postMessage".to_string(),
            "updateMessage" => "chat.update".to_string(),
            "deleteMessage" => "chat.delete".to_string(),
            "listChannels" => "conversations.list".to_string(),
            "getChannelInfo" => "conversations.info".to_string(),
            "joinChannel" => "conversations.join".to_string(),
            "listUsers" => "users.list".to_string(),
            "getUserInfo" => "users.info".to_string(),
            _ => action.to_string(),
        }
    }
}

#[async_trait]
impl NativeConnector for SlackConnector {
    fn descriptor(&self) -> ConnectorDescriptor {
        ConnectorDescriptor {
            type_id: "slack".to_string(),
            display_name: "Slack".to_string(),
            version: "1.0.0".to_string(),
            runtime: RuntimeKind::Native,
            auth: AuthDescriptor {
                kind: AuthKind::OAuth2 {
                    authorize_url: "https://slack.com/oauth/v2/authorize".to_string(),
                    token_url: "https://slack.com/api/oauth.v2.access".to_string(),
                    scopes: vec![
                        "chat:write".to_string(),
                        "channels:read".to_string(),
                        "users:read".to_string(),
                    ],
                    pkce: false,
                    reactor_proxy: false,
                },
                fields: vec![
                    AuthField {
                        name: "client_id".to_string(),
                        label: "Client ID".to_string(),
                        sensitive: false,
                        required: true,
                        description: Some("Slack App Client ID".to_string()),
                    },
                    AuthField {
                        name: "client_secret".to_string(),
                        label: "Client Secret".to_string(),
                        sensitive: true,
                        required: true,
                        description: Some("Slack App Client Secret".to_string()),
                    },
                ],
                test: None,
            },
            streams: vec![],
            actions: vec![
                ActionDescriptor {
                    name: "postMessage".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["channel", "text"],
                        "properties": {
                            "channel": { "type": "string", "description": "Channel ID or name" },
                            "text": { "type": "string", "description": "Message text" },
                            "blocks": { "type": "array", "description": "Block Kit blocks" },
                            "thread_ts": { "type": "string", "description": "Thread timestamp for reply" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "ok": { "type": "boolean" },
                            "ts": { "type": "string" },
                            "channel": { "type": "string" }
                        }
                    }),
                    side_effects: SideEffectKind::Sends,
                    dry_run: DryRunSupport::Synthesized,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "updateMessage".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["channel", "ts", "text"],
                        "properties": {
                            "channel": { "type": "string" },
                            "ts": { "type": "string", "description": "Message timestamp" },
                            "text": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "ok": { "type": "boolean" },
                            "ts": { "type": "string" },
                            "channel": { "type": "string" }
                        }
                    }),
                    side_effects: SideEffectKind::Mutates,
                    dry_run: DryRunSupport::Synthesized,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "deleteMessage".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["channel", "ts"],
                        "properties": {
                            "channel": { "type": "string" },
                            "ts": { "type": "string", "description": "Message timestamp" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "ok": { "type": "boolean" }
                        }
                    }),
                    side_effects: SideEffectKind::Mutates,
                    dry_run: DryRunSupport::Synthesized,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "listChannels".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 100 },
                            "cursor": { "type": "string" },
                            "types": { "type": "string", "default": "public_channel" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "ok": { "type": "boolean" },
                            "channels": { "type": "array" },
                            "response_metadata": { "type": "object" }
                        }
                    }),
                    side_effects: SideEffectKind::Reads,
                    dry_run: DryRunSupport::Native,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "getChannelInfo".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["channel"],
                        "properties": {
                            "channel": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "ok": { "type": "boolean" },
                            "channel": { "type": "object" }
                        }
                    }),
                    side_effects: SideEffectKind::Reads,
                    dry_run: DryRunSupport::Native,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "listUsers".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 100 },
                            "cursor": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "ok": { "type": "boolean" },
                            "members": { "type": "array" },
                            "response_metadata": { "type": "object" }
                        }
                    }),
                    side_effects: SideEffectKind::Reads,
                    dry_run: DryRunSupport::Native,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "getUserInfo".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["user"],
                        "properties": {
                            "user": { "type": "string", "description": "User ID" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "ok": { "type": "boolean" },
                            "user": { "type": "object" }
                        }
                    }),
                    side_effects: SideEffectKind::Reads,
                    dry_run: DryRunSupport::Native,
                    idempotency: None,
                },
            ],
            webhooks: vec![],
            capabilities: ConnectorCapabilities {
                sandbox_mode: false,
                vendor_test_mode: false,
                cdc: false,
                incremental: false,
                schema_discovery: false,
            },
            rate_limits: Some(RateLimitDescriptor {
                requests_per_minute: Some(60),
                requests_per_second: Some(1),
                requests_per_hour: None,
                requests_per_day: None,
                concurrent_requests: None,
            }),
            doc_url: Some("https://api.slack.com/methods".to_string()),
        }
    }

    async fn check(&self, config: &serde_json::Value) -> Result<ConnectionStatus, ConnectError> {
        let token = Self::token_from_config(config)?;

        // Call auth.test to verify credentials
        match self.call_api("auth.test", &token, None).await {
            Ok(result) => {
                let team = result
                    .get("team")
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown");
                let user = result
                    .get("user")
                    .and_then(|u| u.as_str())
                    .unwrap_or("unknown");
                tracing::info!(team = team, user = user, "Slack auth verified");
                Ok(ConnectionStatus::succeeded())
            }
            Err(ConnectError::ActionFailed { cause, .. }) => Ok(ConnectionStatus::failed(format!(
                "Authentication failed: {}. Verify your OAuth token has required scopes.",
                cause
            ))),
            Err(e) => Err(e),
        }
    }

    async fn invoke_action(
        &self,
        config: &serde_json::Value,
        action: &str,
        input: &serde_json::Value,
        opts: &ActionOpts,
    ) -> Result<serde_json::Value, ConnectError> {
        let token = Self::token_from_config(config)?;

        // Synthesize dry-run for write actions
        if opts.dry_run {
            match action {
                "postMessage" => {
                    return Ok(json!({
                        "ok": true,
                        "ts": "1234567890.123456",
                        "channel": input.get("channel"),
                        "_dry_run": true
                    }));
                }
                "updateMessage" => {
                    return Ok(json!({
                        "ok": true,
                        "ts": input.get("ts"),
                        "channel": input.get("channel"),
                        "_dry_run": true
                    }));
                }
                "deleteMessage" => {
                    return Ok(json!({
                        "ok": true,
                        "_dry_run": true
                    }));
                }
                _ => {}
            }
        }

        let method = Self::action_to_method(action);
        self.call_api(&method, &token, Some(input)).await
    }
}
