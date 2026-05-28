//! CLI error types and exit code mapping.

use std::process::ExitCode;

/// Exit codes for the CLI.
///
/// These are documented and stable for scripting/agent use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CliExitCode {
    /// Success.
    Ok = 0,
    /// User error (invalid arguments, missing required flags).
    User = 1,
    /// Configuration/context error (missing config, invalid context).
    Config = 2,
    /// Authentication error (invalid token, permission denied).
    Auth = 3,
    /// Validation error (invalid manifest, schema violation).
    Validation = 4,
    /// Server error (5xx from server, deployment failed).
    Server = 5,
    /// Network error (connection refused, timeout).
    Network = 6,
}

impl From<CliExitCode> for ExitCode {
    fn from(code: CliExitCode) -> Self {
        ExitCode::from(code as u8)
    }
}

/// CLI errors with associated exit codes.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    // User errors (exit code 1)
    #[error("{0}")]
    User(String),

    #[error("missing required argument: {0}")]
    MissingArgument(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("command requires --yes flag in non-interactive mode")]
    RequiresConfirmation,

    // Config errors (exit code 2)
    #[error("configuration error: {0}")]
    Config(String),

    #[error("configuration error: {0}")]
    ConfigError(String),

    #[error("context not found: {0}")]
    ContextNotFound(String),

    #[error("project manifest not found (looking for reactor.toml)")]
    ManifestNotFound,

    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("function not found: {0}")]
    FunctionNotFound(String),

    #[error("feature not available: {0}")]
    FeatureDisabled(String),

    // Auth errors (exit code 3)
    #[error("authentication required")]
    AuthRequired,

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("permission denied: {0}")]
    PermissionDenied(String),

    // Validation errors (exit code 4)
    #[error("validation error: {0}")]
    Validation(String),

    #[error("bundle validation failed: {0}")]
    BundleValidation(String),

    // Server errors (exit code 5)
    #[error("server error: {0}")]
    Server(String),

    #[error("server error: {0}")]
    ServerError(String),

    #[error("deployment failed: {0}")]
    DeploymentFailed(String),

    #[error("partial deployment: {succeeded} succeeded, {failed} failed")]
    PartialDeployment { succeeded: usize, failed: usize },

    // Network errors (exit code 6)
    #[error("network error: {0}")]
    Network(String),

    #[error("connection refused: {0}")]
    ConnectionRefused(String),

    #[error("request timeout")]
    Timeout,

    // Keychain errors (exit code 2)
    #[error("keychain error: {0}")]
    Keychain(String),

    // Internal errors (exit code 5)
    #[error("internal error: {0}")]
    Internal(String),

    // Wrapped errors
    #[error(transparent)]
    Client(#[from] reactor_client::ClientError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Url(#[from] url::ParseError),
}

impl CliError {
    /// Get the exit code for this error.
    pub fn exit_code(&self) -> CliExitCode {
        match self {
            // User errors
            Self::User(_)
            | Self::MissingArgument(_)
            | Self::InvalidArgument(_)
            | Self::RequiresConfirmation => CliExitCode::User,

            // Config errors
            Self::Config(_)
            | Self::ConfigError(_)
            | Self::ContextNotFound(_)
            | Self::ManifestNotFound
            | Self::InvalidManifest(_)
            | Self::FeatureDisabled(_)
            | Self::Keychain(_) => CliExitCode::Config,

            // Auth errors
            Self::AuthRequired | Self::AuthFailed(_) | Self::PermissionDenied(_) => {
                CliExitCode::Auth
            }

            // Validation errors
            Self::Validation(_) | Self::BundleValidation(_) => CliExitCode::Validation,

            // Server errors
            Self::Server(_)
            | Self::ServerError(_)
            | Self::DeploymentFailed(_)
            | Self::PartialDeployment { .. }
            | Self::Internal(_) => CliExitCode::Server,

            // Network errors
            Self::Network(_) | Self::ConnectionRefused(_) | Self::Timeout => CliExitCode::Network,

            // Wrapped errors - map based on underlying type
            Self::Client(e) => {
                if e.is_network() {
                    CliExitCode::Network
                } else if e.is_auth() {
                    CliExitCode::Auth
                } else if e.is_server_error() {
                    CliExitCode::Server
                } else {
                    CliExitCode::User
                }
            }
            Self::Io(_) => CliExitCode::Config,
            Self::Json(_) => CliExitCode::Validation,
            Self::Url(_) => CliExitCode::Config,
            Self::FunctionNotFound(_) => CliExitCode::Config,
        }
    }

    /// Get the error code string for JSON output.
    pub fn code(&self) -> &'static str {
        match self {
            Self::User(_) => "USER_ERROR",
            Self::MissingArgument(_) => "MISSING_ARGUMENT",
            Self::InvalidArgument(_) => "INVALID_ARGUMENT",
            Self::RequiresConfirmation => "REQUIRES_CONFIRMATION",
            Self::Config(_) => "CONFIG_ERROR",
            Self::ConfigError(_) => "CONFIG_ERROR",
            Self::ContextNotFound(_) => "CONTEXT_NOT_FOUND",
            Self::ManifestNotFound => "MANIFEST_NOT_FOUND",
            Self::InvalidManifest(_) => "INVALID_MANIFEST",
            Self::FunctionNotFound(_) => "FUNCTION_NOT_FOUND",
            Self::FeatureDisabled(_) => "FEATURE_DISABLED",
            Self::AuthRequired => "AUTH_REQUIRED",
            Self::AuthFailed(_) => "AUTH_FAILED",
            Self::PermissionDenied(_) => "PERMISSION_DENIED",
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::BundleValidation(_) => "BUNDLE_VALIDATION_ERROR",
            Self::Server(_) => "SERVER_ERROR",
            Self::ServerError(_) => "SERVER_ERROR",
            Self::DeploymentFailed(_) => "DEPLOYMENT_FAILED",
            Self::PartialDeployment { .. } => "PARTIAL_DEPLOYMENT",
            Self::Network(_) => "NETWORK_ERROR",
            Self::ConnectionRefused(_) => "CONNECTION_REFUSED",
            Self::Timeout => "TIMEOUT",
            Self::Keychain(_) => "KEYCHAIN_ERROR",
            Self::Internal(_) => "INTERNAL_ERROR",
            Self::Client(_) => "CLIENT_ERROR",
            Self::Io(_) => "IO_ERROR",
            Self::Json(_) => "JSON_ERROR",
            Self::Url(_) => "URL_ERROR",
        }
    }

    /// Get an optional hint for this error.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::RequiresConfirmation => {
                Some("Pass --yes to confirm, or set REACTOR_ASSUME_YES=1")
            }
            Self::ContextNotFound(_) => Some("Run 'reactor context list' to see available contexts"),
            Self::ManifestNotFound => {
                Some("Run 'reactor init <name>' to create a new project, or cd to a project directory")
            }
            Self::AuthRequired => Some("Run 'reactor login' to authenticate"),
            Self::ConnectionRefused(_) => {
                Some("Is the server running? Try 'reactor dev' to start a local server")
            }
            Self::Timeout => Some("The server may be overloaded. Try again later"),
            _ => None,
        }
    }
}

/// Result type for CLI operations.
pub type CliResult<T> = Result<T, CliError>;
