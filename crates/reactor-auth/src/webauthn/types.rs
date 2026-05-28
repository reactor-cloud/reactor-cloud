//! WebAuthn type definitions.

use chrono::{DateTime, Utc};
use reactor_core::id::UserId;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A stored WebAuthn credential.
#[derive(Debug, Clone)]
pub struct WebAuthnCredential {
    /// Credential ID (database primary key).
    pub id: reactor_core::ReactorId,
    /// User who owns this credential.
    pub user_id: UserId,
    /// The credential ID from the authenticator (raw bytes).
    pub credential_id: Vec<u8>,
    /// The COSE public key (raw bytes).
    pub public_key: Vec<u8>,
    /// Authenticator attestation GUID.
    pub aaguid: Option<uuid::Uuid>,
    /// Signature counter for replay protection.
    pub counter: u64,
    /// Supported transports (e.g., "usb", "nfc", "ble", "internal").
    pub transports: Vec<String>,
    /// User-provided name for this credential.
    pub name: Option<String>,
    /// When the credential was registered.
    pub created_at: DateTime<Utc>,
    /// When the credential was last used.
    pub last_used_at: Option<DateTime<Utc>>,
}

/// A stored WebAuthn challenge.
#[derive(Debug, Clone)]
pub struct WebAuthnChallenge {
    /// Challenge ID.
    pub id: reactor_core::ReactorId,
    /// Session or request ID this challenge is for.
    pub session_id: uuid::Uuid,
    /// The challenge bytes.
    pub challenge: Vec<u8>,
    /// Type of challenge: "registration" or "authentication".
    pub challenge_type: ChallengeType,
    /// User ID (for registration challenges).
    pub user_id: Option<UserId>,
    /// Serialized state for the WebAuthn ceremony.
    pub state: Vec<u8>,
    /// When the challenge was created.
    pub created_at: DateTime<Utc>,
    /// When the challenge expires.
    pub expires_at: DateTime<Utc>,
    /// When the challenge was consumed.
    pub consumed_at: Option<DateTime<Utc>>,
}

/// Type of WebAuthn challenge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChallengeType {
    /// Registration (adding a new credential).
    Registration,
    /// Authentication (using an existing credential).
    Authentication,
}

impl std::fmt::Display for ChallengeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChallengeType::Registration => write!(f, "registration"),
            ChallengeType::Authentication => write!(f, "authentication"),
        }
    }
}

impl std::str::FromStr for ChallengeType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "registration" => Ok(ChallengeType::Registration),
            "authentication" => Ok(ChallengeType::Authentication),
            _ => Err(format!("invalid challenge type: {}", s)),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// API Request/Response types
// ─────────────────────────────────────────────────────────────────────────────

/// Start registration request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct StartRegistrationRequest {
    /// Optional name for the credential (e.g., "MacBook Touch ID").
    pub name: Option<String>,
}

/// Start registration response.
#[derive(Debug, Serialize, ToSchema)]
pub struct StartRegistrationResponse {
    /// Session ID for this registration ceremony.
    pub session_id: String,
    /// WebAuthn credential creation options (JSON).
    /// This should be passed to `navigator.credentials.create()`.
    pub options: serde_json::Value,
}

/// Finish registration request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct FinishRegistrationRequest {
    /// Session ID from the start registration response.
    pub session_id: String,
    /// The credential response from the authenticator.
    /// This is the result of `navigator.credentials.create()`.
    pub credential: serde_json::Value,
    /// Optional name for the credential.
    pub name: Option<String>,
}

/// Finish registration response.
#[derive(Debug, Serialize, ToSchema)]
pub struct FinishRegistrationResponse {
    /// The new credential's ID.
    pub credential_id: String,
    /// Success message.
    pub message: String,
}

/// Start authentication request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct StartAuthenticationRequest {
    /// Optional: specific credential IDs to use.
    /// If not provided, all credentials for the user will be allowed.
    #[serde(default)]
    pub credential_ids: Vec<String>,
}

/// Start authentication response.
#[derive(Debug, Serialize, ToSchema)]
pub struct StartAuthenticationResponse {
    /// Session ID for this authentication ceremony.
    pub session_id: String,
    /// WebAuthn credential request options (JSON).
    /// This should be passed to `navigator.credentials.get()`.
    pub options: serde_json::Value,
}

/// Finish authentication request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct FinishAuthenticationRequest {
    /// Session ID from the start authentication response.
    pub session_id: String,
    /// The credential response from the authenticator.
    /// This is the result of `navigator.credentials.get()`.
    pub credential: serde_json::Value,
}

/// Finish authentication response.
#[derive(Debug, Serialize, ToSchema)]
pub struct FinishAuthenticationResponse {
    /// Whether authentication was successful.
    pub success: bool,
    /// The credential ID that was used.
    pub credential_id: String,
    /// New access token with mfa_at claim set.
    pub access_token: String,
    /// New refresh token.
    pub refresh_token: String,
    /// When the access token expires.
    pub expires_at: String,
}

/// List credentials response.
#[derive(Debug, Serialize, ToSchema)]
pub struct ListCredentialsResponse {
    /// List of credentials.
    pub credentials: Vec<CredentialInfo>,
}

/// Credential info for listing.
#[derive(Debug, Serialize, ToSchema)]
pub struct CredentialInfo {
    /// Credential ID (database ID, not the WebAuthn credential ID).
    pub id: String,
    /// User-provided name.
    pub name: Option<String>,
    /// When the credential was registered.
    pub created_at: String,
    /// When the credential was last used.
    pub last_used_at: Option<String>,
    /// Supported transports.
    pub transports: Vec<String>,
}

/// Delete credential request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct DeleteCredentialRequest {
    /// Credential ID to delete.
    pub credential_id: String,
}

/// Delete credential response.
#[derive(Debug, Serialize, ToSchema)]
pub struct DeleteCredentialResponse {
    /// Success message.
    pub message: String,
}

/// Rename credential request.
#[derive(Debug, Deserialize, ToSchema)]
pub struct RenameCredentialRequest {
    /// New name for the credential.
    pub name: String,
}
