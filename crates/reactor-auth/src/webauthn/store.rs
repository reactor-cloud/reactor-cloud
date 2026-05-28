//! WebAuthn database operations.

use super::types::{ChallengeType, WebAuthnChallenge, WebAuthnCredential};
use super::WebauthnError;
use chrono::{DateTime, Duration, Utc};
use reactor_core::id::UserId;
use sqlx::PgPool;

/// WebAuthn store for database operations.
#[derive(Clone)]
pub struct WebAuthnStore {
    pool: PgPool,
}

impl WebAuthnStore {
    /// Create a new WebAuthn store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Credentials
    // ─────────────────────────────────────────────────────────────────────────────

    /// Create a new WebAuthn credential.
    pub async fn create_credential(
        &self,
        user_id: &UserId,
        credential_id: &[u8],
        public_key: &[u8],
        aaguid: Option<uuid::Uuid>,
        counter: u64,
        transports: Vec<String>,
        name: Option<&str>,
    ) -> Result<WebAuthnCredential, WebauthnError> {
        let id = reactor_core::ReactorId::new();

        let row = sqlx::query_as::<_, WebAuthnCredentialRow>(
            r#"
            INSERT INTO reactor_auth.webauthn_credentials 
                (id, user_id, credential_id, public_key, aaguid, counter, transports, name)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, user_id, credential_id, public_key, aaguid, counter, transports, name, created_at, last_used_at
            "#,
        )
        .bind(id.as_uuid())
        .bind(user_id.as_uuid())
        .bind(credential_id)
        .bind(public_key)
        .bind(aaguid)
        .bind(counter as i64)
        .bind(&transports)
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create webauthn credential");
            WebauthnError::Configuration
        })?;

        Ok(row.into())
    }

    /// Find credentials for a user.
    pub async fn find_credentials_by_user(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<WebAuthnCredential>, WebauthnError> {
        let rows = sqlx::query_as::<_, WebAuthnCredentialRow>(
            r#"
            SELECT id, user_id, credential_id, public_key, aaguid, counter, transports, name, created_at, last_used_at
            FROM reactor_auth.webauthn_credentials
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find webauthn credentials");
            WebauthnError::Configuration
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Find a credential by ID.
    pub async fn find_credential_by_id(
        &self,
        id: &reactor_core::ReactorId,
    ) -> Result<Option<WebAuthnCredential>, WebauthnError> {
        let row = sqlx::query_as::<_, WebAuthnCredentialRow>(
            r#"
            SELECT id, user_id, credential_id, public_key, aaguid, counter, transports, name, created_at, last_used_at
            FROM reactor_auth.webauthn_credentials
            WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find webauthn credential");
            WebauthnError::Configuration
        })?;

        Ok(row.map(Into::into))
    }

    /// Update the counter and last_used_at for a credential.
    pub async fn update_credential_counter(
        &self,
        credential_id: &[u8],
        counter: u64,
    ) -> Result<(), WebauthnError> {
        sqlx::query(
            r#"
            UPDATE reactor_auth.webauthn_credentials
            SET counter = $2, last_used_at = NOW()
            WHERE credential_id = $1
            "#,
        )
        .bind(credential_id)
        .bind(counter as i64)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to update webauthn credential counter");
            WebauthnError::Configuration
        })?;

        Ok(())
    }

    /// Rename a credential.
    pub async fn rename_credential(
        &self,
        id: &reactor_core::ReactorId,
        user_id: &UserId,
        name: &str,
    ) -> Result<(), WebauthnError> {
        let result = sqlx::query(
            r#"
            UPDATE reactor_auth.webauthn_credentials
            SET name = $3
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(id.as_uuid())
        .bind(user_id.as_uuid())
        .bind(name)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to rename webauthn credential");
            WebauthnError::Configuration
        })?;

        if result.rows_affected() == 0 {
            return Err(WebauthnError::CredentialNotFound);
        }

        Ok(())
    }

    /// Delete a credential.
    pub async fn delete_credential(
        &self,
        id: &reactor_core::ReactorId,
        user_id: &UserId,
    ) -> Result<(), WebauthnError> {
        let result = sqlx::query(
            r#"
            DELETE FROM reactor_auth.webauthn_credentials
            WHERE id = $1 AND user_id = $2
            "#,
        )
        .bind(id.as_uuid())
        .bind(user_id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to delete webauthn credential");
            WebauthnError::Configuration
        })?;

        if result.rows_affected() == 0 {
            return Err(WebauthnError::CredentialNotFound);
        }

        Ok(())
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Challenges
    // ─────────────────────────────────────────────────────────────────────────────

    /// Create a new challenge.
    pub async fn create_challenge(
        &self,
        session_id: uuid::Uuid,
        challenge: &[u8],
        challenge_type: ChallengeType,
        user_id: Option<&UserId>,
        state: &[u8],
    ) -> Result<WebAuthnChallenge, WebauthnError> {
        let id = reactor_core::ReactorId::new();
        let expires_at = Utc::now() + Duration::minutes(5);

        let row = sqlx::query_as::<_, WebAuthnChallengeRow>(
            r#"
            INSERT INTO reactor_auth.webauthn_challenges 
                (id, session_id, challenge, challenge_type, user_id, created_at, expires_at)
            VALUES 
                ($1, $2, $3, $4, $5, NOW(), $6)
            RETURNING id, session_id, challenge, challenge_type, user_id, created_at, expires_at, consumed_at
            "#,
        )
        .bind(id.as_uuid())
        .bind(session_id)
        .bind(challenge)
        .bind(challenge_type.to_string())
        .bind(user_id.map(|u| u.as_uuid()))
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create webauthn challenge");
            WebauthnError::Configuration
        })?;

        // Store the state separately (it can be large)
        sqlx::query(
            r#"
            UPDATE reactor_auth.webauthn_challenges
            SET challenge = $2
            WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .bind(state)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to store webauthn challenge state");
            WebauthnError::Configuration
        })?;

        let mut challenge_record: WebAuthnChallenge = row.into();
        challenge_record.state = state.to_vec();
        Ok(challenge_record)
    }

    /// Find and consume a challenge.
    pub async fn consume_challenge(
        &self,
        session_id: uuid::Uuid,
        challenge_type: ChallengeType,
    ) -> Result<WebAuthnChallenge, WebauthnError> {
        // Find the challenge
        let row = sqlx::query_as::<_, WebAuthnChallengeRow>(
            r#"
            UPDATE reactor_auth.webauthn_challenges
            SET consumed_at = NOW()
            WHERE session_id = $1 
                AND challenge_type = $2 
                AND consumed_at IS NULL 
                AND expires_at > NOW()
            RETURNING id, session_id, challenge, challenge_type, user_id, created_at, expires_at, consumed_at
            "#,
        )
        .bind(session_id)
        .bind(challenge_type.to_string())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to consume webauthn challenge");
            WebauthnError::Configuration
        })?
        .ok_or(WebauthnError::ChallengeNotFound)?;

        let mut challenge: WebAuthnChallenge = row.into();
        // The state is stored in the challenge bytes (we overloaded the field)
        challenge.state = challenge.challenge.clone();
        Ok(challenge)
    }

    /// Clean up expired challenges.
    pub async fn cleanup_expired_challenges(&self) -> Result<u64, WebauthnError> {
        let result = sqlx::query(
            r#"
            DELETE FROM reactor_auth.webauthn_challenges
            WHERE expires_at < NOW() OR consumed_at IS NOT NULL
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to cleanup expired webauthn challenges");
            WebauthnError::Configuration
        })?;

        Ok(result.rows_affected())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Row types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct WebAuthnCredentialRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    credential_id: Vec<u8>,
    public_key: Vec<u8>,
    aaguid: Option<uuid::Uuid>,
    counter: i64,
    transports: Option<Vec<String>>,
    name: Option<String>,
    created_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
}

impl From<WebAuthnCredentialRow> for WebAuthnCredential {
    fn from(row: WebAuthnCredentialRow) -> Self {
        Self {
            id: row.id.into(),
            user_id: row.user_id.into(),
            credential_id: row.credential_id,
            public_key: row.public_key,
            aaguid: row.aaguid,
            counter: row.counter as u64,
            transports: row.transports.unwrap_or_default(),
            name: row.name,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct WebAuthnChallengeRow {
    id: uuid::Uuid,
    session_id: uuid::Uuid,
    challenge: Vec<u8>,
    challenge_type: String,
    user_id: Option<uuid::Uuid>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    consumed_at: Option<DateTime<Utc>>,
}

impl From<WebAuthnChallengeRow> for WebAuthnChallenge {
    fn from(row: WebAuthnChallengeRow) -> Self {
        Self {
            id: row.id.into(),
            session_id: row.session_id,
            challenge: row.challenge,
            challenge_type: row.challenge_type.parse().unwrap_or(ChallengeType::Registration),
            user_id: row.user_id.map(Into::into),
            state: vec![], // Will be populated separately
            created_at: row.created_at,
            expires_at: row.expires_at,
            consumed_at: row.consumed_at,
        }
    }
}
