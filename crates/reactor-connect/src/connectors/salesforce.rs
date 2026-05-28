//! Salesforce connector implementation.
//!
//! Salesforce supports multiple auth modes:
//! - OAuth2 PKCE (recommended)
//! - JWT Bearer token
//! - Username/password (legacy)
//!
//! Streams use SOQL for queries and Bulk API 2.0 for large data pulls.
//! Actions use REST API and Composite API for batch operations.
//! Webhooks use Platform Events and CDC via CometD (Streaming API).

use crate::descriptor::{
    ActionDescriptor, AuthDescriptor, AuthField, AuthKind, ConnectorCapabilities,
    ConnectorDescriptor, DryRunSupport, RateLimitDescriptor, SideEffectKind, StreamDescriptor,
    SyncMode, VerificationKind, WebhookDescriptor,
};
use crate::error::ConnectError;
use crate::protocol::{
    AirbyteRecordMessage, ConfiguredCatalog, ConnectorMessage, ConnectionStatus,
    DiscoveredCatalog, StateBundle, SyncLimits,
};
use crate::runtime::native::NativeConnector;
use crate::runtime::{ActionOpts, MessageStream, RuntimeKind};
use async_trait::async_trait;
use serde_json::{json, Value};

/// Salesforce connector.
pub struct SalesforceConnector {
    http: reqwest::Client,
}

impl Default for SalesforceConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl SalesforceConnector {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    fn instance_url(config: &Value) -> Result<String, ConnectError> {
        config
            .get("instance_url")
            .and_then(|v| v.as_str())
            .map(|s| s.trim_end_matches('/').to_string())
            .ok_or_else(|| ConnectError::InvalidInput("instance_url required".into()))
    }

    fn access_token(config: &Value) -> Result<String, ConnectError> {
        config
            .get("access_token")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| ConnectError::InvalidInput("access_token required".into()))
    }

    async fn api_request(
        &self,
        method: reqwest::Method,
        url: &str,
        access_token: &str,
        body: Option<&Value>,
    ) -> Result<Value, ConnectError> {
        let mut req = self
            .http
            .request(method, url)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Type", "application/json");

        if let Some(body) = body {
            req = req.json(body);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status.is_success() {
            resp.json().await.map_err(Into::into)
        } else {
            let error_body: Value = resp.json().await.unwrap_or_default();
            let msg = error_body
                .get(0)
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Salesforce API error");
            Err(ConnectError::ActionFailed {
                code: status.as_str().to_string(),
                cause: msg.to_string(),
                suggested_fix: None,
            })
        }
    }

    async fn soql_query(
        &self,
        instance_url: &str,
        access_token: &str,
        query: &str,
    ) -> Result<Value, ConnectError> {
        let url = format!(
            "{}/services/data/v59.0/query?q={}",
            instance_url,
            urlencoding::encode(query)
        );
        self.api_request(reqwest::Method::GET, &url, access_token, None)
            .await
    }

    async fn query_next_page(
        &self,
        instance_url: &str,
        access_token: &str,
        next_records_url: &str,
    ) -> Result<Value, ConnectError> {
        let url = format!("{}{}", instance_url, next_records_url);
        self.api_request(reqwest::Method::GET, &url, access_token, None)
            .await
    }

    fn standard_stream(name: &str) -> StreamDescriptor {
        StreamDescriptor {
            name: name.to_string(),
            json_schema: json!({
                "type": "object",
                "properties": {
                    "Id": { "type": "string" },
                    "Name": { "type": "string" },
                    "CreatedDate": { "type": "string", "format": "date-time" },
                    "LastModifiedDate": { "type": "string", "format": "date-time" }
                }
            }),
            supported_modes: vec![SyncMode::FullRefresh, SyncMode::IncrementalAppend],
            cursor_field: Some(vec!["LastModifiedDate".to_string()]),
            primary_key: Some(vec![vec!["Id".to_string()]]),
            supports_outbound: true,
            source_defined: false,
        }
    }
}

#[async_trait]
impl NativeConnector for SalesforceConnector {
    fn descriptor(&self) -> ConnectorDescriptor {
        ConnectorDescriptor {
            type_id: "salesforce".to_string(),
            display_name: "Salesforce".to_string(),
            version: "1.0.0".to_string(),
            runtime: RuntimeKind::Native,
            auth: AuthDescriptor {
                kind: AuthKind::OAuth2 {
                    authorize_url: "https://login.salesforce.com/services/oauth2/authorize".into(),
                    token_url: "https://login.salesforce.com/services/oauth2/token".into(),
                    scopes: vec!["api".into(), "refresh_token".into(), "offline_access".into()],
                    pkce: true,
                    reactor_proxy: false,
                },
                fields: vec![
                    AuthField {
                        name: "instance_url".to_string(),
                        label: "Instance URL".to_string(),
                        sensitive: false,
                        required: true,
                        description: Some("Your Salesforce instance URL (e.g., https://yourorg.my.salesforce.com)".into()),
                    },
                    AuthField {
                        name: "client_id".to_string(),
                        label: "Client ID".to_string(),
                        sensitive: true,
                        required: true,
                        description: Some("Connected App consumer key".into()),
                    },
                    AuthField {
                        name: "client_secret".to_string(),
                        label: "Client Secret".to_string(),
                        sensitive: true,
                        required: false,
                        description: Some("Connected App consumer secret (optional for PKCE)".into()),
                    },
                    AuthField {
                        name: "refresh_token".to_string(),
                        label: "Refresh Token".to_string(),
                        sensitive: true,
                        required: true,
                        description: Some("OAuth refresh token".into()),
                    },
                ],
                test: None,
            },
            streams: vec![
                Self::standard_stream("Lead"),
                Self::standard_stream("Contact"),
                Self::standard_stream("Account"),
                Self::standard_stream("Opportunity"),
                Self::standard_stream("Case"),
                Self::standard_stream("Task"),
                Self::standard_stream("User"),
            ],
            actions: vec![
                ActionDescriptor {
                    name: "createLead".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["LastName", "Company"],
                        "properties": {
                            "FirstName": { "type": "string" },
                            "LastName": { "type": "string" },
                            "Company": { "type": "string" },
                            "Email": { "type": "string", "format": "email" },
                            "Phone": { "type": "string" },
                            "Status": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "success": { "type": "boolean" },
                            "errors": { "type": "array" }
                        }
                    }),
                    side_effects: SideEffectKind::Mutates,
                    dry_run: DryRunSupport::Synthesized,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "convertLead".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["leadId"],
                        "properties": {
                            "leadId": { "type": "string" },
                            "accountId": { "type": "string" },
                            "contactId": { "type": "string" },
                            "convertedStatus": { "type": "string" },
                            "opportunityName": { "type": "string" },
                            "doNotCreateOpportunity": { "type": "boolean" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "accountId": { "type": "string" },
                            "contactId": { "type": "string" },
                            "opportunityId": { "type": "string" }
                        }
                    }),
                    side_effects: SideEffectKind::Mutates,
                    dry_run: DryRunSupport::Synthesized,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "createContact".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["LastName"],
                        "properties": {
                            "FirstName": { "type": "string" },
                            "LastName": { "type": "string" },
                            "AccountId": { "type": "string" },
                            "Email": { "type": "string", "format": "email" },
                            "Phone": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "success": { "type": "boolean" }
                        }
                    }),
                    side_effects: SideEffectKind::Mutates,
                    dry_run: DryRunSupport::Synthesized,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "createOpportunity".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["Name", "StageName", "CloseDate"],
                        "properties": {
                            "Name": { "type": "string" },
                            "AccountId": { "type": "string" },
                            "StageName": { "type": "string" },
                            "CloseDate": { "type": "string", "format": "date" },
                            "Amount": { "type": "number" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "success": { "type": "boolean" }
                        }
                    }),
                    side_effects: SideEffectKind::Mutates,
                    dry_run: DryRunSupport::Synthesized,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "createCase".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "Subject": { "type": "string" },
                            "Description": { "type": "string" },
                            "ContactId": { "type": "string" },
                            "AccountId": { "type": "string" },
                            "Status": { "type": "string" },
                            "Priority": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "success": { "type": "boolean" }
                        }
                    }),
                    side_effects: SideEffectKind::Mutates,
                    dry_run: DryRunSupport::Synthesized,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "soqlQuery".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["query"],
                        "properties": {
                            "query": { "type": "string", "description": "SOQL query string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "totalSize": { "type": "integer" },
                            "done": { "type": "boolean" },
                            "records": { "type": "array" }
                        }
                    }),
                    side_effects: SideEffectKind::Reads,
                    dry_run: DryRunSupport::Native,
                    idempotency: None,
                },
            ],
            webhooks: vec![
                WebhookDescriptor {
                    name: "platformEvent".to_string(),
                    verification: VerificationKind::Custom {
                        docs_url: "https://developer.salesforce.com/docs/platform/platform-events".into(),
                    },
                    event_types: vec!["*".to_string()],
                    replay_window_seconds: 300,
                    setup_instructions: "Platform Events use Salesforce Streaming API (CometD), not HTTP webhooks.".into(),
                },
                WebhookDescriptor {
                    name: "changeDataCapture".to_string(),
                    verification: VerificationKind::Custom {
                        docs_url: "https://developer.salesforce.com/docs/atlas.en-us.change_data_capture.meta".into(),
                    },
                    event_types: vec!["*".to_string()],
                    replay_window_seconds: 300,
                    setup_instructions: "CDC uses Salesforce Streaming API (CometD), not HTTP webhooks.".into(),
                },
            ],
            capabilities: ConnectorCapabilities {
                sandbox_mode: true,
                vendor_test_mode: true,
                cdc: true,
                incremental: true,
                schema_discovery: true,
            },
            rate_limits: Some(RateLimitDescriptor {
                requests_per_second: None,
                requests_per_minute: None,
                requests_per_hour: None,
                requests_per_day: Some(15000),
                concurrent_requests: Some(25),
            }),
            doc_url: Some("https://developer.salesforce.com/docs".to_string()),
        }
    }

    async fn check(&self, config: &Value) -> Result<ConnectionStatus, ConnectError> {
        let instance_url = Self::instance_url(config)?;
        let access_token = Self::access_token(config)?;

        let url = format!("{}/services/oauth2/userinfo", instance_url);
        match self
            .api_request(reqwest::Method::GET, &url, &access_token, None)
            .await
        {
            Ok(_) => Ok(ConnectionStatus::succeeded()),
            Err(e) => Ok(ConnectionStatus::failed(format!(
                "Salesforce connection failed: {}",
                e
            ))),
        }
    }

    async fn discover(&self, config: &Value) -> Result<DiscoveredCatalog, ConnectError> {
        let instance_url = Self::instance_url(config)?;
        let access_token = Self::access_token(config)?;

        let url = format!("{}/services/data/v59.0/sobjects", instance_url);
        let response = self
            .api_request(reqwest::Method::GET, &url, &access_token, None)
            .await?;

        let sobjects = response
            .get("sobjects")
            .and_then(|s| s.as_array())
            .cloned()
            .unwrap_or_default();

        let streams: Vec<StreamDescriptor> = sobjects
            .into_iter()
            .filter_map(|obj| {
                let name = obj.get("name")?.as_str()?;
                let queryable = obj.get("queryable")?.as_bool()?;
                if !queryable {
                    return None;
                }
                Some(StreamDescriptor {
                    name: name.to_string(),
                    json_schema: json!({
                        "type": "object",
                        "properties": {
                            "Id": { "type": "string" },
                            "LastModifiedDate": { "type": "string" }
                        }
                    }),
                    supported_modes: vec![SyncMode::FullRefresh, SyncMode::IncrementalAppend],
                    cursor_field: Some(vec!["LastModifiedDate".to_string()]),
                    primary_key: Some(vec![vec!["Id".to_string()]]),
                    supports_outbound: obj
                        .get("createable")
                        .and_then(|c| c.as_bool())
                        .unwrap_or(false),
                    source_defined: true,
                })
            })
            .collect();

        Ok(DiscoveredCatalog { streams })
    }

    async fn read(
        &self,
        config: &Value,
        catalog: &ConfiguredCatalog,
        state: Option<&StateBundle>,
        limits: &SyncLimits,
    ) -> Result<MessageStream, ConnectError> {
        let instance_url = Self::instance_url(config)?;
        let access_token = Self::access_token(config)?;
        let max_records = limits.max_rows.unwrap_or(100_000) as usize;

        let mut all_messages: Vec<Result<ConnectorMessage, ConnectError>> = Vec::new();
        let mut total_records = 0usize;

        for configured_stream in &catalog.streams {
            if total_records >= max_records {
                break;
            }

            let stream_name = &configured_stream.stream;
            let cursor_field = configured_stream
                .cursor_field
                .as_ref()
                .and_then(|c| c.first())
                .map(|s| s.as_str())
                .unwrap_or("LastModifiedDate");

            let mut query = format!(
                "SELECT FIELDS(ALL) FROM {} LIMIT {}",
                stream_name,
                max_records - total_records
            );

            if let Some(bundle) = state {
                if let Some(stream_state) = bundle.stream_states.get(stream_name) {
                    if let Some(cursor_value) = stream_state.get(cursor_field).and_then(|v| v.as_str()) {
                        query = format!(
                            "SELECT FIELDS(ALL) FROM {} WHERE {} > '{}' LIMIT {}",
                            stream_name,
                            cursor_field,
                            cursor_value,
                            max_records - total_records
                        );
                    }
                }
            }

            let mut result = self.soql_query(&instance_url, &access_token, &query).await?;

            loop {
                if total_records >= max_records {
                    break;
                }

                if let Some(records) = result.get("records").and_then(|r| r.as_array()) {
                    for record in records {
                        if total_records >= max_records {
                            break;
                        }
                        all_messages.push(Ok(ConnectorMessage::Record(
                            AirbyteRecordMessage::new(stream_name.clone(), record.clone()),
                        )));
                        total_records += 1;
                    }
                }

                let done = result.get("done").and_then(|d| d.as_bool()).unwrap_or(true);
                if done {
                    break;
                }

                if let Some(next_url) = result.get("nextRecordsUrl").and_then(|u| u.as_str()) {
                    result = self.query_next_page(&instance_url, &access_token, next_url).await?;
                } else {
                    break;
                }
            }
        }

        Ok(Box::pin(futures::stream::iter(all_messages)))
    }

    async fn invoke_action(
        &self,
        config: &Value,
        action: &str,
        input: &Value,
        opts: &ActionOpts,
    ) -> Result<Value, ConnectError> {
        let instance_url = Self::instance_url(config)?;
        let access_token = Self::access_token(config)?;

        if opts.dry_run {
            return Ok(json!({
                "id": "dry_run_preview",
                "success": true,
                "_dry_run": true
            }));
        }

        match action {
            "createLead" => {
                let url = format!("{}/services/data/v59.0/sobjects/Lead", instance_url);
                self.api_request(reqwest::Method::POST, &url, &access_token, Some(input))
                    .await
            }
            "createContact" => {
                let url = format!("{}/services/data/v59.0/sobjects/Contact", instance_url);
                self.api_request(reqwest::Method::POST, &url, &access_token, Some(input))
                    .await
            }
            "createOpportunity" => {
                let url = format!("{}/services/data/v59.0/sobjects/Opportunity", instance_url);
                self.api_request(reqwest::Method::POST, &url, &access_token, Some(input))
                    .await
            }
            "createCase" => {
                let url = format!("{}/services/data/v59.0/sobjects/Case", instance_url);
                self.api_request(reqwest::Method::POST, &url, &access_token, Some(input))
                    .await
            }
            "convertLead" => {
                let lead_id = input
                    .get("leadId")
                    .and_then(|l| l.as_str())
                    .ok_or_else(|| ConnectError::InvalidInput("leadId required".into()))?;

                let convert_request = json!({
                    "leadId": lead_id,
                    "convertedStatus": input.get("convertedStatus").unwrap_or(&json!("Closed - Converted")),
                    "doNotCreateOpportunity": input.get("doNotCreateOpportunity").unwrap_or(&json!(false))
                });

                let url = format!(
                    "{}/services/data/v59.0/sobjects/Lead/{}/convert",
                    instance_url, lead_id
                );
                self.api_request(reqwest::Method::POST, &url, &access_token, Some(&convert_request))
                    .await
            }
            "soqlQuery" => {
                let query = input
                    .get("query")
                    .and_then(|q| q.as_str())
                    .ok_or_else(|| ConnectError::InvalidInput("query required".into()))?;
                self.soql_query(&instance_url, &access_token, query).await
            }
            _ => Err(ConnectError::ActionNotFound(action.to_string())),
        }
    }
}
