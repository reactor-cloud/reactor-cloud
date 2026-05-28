//! Job manifest types.

use serde::{Deserialize, Serialize};

mod validate;

pub use validate::validate_manifest;

/// Trigger kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TriggerKind {
    /// Cron-scheduled trigger.
    Cron,
    /// External webhook trigger.
    Webhook,
    /// Internal event trigger.
    Event,
    /// Manual API trigger.
    Manual,
}

impl std::fmt::Display for TriggerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cron => write!(f, "cron"),
            Self::Webhook => write!(f, "webhook"),
            Self::Event => write!(f, "event"),
            Self::Manual => write!(f, "manual"),
        }
    }
}

impl std::str::FromStr for TriggerKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "cron" => Ok(Self::Cron),
            "webhook" => Ok(Self::Webhook),
            "event" => Ok(Self::Event),
            "manual" => Ok(Self::Manual),
            _ => Err(format!("invalid trigger kind: {}", s)),
        }
    }
}

/// Trigger configuration in manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TriggerConfig {
    /// Cron trigger with schedule.
    Cron {
        /// Cron expression.
        schedule: String,
    },
    /// Event trigger with topic.
    Event {
        /// Event topic to subscribe to.
        topic: String,
    },
    /// Webhook trigger (token generated server-side).
    Webhook {},
    /// Manual trigger (always available).
    Manual {},
}

impl TriggerConfig {
    /// Get the trigger kind.
    pub fn kind(&self) -> TriggerKind {
        match self {
            Self::Cron { .. } => TriggerKind::Cron,
            Self::Event { .. } => TriggerKind::Event,
            Self::Webhook { .. } => TriggerKind::Webhook,
            Self::Manual { .. } => TriggerKind::Manual,
        }
    }
}

/// Retry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryConfig {
    /// Maximum number of attempts.
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,

    /// Backoff strategy.
    #[serde(default)]
    pub backoff: BackoffStrategy,

    /// Initial delay in milliseconds.
    #[serde(default = "default_initial_delay_ms")]
    pub initial_delay_ms: u64,

    /// Maximum delay in milliseconds.
    #[serde(default = "default_max_delay_ms")]
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
            backoff: BackoffStrategy::default(),
            initial_delay_ms: default_initial_delay_ms(),
            max_delay_ms: default_max_delay_ms(),
        }
    }
}

fn default_max_attempts() -> u32 {
    3
}

fn default_initial_delay_ms() -> u64 {
    1000
}

fn default_max_delay_ms() -> u64 {
    60_000
}

/// Backoff strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackoffStrategy {
    /// Linear backoff.
    Linear,
    /// Exponential backoff.
    #[default]
    Exponential,
}

/// Job-specific manifest fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobManifest {
    /// Trigger configurations.
    #[serde(default)]
    pub triggers: Vec<TriggerConfig>,

    /// Retry configuration.
    #[serde(default)]
    pub retry: RetryConfig,

    /// Maximum concurrent runs.
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: u32,

    /// Timeout in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_max_concurrency() -> u32 {
    10
}

fn default_timeout_ms() -> u64 {
    600_000 // 10 minutes
}

impl Default for JobManifest {
    fn default() -> Self {
        Self {
            triggers: Vec::new(),
            retry: RetryConfig::default(),
            max_concurrency: default_max_concurrency(),
            timeout_ms: default_timeout_ms(),
        }
    }
}
