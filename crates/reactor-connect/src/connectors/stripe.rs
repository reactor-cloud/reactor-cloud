//! Stripe connector implementation.
//!
//! Stripe uses API key (PAT) authentication and provides native dry-run support
//! via the Stripe-Idempotency-Key header and test mode API keys.

use crate::descriptor::{
    ActionDescriptor, AuthDescriptor, AuthField, AuthKind, ConnectorCapabilities,
    ConnectorDescriptor, DryRunSupport, IdempotencyHint, RateLimitDescriptor, SideEffectKind,
};
use crate::error::ConnectError;
use crate::protocol::ConnectionStatus;
use crate::runtime::native::NativeConnector;
use crate::runtime::{ActionOpts, RuntimeKind};
use async_trait::async_trait;
use serde_json::json;

/// Stripe connector.
pub struct StripeConnector {
    http: reqwest::Client,
}

impl Default for StripeConnector {
    fn default() -> Self {
        Self::new()
    }
}

impl StripeConnector {
    /// Create a new Stripe connector.
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    fn api_key_from_config(config: &serde_json::Value) -> Result<String, ConnectError> {
        config
            .get("api_key")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| ConnectError::InvalidInput("api_key required".to_string()))
    }

    async fn make_request(
        &self,
        method: reqwest::Method,
        path: &str,
        api_key: &str,
        body: Option<&serde_json::Value>,
        idempotency_key: Option<&str>,
    ) -> Result<serde_json::Value, ConnectError> {
        let url = format!("https://api.stripe.com/v1{}", path);

        let mut req = self
            .http
            .request(method, &url)
            .basic_auth(api_key, Option::<&str>::None)
            .header("Content-Type", "application/x-www-form-urlencoded");

        if let Some(key) = idempotency_key {
            req = req.header("Idempotency-Key", key);
        }

        if let Some(body) = body {
            // Convert JSON to form-urlencoded for Stripe API
            let form_body = json_to_form_body(body);
            req = req.body(form_body);
        }

        let resp = req
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
            let error_msg = body
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            let error_code = body
                .get("error")
                .and_then(|e| e.get("code"))
                .and_then(|c| c.as_str())
                .unwrap_or("stripe_error");
            Err(ConnectError::ActionFailed {
                code: error_code.to_string(),
                cause: error_msg.to_string(),
                suggested_fix: None,
            })
        }
    }
}

/// Convert a JSON object to Stripe's form-urlencoded format.
fn json_to_form_body(value: &serde_json::Value) -> String {
    let mut parts = Vec::new();
    flatten_json(value, "", &mut parts);
    parts.join("&")
}

fn flatten_json(value: &serde_json::Value, prefix: &str, parts: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.to_string()
                } else {
                    format!("{}[{}]", prefix, k)
                };
                flatten_json(v, &key, parts);
            }
        }
        serde_json::Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                let key = format!("{}[{}]", prefix, i);
                flatten_json(v, &key, parts);
            }
        }
        serde_json::Value::String(s) => {
            parts.push(format!(
                "{}={}",
                urlencoding::encode(prefix),
                urlencoding::encode(s)
            ));
        }
        serde_json::Value::Number(n) => {
            parts.push(format!(
                "{}={}",
                urlencoding::encode(prefix),
                n.to_string()
            ));
        }
        serde_json::Value::Bool(b) => {
            parts.push(format!(
                "{}={}",
                urlencoding::encode(prefix),
                if *b { "true" } else { "false" }
            ));
        }
        serde_json::Value::Null => {}
    }
}

#[async_trait]
impl NativeConnector for StripeConnector {
    fn descriptor(&self) -> ConnectorDescriptor {
        ConnectorDescriptor {
            type_id: "stripe".to_string(),
            display_name: "Stripe".to_string(),
            version: "1.0.0".to_string(),
            runtime: RuntimeKind::Native,
            auth: AuthDescriptor {
                kind: AuthKind::PersonalAccessToken {
                    header: "Authorization".to_string(),
                    format: "Bearer {token}".to_string(),
                },
                fields: vec![AuthField {
                    name: "api_key".to_string(),
                    label: "API Key".to_string(),
                    sensitive: true,
                    required: true,
                    description: Some(
                        "Stripe API key (sk_live_* or sk_test_*)".to_string(),
                    ),
                }],
                test: None,
            },
            streams: vec![],
            actions: vec![
                ActionDescriptor {
                    name: "createCustomer".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "email": { "type": "string", "format": "email" },
                            "name": { "type": "string" },
                            "description": { "type": "string" },
                            "metadata": {
                                "type": "object",
                                "additionalProperties": { "type": "string" }
                            }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "email": { "type": "string" },
                            "name": { "type": "string" },
                            "created": { "type": "integer" }
                        }
                    }),
                    side_effects: SideEffectKind::Mutates,
                    dry_run: DryRunSupport::Native,
                    idempotency: Some(IdempotencyHint {
                        key_path: "$.idempotency_key".to_string(),
                        ttl_seconds: 86400,
                    }),
                },
                ActionDescriptor {
                    name: "getCustomer".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["customer_id"],
                        "properties": {
                            "customer_id": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "email": { "type": "string" },
                            "name": { "type": "string" },
                            "created": { "type": "integer" }
                        }
                    }),
                    side_effects: SideEffectKind::Reads,
                    dry_run: DryRunSupport::Native,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "listCustomers".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "properties": {
                            "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 10 },
                            "starting_after": { "type": "string" },
                            "email": { "type": "string" }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "data": {
                                "type": "array",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "id": { "type": "string" },
                                        "email": { "type": "string" }
                                    }
                                }
                            },
                            "has_more": { "type": "boolean" }
                        }
                    }),
                    side_effects: SideEffectKind::Reads,
                    dry_run: DryRunSupport::Native,
                    idempotency: None,
                },
                ActionDescriptor {
                    name: "createPaymentIntent".to_string(),
                    input_schema: json!({
                        "type": "object",
                        "required": ["amount", "currency"],
                        "properties": {
                            "amount": { "type": "integer", "minimum": 1, "description": "Amount in cents" },
                            "currency": { "type": "string", "pattern": "^[a-z]{3}$" },
                            "customer": { "type": "string" },
                            "description": { "type": "string" },
                            "metadata": {
                                "type": "object",
                                "additionalProperties": { "type": "string" }
                            }
                        }
                    }),
                    output_schema: json!({
                        "type": "object",
                        "properties": {
                            "id": { "type": "string" },
                            "amount": { "type": "integer" },
                            "currency": { "type": "string" },
                            "status": { "type": "string" },
                            "client_secret": { "type": "string" }
                        }
                    }),
                    side_effects: SideEffectKind::Mutates,
                    dry_run: DryRunSupport::Native,
                    idempotency: Some(IdempotencyHint {
                        key_path: "$.idempotency_key".to_string(),
                        ttl_seconds: 86400,
                    }),
                },
            ],
            webhooks: vec![],
            capabilities: ConnectorCapabilities {
                sandbox_mode: true,
                vendor_test_mode: true,
                cdc: false,
                incremental: false,
                schema_discovery: false,
            },
            rate_limits: Some(RateLimitDescriptor {
                requests_per_second: Some(100),
                requests_per_minute: None,
                requests_per_hour: None,
                requests_per_day: None,
                concurrent_requests: Some(200),
            }),
            doc_url: Some("https://stripe.com/docs/api".to_string()),
        }
    }

    async fn check(&self, config: &serde_json::Value) -> Result<ConnectionStatus, ConnectError> {
        let api_key = Self::api_key_from_config(config)?;

        // Call /v1/balance to verify credentials
        match self
            .make_request(reqwest::Method::GET, "/balance", &api_key, None, None)
            .await
        {
            Ok(_) => Ok(ConnectionStatus::succeeded()),
            Err(ConnectError::ActionFailed { cause, .. }) => Ok(ConnectionStatus::failed(format!(
                "Authentication failed: {}. Verify your Stripe API key is correct and active.",
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

        // For dry-run with test mode keys, Stripe handles it natively
        let idempotency_key = opts.idempotency_key.as_deref();

        match action {
            "createCustomer" => {
                if opts.dry_run && !api_key.starts_with("sk_test_") {
                    // Synthesize response for live mode dry-run
                    return Ok(json!({
                        "id": "cus_dry_run_preview",
                        "email": input.get("email"),
                        "name": input.get("name"),
                        "created": chrono::Utc::now().timestamp(),
                        "_dry_run": true
                    }));
                }

                self.make_request(
                    reqwest::Method::POST,
                    "/customers",
                    &api_key,
                    Some(input),
                    idempotency_key,
                )
                .await
            }
            "getCustomer" => {
                let customer_id = input
                    .get("customer_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ConnectError::InvalidInput("customer_id required".to_string())
                    })?;

                self.make_request(
                    reqwest::Method::GET,
                    &format!("/customers/{}", customer_id),
                    &api_key,
                    None,
                    None,
                )
                .await
            }
            "listCustomers" => {
                let mut path = "/customers".to_string();
                let mut query_parts = Vec::new();

                if let Some(limit) = input.get("limit").and_then(|v| v.as_i64()) {
                    query_parts.push(format!("limit={}", limit));
                }
                if let Some(after) = input.get("starting_after").and_then(|v| v.as_str()) {
                    query_parts.push(format!("starting_after={}", after));
                }
                if let Some(email) = input.get("email").and_then(|v| v.as_str()) {
                    query_parts.push(format!("email={}", urlencoding::encode(email)));
                }

                if !query_parts.is_empty() {
                    path = format!("{}?{}", path, query_parts.join("&"));
                }

                self.make_request(reqwest::Method::GET, &path, &api_key, None, None)
                    .await
            }
            "createPaymentIntent" => {
                if opts.dry_run && !api_key.starts_with("sk_test_") {
                    return Ok(json!({
                        "id": "pi_dry_run_preview",
                        "amount": input.get("amount"),
                        "currency": input.get("currency"),
                        "status": "requires_payment_method",
                        "client_secret": "pi_dry_run_secret",
                        "_dry_run": true
                    }));
                }

                self.make_request(
                    reqwest::Method::POST,
                    "/payment_intents",
                    &api_key,
                    Some(input),
                    idempotency_key,
                )
                .await
            }
            _ => Err(ConnectError::ActionNotFound(action.to_string())),
        }
    }
}
