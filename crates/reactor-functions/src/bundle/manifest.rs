//! Manifest schema and validation.

use crate::error::FunctionsError;
use serde::{Deserialize, Serialize};

// ============================================================================
// Job-specific configuration (for reactor-jobs integration)
// ============================================================================

/// Trigger kind for jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobTriggerKind {
    /// Cron-scheduled trigger.
    Cron,
    /// External webhook trigger.
    Webhook,
    /// Internal event trigger.
    Event,
    /// Manual API trigger.
    Manual,
}

/// Trigger configuration for jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum JobTriggerConfig {
    /// Cron trigger with schedule.
    Cron {
        /// Cron expression (e.g., "0 9 * * *").
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

/// Backoff strategy for job retries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum JobBackoffStrategy {
    /// Linear backoff.
    Linear,
    /// Exponential backoff.
    #[default]
    Exponential,
}

/// Retry configuration for jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRetryConfig {
    /// Maximum number of attempts (default: 3).
    #[serde(default = "default_job_max_attempts")]
    pub max_attempts: u32,

    /// Backoff strategy (default: exponential).
    #[serde(default)]
    pub backoff: JobBackoffStrategy,

    /// Initial delay in milliseconds (default: 1000).
    #[serde(default = "default_job_initial_delay_ms")]
    pub initial_delay_ms: u64,

    /// Maximum delay in milliseconds (default: 60000).
    #[serde(default = "default_job_max_delay_ms")]
    pub max_delay_ms: u64,
}

impl Default for JobRetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: default_job_max_attempts(),
            backoff: JobBackoffStrategy::default(),
            initial_delay_ms: default_job_initial_delay_ms(),
            max_delay_ms: default_job_max_delay_ms(),
        }
    }
}

fn default_job_max_attempts() -> u32 {
    3
}
fn default_job_initial_delay_ms() -> u64 {
    1000
}
fn default_job_max_delay_ms() -> u64 {
    60_000
}
fn default_job_max_concurrency() -> u32 {
    10
}
fn default_job_timeout_ms() -> u64 {
    600_000
}

/// Job-specific manifest configuration.
///
/// When present, deploying this function also creates/updates a job
/// in reactor-jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobConfig {
    /// Trigger configurations.
    #[serde(default)]
    pub triggers: Vec<JobTriggerConfig>,

    /// Retry configuration.
    #[serde(default)]
    pub retry: JobRetryConfig,

    /// Maximum concurrent runs for this job.
    #[serde(default = "default_job_max_concurrency")]
    pub max_concurrency: u32,

    /// Job timeout in milliseconds.
    #[serde(default = "default_job_timeout_ms")]
    pub timeout_ms: u64,

    /// Job description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Default for JobConfig {
    fn default() -> Self {
        Self {
            triggers: Vec::new(),
            retry: JobRetryConfig::default(),
            max_concurrency: default_job_max_concurrency(),
            timeout_ms: default_job_timeout_ms(),
            description: None,
        }
    }
}

impl JobConfig {
    /// Validate the job configuration.
    pub fn validate(&self) -> Result<(), FunctionsError> {
        // Validate triggers
        for trigger in &self.triggers {
            match trigger {
                JobTriggerConfig::Cron { schedule } => {
                    // Validate cron expression syntax
                    if schedule.is_empty() {
                        return Err(FunctionsError::ManifestInvalid(
                            "cron trigger schedule cannot be empty".to_string(),
                        ));
                    }
                    // Basic format check (5 or 6 fields)
                    let parts: Vec<&str> = schedule.split_whitespace().collect();
                    if parts.len() < 5 || parts.len() > 6 {
                        return Err(FunctionsError::ManifestInvalid(format!(
                            "invalid cron expression '{}': expected 5 or 6 fields",
                            schedule
                        )));
                    }
                }
                JobTriggerConfig::Event { topic } => {
                    if topic.is_empty() {
                        return Err(FunctionsError::ManifestInvalid(
                            "event trigger topic cannot be empty".to_string(),
                        ));
                    }
                    // Validate topic format (alphanumeric with dots)
                    if !topic.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') {
                        return Err(FunctionsError::ManifestInvalid(format!(
                            "invalid event topic '{}': must be alphanumeric with dots",
                            topic
                        )));
                    }
                }
                JobTriggerConfig::Webhook {} | JobTriggerConfig::Manual {} => {
                    // No validation needed
                }
            }
        }

        // Validate retry config
        if self.retry.max_attempts == 0 {
            return Err(FunctionsError::ManifestInvalid(
                "job retry max_attempts must be at least 1".to_string(),
            ));
        }

        if self.retry.initial_delay_ms == 0 {
            return Err(FunctionsError::ManifestInvalid(
                "job retry initial_delay_ms must be greater than 0".to_string(),
            ));
        }

        // Validate concurrency
        if self.max_concurrency == 0 {
            return Err(FunctionsError::ManifestInvalid(
                "job max_concurrency must be at least 1".to_string(),
            ));
        }

        // Validate timeout
        if self.timeout_ms == 0 {
            return Err(FunctionsError::ManifestInvalid(
                "job timeout_ms must be greater than 0".to_string(),
            ));
        }

        // Cap timeout at 1 hour for jobs
        if self.timeout_ms > 3_600_000 {
            return Err(FunctionsError::ManifestInvalid(
                "job timeout_ms exceeds maximum of 3600000 (1 hour)".to_string(),
            ));
        }

        Ok(())
    }
}

// ============================================================================
// Function manifest
// ============================================================================

/// Maximum bundle size in bytes (50 MiB).
pub const BUNDLE_MAX_SIZE: u64 = 50 * 1024 * 1024;

/// Default timeout in milliseconds.
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

/// Default memory in MB.
pub const DEFAULT_MEMORY_MB: u32 = 256;

/// Default max body in MB.
pub const DEFAULT_MAX_BODY_IN_MB: u32 = 5;

/// Default max body out MB.
pub const DEFAULT_MAX_BODY_OUT_MB: u32 = 6;

/// Default min instances.
pub const DEFAULT_MIN_INSTANCES: u32 = 0;

/// Default max concurrency.
pub const DEFAULT_MAX_CONCURRENCY: u32 = 50;

/// Runtime kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeKind {
    /// WebAssembly runtime (wasmtime + WASI HTTP).
    Wasm,
    /// Bun subprocess runtime.
    Bun,
    /// AWS Lambda runtime.
    Lambda,
}

impl std::fmt::Display for RuntimeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeKind::Wasm => write!(f, "wasm"),
            RuntimeKind::Bun => write!(f, "bun"),
            RuntimeKind::Lambda => write!(f, "lambda"),
        }
    }
}

impl std::str::FromStr for RuntimeKind {
    type Err = FunctionsError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "wasm" => Ok(RuntimeKind::Wasm),
            "bun" => Ok(RuntimeKind::Bun),
            "lambda" => Ok(RuntimeKind::Lambda),
            _ => Err(FunctionsError::UnsupportedRuntime(s.to_string())),
        }
    }
}

/// Resource limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleLimits {
    /// Timeout in milliseconds.
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,

    /// Memory limit in MB.
    #[serde(default = "default_memory_mb")]
    pub memory_mb: u32,

    /// Maximum request body size in MB.
    #[serde(default = "default_max_body_in_mb")]
    pub max_body_in_mb: u32,

    /// Maximum response body size in MB.
    #[serde(default = "default_max_body_out_mb")]
    pub max_body_out_mb: u32,
}

impl Default for BundleLimits {
    fn default() -> Self {
        Self {
            timeout_ms: DEFAULT_TIMEOUT_MS,
            memory_mb: DEFAULT_MEMORY_MB,
            max_body_in_mb: DEFAULT_MAX_BODY_IN_MB,
            max_body_out_mb: DEFAULT_MAX_BODY_OUT_MB,
        }
    }
}

fn default_timeout_ms() -> u64 {
    DEFAULT_TIMEOUT_MS
}
fn default_memory_mb() -> u32 {
    DEFAULT_MEMORY_MB
}
fn default_max_body_in_mb() -> u32 {
    DEFAULT_MAX_BODY_IN_MB
}
fn default_max_body_out_mb() -> u32 {
    DEFAULT_MAX_BODY_OUT_MB
}

/// Concurrency configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Minimum instances to keep warm.
    #[serde(default)]
    pub min_instances: u32,

    /// Maximum concurrent invocations.
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: u32,
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            min_instances: DEFAULT_MIN_INSTANCES,
            max_concurrency: DEFAULT_MAX_CONCURRENCY,
        }
    }
}

fn default_max_concurrency() -> u32 {
    DEFAULT_MAX_CONCURRENCY
}

/// Bundle manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Function name.
    pub name: String,

    /// Version number (server-assigned).
    #[serde(default)]
    pub version: i64,

    /// Runtime type.
    pub runtime: RuntimeKind,

    /// Entrypoint file (relative to bundle root).
    pub entrypoint: String,

    /// Resource limits.
    #[serde(default)]
    pub limits: BundleLimits,

    /// Concurrency configuration.
    #[serde(default)]
    pub concurrency: ConcurrencyConfig,

    /// Required environment variable keys.
    #[serde(default)]
    pub env_keys: Vec<String>,

    /// Required secret keys.
    #[serde(default)]
    pub secret_keys: Vec<String>,

    /// Whether to forward the Authorization header to the function.
    #[serde(default)]
    pub forward_authorization: bool,

    /// Bundle SHA256 hash (hex-encoded, verified by server).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle_sha256: Option<String>,

    /// Job configuration (optional).
    ///
    /// When present, deploying this function also creates/updates a job
    /// in reactor-jobs with the specified triggers and retry configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job: Option<JobConfig>,
}

impl Manifest {
    /// Validate the manifest against runtime-specific rules.
    pub fn validate(&self, expected_runtime: &str) -> Result<(), FunctionsError> {
        // Check runtime matches
        if self.runtime.to_string() != expected_runtime {
            return Err(FunctionsError::ManifestInvalid(format!(
                "runtime mismatch: manifest has '{}', function expects '{}'",
                self.runtime, expected_runtime
            )));
        }

        // Validate entrypoint
        if self.entrypoint.is_empty() {
            return Err(FunctionsError::ManifestInvalid(
                "entrypoint is required".to_string(),
            ));
        }
        if !self.entrypoint.starts_with("code/") {
            return Err(FunctionsError::ManifestInvalid(
                "entrypoint must start with 'code/'".to_string(),
            ));
        }

        // Validate limits per runtime
        self.validate_limits()?;

        // Validate job configuration if present
        if let Some(job) = &self.job {
            job.validate()?;
        }

        Ok(())
    }

    /// Returns true if this manifest has job configuration.
    pub fn is_job(&self) -> bool {
        self.job.is_some()
    }

    fn validate_limits(&self) -> Result<(), FunctionsError> {
        // Timeout caps per runtime
        let max_timeout_ms = match self.runtime {
            RuntimeKind::Wasm => 300_000,
            RuntimeKind::Bun => 300_000,
            RuntimeKind::Lambda => 900_000,
        };

        if self.limits.timeout_ms > max_timeout_ms {
            return Err(FunctionsError::ManifestInvalid(format!(
                "timeout_ms {} exceeds maximum {} for {} runtime",
                self.limits.timeout_ms, max_timeout_ms, self.runtime
            )));
        }

        // Memory ranges per runtime
        let (min_memory, max_memory) = match self.runtime {
            RuntimeKind::Wasm => (32, 1024),
            RuntimeKind::Bun => (64, 2048),
            RuntimeKind::Lambda => (128, 10_240),
        };

        if self.limits.memory_mb < min_memory || self.limits.memory_mb > max_memory {
            return Err(FunctionsError::ManifestInvalid(format!(
                "memory_mb {} out of range [{}, {}] for {} runtime",
                self.limits.memory_mb, min_memory, max_memory, self.runtime
            )));
        }

        // Concurrency validation
        if self.concurrency.max_concurrency == 0 {
            return Err(FunctionsError::ManifestInvalid(
                "max_concurrency must be > 0".to_string(),
            ));
        }

        if self.concurrency.max_concurrency > 1000 {
            return Err(FunctionsError::ManifestInvalid(
                "max_concurrency exceeds maximum of 1000".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest() -> Manifest {
        Manifest {
            name: "test-func".to_string(),
            version: 0,
            runtime: RuntimeKind::Bun,
            entrypoint: "code/index.ts".to_string(),
            limits: BundleLimits::default(),
            concurrency: ConcurrencyConfig::default(),
            env_keys: vec![],
            secret_keys: vec![],
            forward_authorization: false,
            bundle_sha256: None,
            job: None,
        }
    }

    fn valid_job_manifest() -> Manifest {
        let mut manifest = valid_manifest();
        manifest.job = Some(JobConfig {
            triggers: vec![
                JobTriggerConfig::Cron {
                    schedule: "0 9 * * *".to_string(),
                },
                JobTriggerConfig::Manual {},
            ],
            retry: JobRetryConfig::default(),
            max_concurrency: 5,
            timeout_ms: 300_000,
            description: Some("Test job".to_string()),
        });
        manifest
    }

    #[test]
    fn test_valid_manifest() {
        let manifest = valid_manifest();
        assert!(manifest.validate("bun").is_ok());
    }

    #[test]
    fn test_runtime_mismatch() {
        let manifest = valid_manifest();
        let err = manifest.validate("wasm").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_empty_entrypoint() {
        let mut manifest = valid_manifest();
        manifest.entrypoint = String::new();
        let err = manifest.validate("bun").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_invalid_entrypoint() {
        let mut manifest = valid_manifest();
        manifest.entrypoint = "index.ts".to_string();
        let err = manifest.validate("bun").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_timeout_exceeded() {
        let mut manifest = valid_manifest();
        manifest.limits.timeout_ms = 1_000_000;
        let err = manifest.validate("bun").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_memory_out_of_range() {
        let mut manifest = valid_manifest();
        manifest.limits.memory_mb = 10;
        let err = manifest.validate("bun").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_wasm_memory_range() {
        let mut manifest = valid_manifest();
        manifest.runtime = RuntimeKind::Wasm;
        manifest.entrypoint = "code/main.wasm".to_string();
        manifest.limits.memory_mb = 32;
        assert!(manifest.validate("wasm").is_ok());

        manifest.limits.memory_mb = 1024;
        assert!(manifest.validate("wasm").is_ok());

        manifest.limits.memory_mb = 31;
        assert!(manifest.validate("wasm").is_err());

        manifest.limits.memory_mb = 1025;
        assert!(manifest.validate("wasm").is_err());
    }

    // Job validation tests
    #[test]
    fn test_valid_job_manifest() {
        let manifest = valid_job_manifest();
        assert!(manifest.validate("bun").is_ok());
        assert!(manifest.is_job());
    }

    #[test]
    fn test_job_empty_cron() {
        let mut manifest = valid_job_manifest();
        manifest.job = Some(JobConfig {
            triggers: vec![JobTriggerConfig::Cron {
                schedule: "".to_string(),
            }],
            ..Default::default()
        });
        let err = manifest.validate("bun").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_job_invalid_cron() {
        let mut manifest = valid_job_manifest();
        manifest.job = Some(JobConfig {
            triggers: vec![JobTriggerConfig::Cron {
                schedule: "invalid".to_string(),
            }],
            ..Default::default()
        });
        let err = manifest.validate("bun").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_job_empty_event_topic() {
        let mut manifest = valid_job_manifest();
        manifest.job = Some(JobConfig {
            triggers: vec![JobTriggerConfig::Event {
                topic: "".to_string(),
            }],
            ..Default::default()
        });
        let err = manifest.validate("bun").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_job_invalid_event_topic() {
        let mut manifest = valid_job_manifest();
        manifest.job = Some(JobConfig {
            triggers: vec![JobTriggerConfig::Event {
                topic: "invalid topic!".to_string(),
            }],
            ..Default::default()
        });
        let err = manifest.validate("bun").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_job_zero_max_attempts() {
        let mut manifest = valid_job_manifest();
        manifest.job = Some(JobConfig {
            retry: JobRetryConfig {
                max_attempts: 0,
                ..Default::default()
            },
            ..Default::default()
        });
        let err = manifest.validate("bun").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_job_zero_concurrency() {
        let mut manifest = valid_job_manifest();
        manifest.job = Some(JobConfig {
            max_concurrency: 0,
            ..Default::default()
        });
        let err = manifest.validate("bun").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_job_timeout_exceeds_max() {
        let mut manifest = valid_job_manifest();
        manifest.job = Some(JobConfig {
            timeout_ms: 4_000_000, // > 1 hour
            ..Default::default()
        });
        let err = manifest.validate("bun").unwrap_err();
        assert!(matches!(err, FunctionsError::ManifestInvalid(_)));
    }

    #[test]
    fn test_job_webhook_trigger() {
        let mut manifest = valid_job_manifest();
        manifest.job = Some(JobConfig {
            triggers: vec![JobTriggerConfig::Webhook {}],
            ..Default::default()
        });
        assert!(manifest.validate("bun").is_ok());
    }

    #[test]
    fn test_job_event_trigger() {
        let mut manifest = valid_job_manifest();
        manifest.job = Some(JobConfig {
            triggers: vec![JobTriggerConfig::Event {
                topic: "orders.created".to_string(),
            }],
            ..Default::default()
        });
        assert!(manifest.validate("bun").is_ok());
    }

    #[test]
    fn test_is_job_without_config() {
        let manifest = valid_manifest();
        assert!(!manifest.is_job());
    }
}
