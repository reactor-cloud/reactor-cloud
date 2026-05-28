//! Linear connector implementation.
//!
//! Linear uses OAuth2 or API key authentication with a GraphQL API.
//! Write actions are synthesized for dry-run.

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

/// Linear connector.
pub struct LinearConnector {
    http: reqwest::Client,
}

impl Default for LinearConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl LinearConnector {
    /// Create a new Linear connector.
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    fn api_key_from_config(config: &serde_json::Value) -> Result<String, ConnectError> {
        // Support both OAuth access_token and API key
        config
            .get("access_token")
            .or_else(|| config.get("api_key"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                ConnectError::InvalidInput("access_token or api_key required".to_string())
            })
    }

    async fn graphql(
        &self,
        api_key: &str,
        query: &str,
        variables: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, ConnectError> {
        let body = json!({
            "query": query,
            "variables": variables.unwrap_or_default()
        });

        let resp = self
            .http
            .post("https://api.linear.app/graphql")
            .header("Authorization", api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ConnectError::Internal(format!("request failed: {}", e)))?;

        let result: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ConnectError::Internal(format!("json parse failed: {}", e)))?;

        if let Some(errors) = result.get("errors") {
            let first_error = errors
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            return Err(ConnectError::ActionFailed {
                code: "graphql_error".to_string(),
                cause: first_error.to_string(),
                suggested_fix: None,
            });
        }

        result
            .get("data")
            .cloned()
            .ok_or_else(|| ConnectError::Internal("no data in response".to_string()))
    }

    async fn execute_action(
        &self,
        api_key: &str,
        action: &str,
        input: &serde_json::Value,
        dry_run: bool,
    ) -> Result<serde_json::Value, ConnectError> {
        match action {
            "createIssue" => {
                if dry_run {
                    return Ok(json!({
                        "issueCreate": {
                            "success": true,
                            "issue": {
                                "id": "dry_run_issue_id",
                                "title": input.get("title"),
                                "teamId": input.get("teamId"),
                                "_dry_run": true
                            }
                        }
                    }));
                }

                let query = r#"
                    mutation IssueCreate($input: IssueCreateInput!) {
                        issueCreate(input: $input) {
                            success
                            issue {
                                id
                                identifier
                                title
                                url
                            }
                        }
                    }
                "#;

                self.graphql(api_key, query, Some(json!({ "input": input })))
                    .await
            }
            "updateIssue" => {
                let issue_id = input
                    .get("issueId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ConnectError::InvalidInput("issueId required".to_string()))?;

                if dry_run {
                    return Ok(json!({
                        "issueUpdate": {
                            "success": true,
                            "issue": {
                                "id": issue_id,
                                "_dry_run": true
                            }
                        }
                    }));
                }

                let query = r#"
                    mutation IssueUpdate($id: String!, $input: IssueUpdateInput!) {
                        issueUpdate(id: $id, input: $input) {
                            success
                            issue {
                                id
                                identifier
                                title
                                url
                            }
                        }
                    }
                "#;

                let mut update_input = input.clone();
                if let Some(obj) = update_input.as_object_mut() {
                    obj.remove("issueId");
                }

                self.graphql(
                    api_key,
                    query,
                    Some(json!({ "id": issue_id, "input": update_input })),
                )
                .await
            }
            "getIssue" => {
                let issue_id = input
                    .get("issueId")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ConnectError::InvalidInput("issueId required".to_string()))?;

                let query = r#"
                    query Issue($id: String!) {
                        issue(id: $id) {
                            id
                            identifier
                            title
                            description
                            state { name }
                            assignee { name email }
                            priority
                            url
                        }
                    }
                "#;

                self.graphql(api_key, query, Some(json!({ "id": issue_id })))
                    .await
            }
            "listIssues" => {
                let query = r#"
                    query Issues($first: Int, $after: String) {
                        issues(first: $first, after: $after) {
                            nodes {
                                id
                                identifier
                                title
                                state { name }
                                priority
                            }
                            pageInfo {
                                hasNextPage
                                endCursor
                            }
                        }
                    }
                "#;

                let first = input
                    .get("limit")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(50);
                let after = input.get("cursor").cloned();

                self.graphql(
                    api_key,
                    query,
                    Some(json!({ "first": first, "after": after })),
                )
                .await
            }
            "listTeams" => {
                let query = r#"
                    query Teams {
                        teams {
                            nodes {
                                id
                                name
                                key
                            }
                        }
                    }
                "#;

                self.graphql(api_key, query, None).await
            }
            _ => Err(ConnectError::ActionNotFound(action.to_string())),
        }
    }
}

#[async_trait]
impl NativeConnector for LinearConnector {
    fn descriptor(&self) -> ConnectorDescriptor {
        ConnectorDescriptor {
            type_id: "linear".to_string(),
            display_name: "Linear".to_string(),
            version: "1.0.0".to_string(),
            runtime: RuntimeKind::Native,
            auth: AuthDescriptor {
                kind: AuthKind::OAuth2 {
                    authorize_url: "https://linear.app/oauth/authorize".to_string(),
                    token_url: "https://api.linear.app/oauth/token".to_string(),
                    scopes: vec!["read".to_string(), "write".to_string()],
                    pkce: true,
                    reactor_proxy: false,
                },
                fields: vec![
                    AuthField {
                        name: "client_id".to_string(),
                        label: "Client ID".to_string(),
                        sensitive: false,
                        required: true,
                        description: Some("Linear OAuth Client ID".to_string()),
                    },
                    AuthField {
                        name: "client_secret".to_string(),
                        label: "Client Secret".to_string(),
                        sensitive: true,
                        required: true,
                        description: Some("Linear OAuth Client Secret".to_string()),
                    },
                ],
                test: None,
            },
            streams: vec![],
            actions: vec![
                ActionDescriptor {
                    name: "createIssue".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["title", "teamId"],
                        "properties": {
                            "title": { "type": "string" },
                            "description": { "type": "string" },
                            "teamId": { "type": "string" },
                            "assigneeId": { "type": "string" },
                            "priority": { "type": "integer", "minimum": 0, "maximum": 4 },
                            "stateId": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "issueCreate": {
                                "type": "object",
                                "properties": {
                                    "success": { "type": "boolean" },
                                    "issue": {
                                        "type": "object",
                                        "properties": {
                                            "id": { "type": "string" },
                                            "identifier": { "type": "string" },
                                            "title": { "type": "string" },
                                            "url": { "type": "string" }
                                        }
                                    }
                                }
                            }
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
                        "required": ["issueId"],
                        "properties": {
                            "issueId": { "type": "string" },
                            "title": { "type": "string" },
                            "description": { "type": "string" },
                            "assigneeId": { "type": "string" },
                            "priority": { "type": "integer" },
                            "stateId": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "issueUpdate": {
                                "type": "object",
                                "properties": {
                                    "success": { "type": "boolean" },
                                    "issue": { "type": "object" }
                                }
                            }
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
                        "required": ["issueId"],
                        "properties": {
                            "issueId": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "issue": { "type": "object" }
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
                            "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 50 },
                            "cursor": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "issues": {
                                "type": "object",
                                "properties": {
                                    "nodes": { "type": "array" },
                                    "pageInfo": { "type": "object" }
                                }
                            }
                        }
                    }),
                    side_effects: SideEffectKind::Reads,
                    dry_run: DryRunSupport::Native,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "listTeams".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {}
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "teams": {
                                "type": "object",
                                "properties": {
                                    "nodes": { "type": "array" }
                                }
                            }
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
                incremental: true,
                schema_discovery: false,
            },
            rate_limits: Some(RateLimitDescriptor {
                requests_per_minute: Some(1500),
                requests_per_second: None,
                requests_per_hour: Some(50000),
                requests_per_day: None,
                concurrent_requests: None,
            }),
            doc_url: Some("https://developers.linear.app/docs".to_string()),
        }
    }

    async fn check(&self, config: &serde_json::Value) -> Result<ConnectionStatus, ConnectError> {
        let api_key = Self::api_key_from_config(config)?;

        // Query viewer to verify credentials
        let query = r#"
            query { viewer { id name email } }
        "#;

        match self.graphql(&api_key, query, None).await {
            Ok(data) => {
                let name = data
                    .get("viewer")
                    .and_then(|v| v.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");
                tracing::info!(user = name, "Linear auth verified");
                Ok(ConnectionStatus::succeeded())
            }
            Err(ConnectError::ActionFailed { cause, .. }) => Ok(ConnectionStatus::failed(format!(
                "Authentication failed: {}. Verify your API key or OAuth token is valid.",
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
        let api_key = Self::api_key_from_config(config)?;
        self.execute_action(&api_key, action, input, opts.dry_run)
            .await
    }
}
