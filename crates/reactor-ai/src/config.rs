//! AI capability configuration.

use figment::{providers::Env, Figment};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::path::PathBuf;

/// Deployment mode.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Deployment {
    /// Monolith mode: AI runs with in-process auth.
    #[default]
    Monolith,
    /// Microservices mode: AI talks to remote auth service.
    Microservices,
}

/// Provider credentials configuration.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ProvidersConfig {
    /// OpenRouter API key.
    #[serde(default)]
    pub openrouter_api_key: Option<String>,

    /// AWS Access Key ID (for Bedrock).
    #[serde(default)]
    pub aws_access_key_id: Option<String>,

    /// AWS Secret Access Key (for Bedrock).
    #[serde(default)]
    pub aws_secret_access_key: Option<String>,

    /// AWS Session Token (optional, for STS temporary credentials).
    #[serde(default)]
    pub aws_session_token: Option<String>,

    /// AWS Bedrock region (defaults to us-east-1).
    #[serde(default = "default_aws_region")]
    pub aws_bedrock_region: String,

    /// Azure Foundry endpoint.
    #[serde(default)]
    pub azure_foundry_endpoint: Option<String>,

    /// Azure Foundry API key.
    #[serde(default)]
    pub azure_foundry_api_key: Option<String>,

    /// Generic OpenAI-compatible endpoints (key = provider name, value = config).
    #[serde(default)]
    pub openai_compatible: Vec<OpenAiCompatibleConfig>,
}

/// Configuration for a generic OpenAI-compatible provider.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenAiCompatibleConfig {
    /// Provider name (used in model routing).
    pub name: String,
    /// Base URL for the API.
    pub base_url: String,
    /// API key.
    pub api_key: String,
    /// Optional custom headers.
    #[serde(default)]
    pub headers: Vec<(String, String)>,
}

fn default_aws_region() -> String {
    "us-east-1".to_string()
}

/// AI service configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AiConfig {
    /// HTTP bind address.
    #[serde(default = "default_bind")]
    pub bind: SocketAddr,

    /// Deployment mode.
    #[serde(default)]
    pub deployment: Deployment,

    /// Admin token for internal/system operations.
    #[serde(default)]
    pub admin_token: Option<String>,

    /// Provider credentials.
    #[serde(default)]
    pub providers: ProvidersConfig,

    /// Path to registry overlay TOML file (optional).
    #[serde(default)]
    pub registry_overlay: Option<PathBuf>,

    /// URL to fetch registry overlay from (optional).
    #[serde(default)]
    pub registry_url: Option<String>,

    /// Default alias to use when model is not specified.
    #[serde(default)]
    pub default_alias: Option<String>,

    /// Auth service URL (for remote auth client in microservices mode).
    #[serde(default)]
    pub auth_url: Option<String>,

    /// Enable /metrics endpoint.
    #[serde(default)]
    pub metrics: bool,

    /// Log filter.
    #[serde(default = "default_log")]
    pub log: String,

    /// Request timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_bind() -> SocketAddr {
    "127.0.0.1:8090".parse().unwrap()
}

fn default_log() -> String {
    "info".to_string()
}

fn default_timeout() -> u64 {
    120
}

impl AiConfig {
    /// Load configuration from environment.
    pub fn from_env() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Env::prefixed("REACTOR_AI_"))
            .extract()
    }

    /// Check if OpenRouter credentials are configured.
    pub fn has_openrouter(&self) -> bool {
        self.providers.openrouter_api_key.is_some()
    }

    /// Check if Bedrock credentials are configured.
    pub fn has_bedrock(&self) -> bool {
        self.providers.aws_access_key_id.is_some()
            && self.providers.aws_secret_access_key.is_some()
    }

    /// Check if Azure Foundry credentials are configured.
    pub fn has_foundry(&self) -> bool {
        self.providers.azure_foundry_endpoint.is_some()
            && self.providers.azure_foundry_api_key.is_some()
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            bind: default_bind(),
            deployment: Deployment::default(),
            admin_token: None,
            providers: ProvidersConfig::default(),
            registry_overlay: None,
            registry_url: None,
            default_alias: None,
            auth_url: None,
            metrics: false,
            log: default_log(),
            timeout_secs: default_timeout(),
        }
    }
}
