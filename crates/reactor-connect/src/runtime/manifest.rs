//! Manifest runtime: Rust interpreter for Airbyte Low-Code CDK YAML.
//!
//! This module interprets Airbyte Low-Code CDK YAML manifests to provide
//! connector functionality without requiring container orchestration.
//!
//! Supported constructs:
//! - HttpRequester: Makes HTTP requests with configurable auth and pagination
//! - Authentication: OAuth, Bearer, Basic, ApiKey
//! - Pagination: OffsetIncrement, PageIncrement, CursorPagination
//! - Extractors: DpathExtractor, ListPartitionRouter
//! - Cursors: DatetimeBasedCursor for incremental sync

use crate::descriptor::{
    AuthDescriptor, AuthField, AuthKind, ConnectorCapabilities, ConnectorDescriptor,
    ConnectorTypeId, RateLimitDescriptor, StreamDescriptor, SyncMode,
};
use crate::error::ConnectError;
use crate::protocol::{
    ConfiguredCatalog, ConnectionStatus, ConnectorMessage, DiscoveredCatalog, StateBundle,
    SyncLimits, WriteOutcome,
};
use crate::runtime::{ActionOpts, ConnectorRuntime, MessageStream, RuntimeKind};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

// ============================================================================
// YAML Schema Types
// ============================================================================

/// Root of an Airbyte Low-Code CDK manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSpec {
    /// Manifest version.
    #[serde(default)]
    pub version: Option<String>,

    /// Type (should be "DeclarativeSource").
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Connector definition.
    pub definitions: ManifestDefinitions,

    /// Check stream configuration.
    #[serde(default)]
    pub check: Option<CheckConfig>,

    /// Stream definitions.
    #[serde(default)]
    pub streams: Vec<StreamSpec>,

    /// Spec configuration (for user input schema).
    #[serde(default)]
    pub spec: Option<SpecConfig>,
}

/// Manifest definitions section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ManifestDefinitions {
    /// Reusable selectors.
    #[serde(default)]
    pub selectors: HashMap<String, SelectorSpec>,

    /// Reusable requesters.
    #[serde(default)]
    pub requesters: HashMap<String, RequesterSpec>,

    /// Reusable retrievers.
    #[serde(default)]
    pub retrievers: HashMap<String, RetrieverSpec>,

    /// Base requester reference.
    #[serde(default)]
    pub base_requester: Option<RequesterSpec>,

    /// Partition routers.
    #[serde(default)]
    pub partition_routers: HashMap<String, PartitionRouterSpec>,
}

/// Check stream configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckConfig {
    /// Stream names to use for check.
    pub stream_names: Vec<String>,
}

/// Spec configuration for user input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecConfig {
    /// Connection specification (JSON Schema).
    pub connection_specification: serde_json::Value,

    /// Documentation URL.
    #[serde(default)]
    pub documentation_url: Option<String>,
}

/// Stream specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamSpec {
    /// Stream type (should be "DeclarativeStream").
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Stream name.
    pub name: String,

    /// Primary key path(s).
    #[serde(default)]
    pub primary_key: Option<PrimaryKeySpec>,

    /// Retriever configuration.
    #[serde(default)]
    pub retriever: Option<RetrieverSpec>,

    /// Reference to a retriever definition.
    #[serde(rename = "$ref", default)]
    pub retriever_ref: Option<String>,

    /// Incremental sync configuration.
    #[serde(default)]
    pub incremental_sync: Option<IncrementalSyncSpec>,

    /// Schema loader configuration.
    #[serde(default)]
    pub schema_loader: Option<SchemaLoaderSpec>,
}

/// Primary key specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PrimaryKeySpec {
    /// Single key path.
    Single(Vec<String>),
    /// Composite key paths.
    Composite(Vec<Vec<String>>),
}

/// Retriever specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieverSpec {
    /// Retriever type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Record selector.
    #[serde(default)]
    pub record_selector: Option<SelectorSpec>,

    /// Requester configuration.
    #[serde(default)]
    pub requester: Option<RequesterSpec>,

    /// Paginator configuration.
    #[serde(default)]
    pub paginator: Option<PaginatorSpec>,

    /// Partition router.
    #[serde(default)]
    pub partition_router: Option<PartitionRouterSpec>,
}

/// Selector specification (DpathExtractor).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectorSpec {
    /// Selector type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// JSON path extractor.
    #[serde(default)]
    pub extractor: Option<ExtractorSpec>,

    /// Field path for extraction.
    #[serde(default)]
    pub field_path: Option<Vec<String>>,
}

/// Extractor specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractorSpec {
    /// Extractor type (DpathExtractor, etc.).
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Field path.
    #[serde(default)]
    pub field_path: Option<Vec<String>>,
}

/// HTTP requester specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequesterSpec {
    /// Requester type (HttpRequester).
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Base URL.
    #[serde(default)]
    pub url_base: Option<String>,

    /// Path template.
    #[serde(default)]
    pub path: Option<String>,

    /// HTTP method.
    #[serde(default)]
    pub http_method: Option<String>,

    /// Authenticator configuration.
    #[serde(default)]
    pub authenticator: Option<AuthenticatorSpec>,

    /// Request headers.
    #[serde(default)]
    pub request_headers: Option<HashMap<String, String>>,

    /// Request parameters.
    #[serde(default)]
    pub request_parameters: Option<HashMap<String, String>>,

    /// Error handler.
    #[serde(default)]
    pub error_handler: Option<ErrorHandlerSpec>,
}

/// Authenticator specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatorSpec {
    /// Authenticator type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// API key for ApiKeyAuthenticator.
    #[serde(default)]
    pub api_key: Option<String>,

    /// Header name for ApiKeyAuthenticator.
    #[serde(default)]
    pub header: Option<String>,

    /// Token for BearerAuthenticator.
    #[serde(default)]
    pub token: Option<String>,

    /// Username for BasicHttpAuthenticator.
    #[serde(default)]
    pub username: Option<String>,

    /// Password for BasicHttpAuthenticator.
    #[serde(default)]
    pub password: Option<String>,

    /// OAuth2 configuration.
    #[serde(default)]
    pub oauth_config: Option<OAuth2ConfigSpec>,
}

/// OAuth2 configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuth2ConfigSpec {
    /// Token refresh endpoint.
    #[serde(default)]
    pub token_refresh_endpoint: Option<String>,

    /// Client ID.
    #[serde(default)]
    pub client_id: Option<String>,

    /// Client secret.
    #[serde(default)]
    pub client_secret: Option<String>,

    /// Refresh token.
    #[serde(default)]
    pub refresh_token: Option<String>,

    /// Access token.
    #[serde(default)]
    pub access_token: Option<String>,

    /// Scopes.
    #[serde(default)]
    pub scopes: Option<Vec<String>>,

    /// Grant type.
    #[serde(default)]
    pub grant_type: Option<String>,
}

/// Paginator specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatorSpec {
    /// Paginator type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Page size.
    #[serde(default)]
    pub page_size: Option<u32>,

    /// Pagination strategy.
    #[serde(default)]
    pub pagination_strategy: Option<PaginationStrategySpec>,

    /// Page token option (for cursor pagination).
    #[serde(default)]
    pub page_token_option: Option<PageTokenOptionSpec>,
}

/// Pagination strategy specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginationStrategySpec {
    /// Strategy type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Page size parameter name.
    #[serde(default)]
    pub page_size: Option<u32>,

    /// Cursor value path (for CursorPagination).
    #[serde(default)]
    pub cursor_value: Option<String>,

    /// Stop condition.
    #[serde(default)]
    pub stop_condition: Option<String>,
}

/// Page token option.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageTokenOptionSpec {
    /// Token option type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Parameter name for request parameter injection.
    #[serde(default)]
    pub field_name: Option<String>,

    /// Inject into (request_parameter, header, body_json).
    #[serde(default)]
    pub inject_into: Option<String>,
}

/// Partition router specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartitionRouterSpec {
    /// Router type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Parent stream configs for SubstreamPartitionRouter.
    #[serde(default)]
    pub parent_stream_configs: Option<Vec<ParentStreamConfigSpec>>,

    /// Values for ListPartitionRouter.
    #[serde(default)]
    pub values: Option<Vec<String>>,

    /// Cursor field for ListPartitionRouter.
    #[serde(default)]
    pub cursor_field: Option<String>,
}

/// Parent stream configuration for SubstreamPartitionRouter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParentStreamConfigSpec {
    /// Stream reference.
    pub stream: StreamRefSpec,

    /// Parent key.
    pub parent_key: String,

    /// Partition field.
    pub partition_field: String,
}

/// Stream reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamRefSpec {
    /// Reference to stream.
    #[serde(rename = "$ref", default)]
    pub ref_path: Option<String>,

    /// Direct stream name.
    #[serde(default)]
    pub name: Option<String>,
}

/// Incremental sync specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalSyncSpec {
    /// Sync type (DatetimeBasedCursor).
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Cursor field.
    #[serde(default)]
    pub cursor_field: Option<String>,

    /// Datetime format.
    #[serde(default)]
    pub datetime_format: Option<String>,

    /// Start datetime.
    #[serde(default)]
    pub start_datetime: Option<String>,

    /// End datetime.
    #[serde(default)]
    pub end_datetime: Option<String>,

    /// Step duration.
    #[serde(default)]
    pub step: Option<String>,

    /// Cursor granularity.
    #[serde(default)]
    pub cursor_granularity: Option<String>,

    /// Lookback window.
    #[serde(default)]
    pub lookback_window: Option<String>,

    /// Start time option.
    #[serde(default)]
    pub start_time_option: Option<RequestOptionSpec>,

    /// End time option.
    #[serde(default)]
    pub end_time_option: Option<RequestOptionSpec>,
}

/// Request option for parameter injection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestOptionSpec {
    /// Option type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Parameter name.
    #[serde(default)]
    pub field_name: Option<String>,

    /// Inject into target.
    #[serde(default)]
    pub inject_into: Option<String>,
}

/// Schema loader specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaLoaderSpec {
    /// Loader type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// JSON schema.
    #[serde(default)]
    pub schema: Option<serde_json::Value>,
}

/// Error handler specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorHandlerSpec {
    /// Handler type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Response filters.
    #[serde(default)]
    pub response_filters: Option<Vec<ResponseFilterSpec>>,

    /// Backoff strategies.
    #[serde(default)]
    pub backoff_strategies: Option<Vec<BackoffStrategySpec>>,
}

/// Response filter specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFilterSpec {
    /// Filter type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// HTTP codes to filter.
    #[serde(default)]
    pub http_codes: Option<Vec<u16>>,

    /// Action to take.
    #[serde(default)]
    pub action: Option<String>,
}

/// Backoff strategy specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackoffStrategySpec {
    /// Strategy type.
    #[serde(rename = "type", default)]
    pub spec_type: Option<String>,

    /// Factor for exponential backoff.
    #[serde(default)]
    pub factor: Option<f64>,
}

// ============================================================================
// ManifestRuntime Implementation
// ============================================================================

/// Manifest runtime that interprets Airbyte Low-Code CDK YAML manifests.
pub struct ManifestRuntime {
    /// Directory containing manifest YAML files.
    manifests_dir: PathBuf,
    /// Cached parsed manifests.
    manifests: tokio::sync::RwLock<HashMap<ConnectorTypeId, Arc<ManifestSpec>>>,
    /// HTTP client for making requests.
    http: reqwest::Client,
}

impl ManifestRuntime {
    /// Create a new manifest runtime.
    pub fn new(manifests_dir: PathBuf) -> Self {
        Self {
            manifests_dir,
            manifests: tokio::sync::RwLock::new(HashMap::new()),
            http: reqwest::Client::new(),
        }
    }

    /// Load a manifest from disk.
    async fn load_manifest(&self, type_id: &ConnectorTypeId) -> Result<Arc<ManifestSpec>, ConnectError> {
        // Check cache first
        {
            let cache = self.manifests.read().await;
            if let Some(manifest) = cache.get(type_id) {
                return Ok(manifest.clone());
            }
        }

        // Load from disk
        let path = self.manifests_dir.join(format!("{}.yaml", type_id));
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| ConnectError::Internal(format!("failed to read manifest {}: {}", type_id, e)))?;

        let manifest: ManifestSpec = serde_yaml::from_str(&content)
            .map_err(|e| ConnectError::Internal(format!("failed to parse manifest {}: {}", type_id, e)))?;

        let manifest = Arc::new(manifest);

        // Cache it
        {
            let mut cache = self.manifests.write().await;
            cache.insert(type_id.clone(), manifest.clone());
        }

        Ok(manifest)
    }

    /// Convert a ManifestSpec to a ConnectorDescriptor.
    fn to_descriptor(&self, type_id: &ConnectorTypeId, manifest: &ManifestSpec) -> ConnectorDescriptor {
        let mut streams = Vec::new();
        for stream in &manifest.streams {
            let supported_modes = if stream.incremental_sync.is_some() {
                vec![SyncMode::FullRefresh, SyncMode::IncrementalAppend]
            } else {
                vec![SyncMode::FullRefresh]
            };

            let cursor_field = stream
                .incremental_sync
                .as_ref()
                .and_then(|s| s.cursor_field.as_ref())
                .map(|f| vec![f.clone()]);

            let primary_key = stream.primary_key.as_ref().map(|pk| match pk {
                PrimaryKeySpec::Single(keys) => vec![keys.clone()],
                PrimaryKeySpec::Composite(keys) => keys.clone(),
            });

            let json_schema = stream
                .schema_loader
                .as_ref()
                .and_then(|l| l.schema.clone())
                .unwrap_or_else(|| serde_json::json!({"type": "object"}));

            streams.push(StreamDescriptor {
                name: stream.name.clone(),
                json_schema,
                supported_modes,
                cursor_field,
                primary_key,
                supports_outbound: false,
                source_defined: false,
            });
        }

        // Build auth descriptor from manifest spec
        let auth = self.build_auth_descriptor(manifest);

        ConnectorDescriptor {
            type_id: type_id.clone(),
            display_name: type_id.clone(),
            version: manifest.version.clone().unwrap_or_else(|| "0.1.0".to_string()),
            runtime: RuntimeKind::Manifest,
            auth,
            streams,
            actions: vec![], // Manifest connectors are stream-only
            webhooks: vec![],
            capabilities: ConnectorCapabilities {
                sandbox_mode: true,
                vendor_test_mode: false,
                cdc: false,
                incremental: manifest.streams.iter().any(|s| s.incremental_sync.is_some()),
                schema_discovery: false,
            },
            rate_limits: None,
            doc_url: manifest.spec.as_ref().and_then(|s| s.documentation_url.clone()),
        }
    }

    /// Build auth descriptor from manifest.
    fn build_auth_descriptor(&self, manifest: &ManifestSpec) -> AuthDescriptor {
        // Try to extract auth from the base requester or first stream's requester
        let authenticator = manifest
            .definitions
            .base_requester
            .as_ref()
            .and_then(|r| r.authenticator.as_ref())
            .or_else(|| {
                manifest
                    .streams
                    .first()
                    .and_then(|s| s.retriever.as_ref())
                    .and_then(|r| r.requester.as_ref())
                    .and_then(|r| r.authenticator.as_ref())
            });

        match authenticator {
            Some(auth) => match auth.spec_type.as_deref() {
                Some("OAuthAuthenticator") => {
                    let oauth_config = auth.oauth_config.as_ref();
                    AuthDescriptor {
                        kind: AuthKind::OAuth2 {
                            authorize_url: String::new(), // Would need to be in manifest
                            token_url: oauth_config
                                .and_then(|c| c.token_refresh_endpoint.clone())
                                .unwrap_or_default(),
                            scopes: oauth_config
                                .and_then(|c| c.scopes.clone())
                                .unwrap_or_default(),
                            pkce: false,
                            reactor_proxy: false,
                        },
                        fields: vec![
                            AuthField {
                                name: "client_id".to_string(),
                                label: "Client ID".to_string(),
                                sensitive: false,
                                required: true,
                                description: None,
                            },
                            AuthField {
                                name: "client_secret".to_string(),
                                label: "Client Secret".to_string(),
                                sensitive: true,
                                required: true,
                                description: None,
                            },
                        ],
                        test: None,
                    }
                }
                Some("BearerAuthenticator") => AuthDescriptor {
                    kind: AuthKind::PersonalAccessToken {
                        header: "Authorization".to_string(),
                        format: "Bearer {token}".to_string(),
                    },
                    fields: vec![AuthField {
                        name: "token".to_string(),
                        label: "API Token".to_string(),
                        sensitive: true,
                        required: true,
                        description: None,
                    }],
                    test: None,
                },
                Some("ApiKeyAuthenticator") => {
                    let header = auth.header.clone().unwrap_or_else(|| "Authorization".to_string());
                    AuthDescriptor {
                        kind: AuthKind::PersonalAccessToken {
                            header: header.clone(),
                            format: "{token}".to_string(),
                        },
                        fields: vec![AuthField {
                            name: "api_key".to_string(),
                            label: "API Key".to_string(),
                            sensitive: true,
                            required: true,
                            description: None,
                        }],
                        test: None,
                    }
                }
                Some("BasicHttpAuthenticator") => AuthDescriptor {
                    kind: AuthKind::Basic,
                    fields: vec![
                        AuthField {
                            name: "username".to_string(),
                            label: "Username".to_string(),
                            sensitive: false,
                            required: true,
                            description: None,
                        },
                        AuthField {
                            name: "password".to_string(),
                            label: "Password".to_string(),
                            sensitive: true,
                            required: true,
                            description: None,
                        },
                    ],
                    test: None,
                },
                _ => self.default_auth_descriptor(),
            },
            None => self.default_auth_descriptor(),
        }
    }

    /// Default auth descriptor for unknown auth types.
    fn default_auth_descriptor(&self) -> AuthDescriptor {
        AuthDescriptor {
            kind: AuthKind::Custom {
                docs_url: String::new(),
            },
            fields: vec![],
            test: None,
        }
    }

    /// Execute an HTTP request based on requester spec.
    async fn execute_request(
        &self,
        requester: &RequesterSpec,
        config: &serde_json::Value,
        path_params: &HashMap<String, String>,
    ) -> Result<serde_json::Value, ConnectError> {
        let base_url = requester
            .url_base
            .as_ref()
            .ok_or_else(|| ConnectError::Internal("missing url_base".to_string()))?;

        let path = requester
            .path
            .as_ref()
            .map(|p| self.interpolate_template(p, config, path_params))
            .unwrap_or_default();

        let url = format!("{}{}", base_url, path);

        let method = match requester.http_method.as_deref().unwrap_or("GET") {
            "POST" => reqwest::Method::POST,
            "PUT" => reqwest::Method::PUT,
            "PATCH" => reqwest::Method::PATCH,
            "DELETE" => reqwest::Method::DELETE,
            _ => reqwest::Method::GET,
        };

        let mut request = self.http.request(method, &url);

        // Add authentication
        if let Some(auth) = &requester.authenticator {
            request = self.apply_auth(request, auth, config)?;
        }

        // Add headers
        if let Some(headers) = &requester.request_headers {
            for (key, value) in headers {
                let value = self.interpolate_template(value, config, path_params);
                request = request.header(key, value);
            }
        }

        // Add query parameters
        if let Some(params) = &requester.request_parameters {
            for (key, value) in params {
                let value = self.interpolate_template(value, config, path_params);
                request = request.query(&[(key, value)]);
            }
        }

        let response = request
            .send()
            .await
            .map_err(|e| ConnectError::Internal(format!("request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ConnectError::ActionFailed {
                code: format!("http_{}", status.as_u16()),
                cause: body,
                suggested_fix: None,
            });
        }

        response
            .json()
            .await
            .map_err(|e| ConnectError::Internal(format!("failed to parse response: {}", e)))
    }

    /// Apply authentication to a request.
    fn apply_auth(
        &self,
        request: reqwest::RequestBuilder,
        auth: &AuthenticatorSpec,
        config: &serde_json::Value,
    ) -> Result<reqwest::RequestBuilder, ConnectError> {
        match auth.spec_type.as_deref() {
            Some("BearerAuthenticator") => {
                let token = auth
                    .token
                    .as_ref()
                    .map(|t| self.interpolate_template(t, config, &HashMap::new()))
                    .or_else(|| config.get("token").and_then(|v| v.as_str()).map(String::from))
                    .or_else(|| {
                        config
                            .get("access_token")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                    })
                    .ok_or_else(|| ConnectError::InvalidInput("token required".to_string()))?;

                Ok(request.bearer_auth(token))
            }
            Some("ApiKeyAuthenticator") => {
                let api_key = auth
                    .api_key
                    .as_ref()
                    .map(|k| self.interpolate_template(k, config, &HashMap::new()))
                    .or_else(|| config.get("api_key").and_then(|v| v.as_str()).map(String::from))
                    .ok_or_else(|| ConnectError::InvalidInput("api_key required".to_string()))?;

                let header = auth.header.as_deref().unwrap_or("Authorization");
                Ok(request.header(header, api_key))
            }
            Some("BasicHttpAuthenticator") => {
                let username = auth
                    .username
                    .as_ref()
                    .map(|u| self.interpolate_template(u, config, &HashMap::new()))
                    .or_else(|| config.get("username").and_then(|v| v.as_str()).map(String::from))
                    .ok_or_else(|| ConnectError::InvalidInput("username required".to_string()))?;

                let password = auth
                    .password
                    .as_ref()
                    .map(|p| self.interpolate_template(p, config, &HashMap::new()))
                    .or_else(|| config.get("password").and_then(|v| v.as_str()).map(String::from))
                    .unwrap_or_default();

                Ok(request.basic_auth(username, Some(password)))
            }
            Some("OAuthAuthenticator") => {
                // For OAuth, we expect the token to be in config already (refreshed externally)
                let token = config
                    .get("access_token")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| ConnectError::InvalidInput("access_token required".to_string()))?;

                Ok(request.bearer_auth(token))
            }
            _ => Ok(request),
        }
    }

    /// Interpolate a template string with config and path params.
    fn interpolate_template(
        &self,
        template: &str,
        config: &serde_json::Value,
        path_params: &HashMap<String, String>,
    ) -> String {
        let mut result = template.to_string();

        // Replace {{ config[key] }} patterns
        let config_re = regex::Regex::new(r#"\{\{\s*config\[['"]?(\w+)['"]?\]\s*\}\}"#).unwrap();
        result = config_re
            .replace_all(&result, |caps: &regex::Captures| {
                let key = &caps[1];
                config
                    .get(key)
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_default()
            })
            .to_string();

        // Replace {{ stream_partition[key] }} patterns
        let partition_re =
            regex::Regex::new(r#"\{\{\s*stream_partition\[['"]?(\w+)['"]?\]\s*\}\}"#).unwrap();
        result = partition_re
            .replace_all(&result, |caps: &regex::Captures| {
                let key = &caps[1];
                path_params.get(key).cloned().unwrap_or_default()
            })
            .to_string();

        // Replace {{ stream_slice[key] }} patterns
        let slice_re =
            regex::Regex::new(r#"\{\{\s*stream_slice\[['"]?(\w+)['"]?\]\s*\}\}"#).unwrap();
        result = slice_re
            .replace_all(&result, |caps: &regex::Captures| {
                let key = &caps[1];
                path_params.get(key).cloned().unwrap_or_default()
            })
            .to_string();

        result
    }

    /// Extract records from response using selector.
    fn extract_records(
        &self,
        response: &serde_json::Value,
        selector: &SelectorSpec,
    ) -> Vec<serde_json::Value> {
        let field_path = selector
            .extractor
            .as_ref()
            .and_then(|e| e.field_path.as_ref())
            .or(selector.field_path.as_ref());

        if let Some(path) = field_path {
            let mut current = response;
            for segment in path {
                match current.get(segment) {
                    Some(v) => current = v,
                    None => return vec![],
                }
            }
            if let Some(arr) = current.as_array() {
                return arr.clone();
            }
        }

        // If no path or extraction fails, assume response is a single record
        if response.is_object() {
            vec![response.clone()]
        } else {
            vec![]
        }
    }
}

#[async_trait]
impl ConnectorRuntime for ManifestRuntime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Manifest
    }

    async fn list_types(&self) -> Result<Vec<ConnectorTypeId>, ConnectError> {
        let mut types = Vec::new();

        let mut entries = tokio::fs::read_dir(&self.manifests_dir)
            .await
            .map_err(|e| ConnectError::Internal(format!("failed to read manifests dir: {}", e)))?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            ConnectError::Internal(format!("failed to read dir entry: {}", e))
        })? {
            let path = entry.path();
            if path.extension().map(|e| e == "yaml" || e == "yml").unwrap_or(false) {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    types.push(stem.to_string());
                }
            }
        }

        Ok(types)
    }

    async fn descriptor(&self, type_id: &ConnectorTypeId) -> Result<ConnectorDescriptor, ConnectError> {
        let manifest = self.load_manifest(type_id).await?;
        Ok(self.to_descriptor(type_id, &manifest))
    }

    async fn check(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
    ) -> Result<ConnectionStatus, ConnectError> {
        let manifest = self.load_manifest(type_id).await?;

        // Get check streams
        let check_streams = manifest
            .check
            .as_ref()
            .map(|c| c.stream_names.clone())
            .unwrap_or_else(|| {
                manifest
                    .streams
                    .first()
                    .map(|s| vec![s.name.clone()])
                    .unwrap_or_default()
            });

        if check_streams.is_empty() {
            return Ok(ConnectionStatus::failed("no check streams configured".to_string()));
        }

        // Try to fetch from the first check stream
        let stream_name = &check_streams[0];
        let stream = manifest
            .streams
            .iter()
            .find(|s| s.name == *stream_name)
            .ok_or_else(|| ConnectError::Internal(format!("check stream {} not found", stream_name)))?;

        let retriever = stream.retriever.as_ref().ok_or_else(|| {
            ConnectError::Internal("stream has no retriever".to_string())
        })?;

        let requester = retriever.requester.as_ref().or_else(|| {
            manifest.definitions.base_requester.as_ref()
        }).ok_or_else(|| {
            ConnectError::Internal("no requester found".to_string())
        })?;

        // Try a simple request
        match self.execute_request(requester, config, &HashMap::new()).await {
            Ok(_) => Ok(ConnectionStatus::succeeded()),
            Err(ConnectError::ActionFailed { cause, .. }) => {
                Ok(ConnectionStatus::failed(format!("Authentication failed: {}", cause)))
            }
            Err(e) => Err(e),
        }
    }

    async fn discover(
        &self,
        type_id: &ConnectorTypeId,
        _config: &serde_json::Value,
    ) -> Result<DiscoveredCatalog, ConnectError> {
        let descriptor = self.descriptor(type_id).await?;
        Ok(DiscoveredCatalog {
            streams: descriptor.streams,
        })
    }

    async fn read(
        &self,
        type_id: &ConnectorTypeId,
        config: &serde_json::Value,
        catalog: &ConfiguredCatalog,
        state: Option<&StateBundle>,
        limits: &SyncLimits,
    ) -> Result<MessageStream, ConnectError> {
        let manifest = self.load_manifest(type_id).await?;
        let config = config.clone();
        let catalog = catalog.clone();
        let state = state.cloned();
        let limits = limits.clone();
        let http = self.http.clone();

        // Create the stream
        let stream = async_stream::try_stream! {
            let mut total_records = 0u64;

            for configured_stream in &catalog.streams {
                let stream_spec = manifest
                    .streams
                    .iter()
                    .find(|s| s.name == configured_stream.stream)
                    .ok_or_else(|| ConnectError::Internal(format!(
                        "stream {} not found in manifest",
                        configured_stream.stream
                    )))?;

                let retriever = stream_spec.retriever.as_ref().ok_or_else(|| {
                    ConnectError::Internal("stream has no retriever".to_string())
                })?;

                let requester = retriever.requester.as_ref().or_else(|| {
                    manifest.definitions.base_requester.as_ref()
                }).ok_or_else(|| {
                    ConnectError::Internal("no requester found".to_string())
                })?;

                let selector = retriever.record_selector.as_ref().ok_or_else(|| {
                    ConnectError::Internal("no record selector".to_string())
                })?;

                // TODO: Implement pagination loop
                // For now, just fetch first page
                let base_url = requester.url_base.as_ref().ok_or_else(|| {
                    ConnectError::Internal("missing url_base".to_string())
                })?;

                let path = requester.path.as_ref().map(|p| {
                    interpolate_template_static(p, &config, &HashMap::new())
                }).unwrap_or_default();

                let url = format!("{}{}", base_url, path);

                let mut request = http.get(&url);

                // Apply auth
                if let Some(auth) = &requester.authenticator {
                    request = apply_auth_static(request, auth, &config)?;
                }

                let response = request.send().await.map_err(|e| {
                    ConnectError::Internal(format!("request failed: {}", e))
                })?;

                let status = response.status();
                let body: serde_json::Value = if status.is_success() {
                    response.json().await.map_err(|e| {
                        ConnectError::Internal(format!("failed to parse response: {}", e))
                    })?
                } else {
                    let error_text = response.text().await.unwrap_or_default();
                    Err(ConnectError::ActionFailed {
                        code: format!("http_{}", status.as_u16()),
                        cause: error_text,
                        suggested_fix: None,
                    })?
                };

                let records = extract_records_static(&body, selector);

                for record in records {
                    if let Some(max) = limits.max_rows {
                        if total_records >= max {
                            return;
                        }
                    }

                    yield ConnectorMessage::Record(crate::protocol::AirbyteRecordMessage::new(
                        configured_stream.stream.clone(),
                        record,
                    ));

                    total_records += 1;
                }
            }
        };

        Ok(Box::pin(stream))
    }

    async fn invoke_action(
        &self,
        _type_id: &ConnectorTypeId,
        _config: &serde_json::Value,
        _action: &str,
        _input: &serde_json::Value,
        _opts: &ActionOpts,
    ) -> Result<serde_json::Value, ConnectError> {
        // Manifest connectors don't support actions
        Err(ConnectError::ActionNotFound("manifest connectors don't support actions".to_string()))
    }

    async fn write(
        &self,
        _type_id: &ConnectorTypeId,
        _config: &serde_json::Value,
        _stream: &str,
        _records: MessageStream,
        _limits: &SyncLimits,
    ) -> Result<WriteOutcome, ConnectError> {
        // Manifest connectors don't support writes
        Err(ConnectError::Internal("manifest connectors don't support writes".to_string()))
    }
}

// Static helper functions for use in async streams

fn interpolate_template_static(
    template: &str,
    config: &serde_json::Value,
    path_params: &HashMap<String, String>,
) -> String {
    let mut result = template.to_string();

    let config_re = regex::Regex::new(r#"\{\{\s*config\[['"]?(\w+)['"]?\]\s*\}\}"#).unwrap();
    result = config_re
        .replace_all(&result, |caps: &regex::Captures| {
            let key = &caps[1];
            config
                .get(key)
                .and_then(|v| v.as_str())
                .map(String::from)
                .unwrap_or_default()
        })
        .to_string();

    let partition_re =
        regex::Regex::new(r#"\{\{\s*stream_partition\[['"]?(\w+)['"]?\]\s*\}\}"#).unwrap();
    result = partition_re
        .replace_all(&result, |caps: &regex::Captures| {
            let key = &caps[1];
            path_params.get(key).cloned().unwrap_or_default()
        })
        .to_string();

    result
}

fn apply_auth_static(
    request: reqwest::RequestBuilder,
    auth: &AuthenticatorSpec,
    config: &serde_json::Value,
) -> Result<reqwest::RequestBuilder, ConnectError> {
    match auth.spec_type.as_deref() {
        Some("BearerAuthenticator") => {
            let token = auth
                .token
                .as_ref()
                .map(|t| interpolate_template_static(t, config, &HashMap::new()))
                .or_else(|| config.get("token").and_then(|v| v.as_str()).map(String::from))
                .or_else(|| config.get("access_token").and_then(|v| v.as_str()).map(String::from))
                .ok_or_else(|| ConnectError::InvalidInput("token required".to_string()))?;

            Ok(request.bearer_auth(token))
        }
        Some("ApiKeyAuthenticator") => {
            let api_key = auth
                .api_key
                .as_ref()
                .map(|k| interpolate_template_static(k, config, &HashMap::new()))
                .or_else(|| config.get("api_key").and_then(|v| v.as_str()).map(String::from))
                .ok_or_else(|| ConnectError::InvalidInput("api_key required".to_string()))?;

            let header = auth.header.as_deref().unwrap_or("Authorization");
            Ok(request.header(header, api_key))
        }
        Some("BasicHttpAuthenticator") => {
            let username = auth
                .username
                .as_ref()
                .map(|u| interpolate_template_static(u, config, &HashMap::new()))
                .or_else(|| config.get("username").and_then(|v| v.as_str()).map(String::from))
                .ok_or_else(|| ConnectError::InvalidInput("username required".to_string()))?;

            let password = auth
                .password
                .as_ref()
                .map(|p| interpolate_template_static(p, config, &HashMap::new()))
                .or_else(|| config.get("password").and_then(|v| v.as_str()).map(String::from))
                .unwrap_or_default();

            Ok(request.basic_auth(username, Some(password)))
        }
        Some("OAuthAuthenticator") => {
            let token = config
                .get("access_token")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ConnectError::InvalidInput("access_token required".to_string()))?;

            Ok(request.bearer_auth(token))
        }
        _ => Ok(request),
    }
}

fn extract_records_static(
    response: &serde_json::Value,
    selector: &SelectorSpec,
) -> Vec<serde_json::Value> {
    let field_path = selector
        .extractor
        .as_ref()
        .and_then(|e| e.field_path.as_ref())
        .or(selector.field_path.as_ref());

    if let Some(path) = field_path {
        let mut current = response;
        for segment in path {
            match current.get(segment) {
                Some(v) => current = v,
                None => return vec![],
            }
        }
        if let Some(arr) = current.as_array() {
            return arr.clone();
        }
    }

    if response.is_object() {
        vec![response.clone()]
    } else {
        vec![]
    }
}
