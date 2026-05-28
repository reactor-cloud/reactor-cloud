//! PostgreSQL implementation of IdentityStore.

use super::{ApiKey, IdentityStore, Invitation, Membership, Org, RefreshToken, Role, Session, SigningKey};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use reactor_core::auth::{AuthError, User};
use reactor_core::id::{OrgId, RoleId, SessionId, UserId};
use sqlx::PgPool;

/// PostgreSQL-backed identity store.
#[derive(Clone)]
pub struct PgIdentityStore {
    pool: PgPool,
}

impl PgIdentityStore {
    /// Create a new Postgres identity store.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get a reference to the connection pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl IdentityStore for PgIdentityStore {
    // ========== Users ==========

    async fn find_user_by_id(&self, id: &UserId) -> Result<Option<User>, AuthError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, email, email_verified, metadata, default_org_id, disabled_at, created_at, updated_at
            FROM reactor_auth.users
            WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find user by id");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn find_user_by_email(&self, email: &str) -> Result<Option<User>, AuthError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            SELECT id, email, email_verified, metadata, default_org_id, disabled_at, created_at, updated_at
            FROM reactor_auth.users
            WHERE email = $1
            "#,
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find user by email");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn create_user(
        &self,
        id: UserId,
        email: &str,
        password_hash: Option<&str>,
        metadata: serde_json::Value,
    ) -> Result<User, AuthError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            INSERT INTO reactor_auth.users (id, email, password_hash, metadata)
            VALUES ($1, $2, $3, $4)
            RETURNING id, email, email_verified, metadata, default_org_id, disabled_at, created_at, updated_at
            "#,
        )
        .bind(id.as_uuid())
        .bind(email)
        .bind(password_hash)
        .bind(&metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let Some(db_err) = e.as_database_error() {
                if db_err.constraint() == Some("users_email_key") {
                    return AuthError::EmailExists;
                }
            }
            tracing::error!(error = %e, "failed to create user");
            AuthError::Internal
        })?;

        Ok(row.into())
    }

    async fn update_user(
        &self,
        id: &UserId,
        email: Option<&str>,
        password_hash: Option<&str>,
        email_verified: Option<bool>,
        metadata: Option<serde_json::Value>,
        default_org_id: Option<Option<OrgId>>,
    ) -> Result<User, AuthError> {
        let row = sqlx::query_as::<_, UserRow>(
            r#"
            UPDATE reactor_auth.users
            SET
                email = COALESCE($2, email),
                password_hash = COALESCE($3, password_hash),
                email_verified = COALESCE($4, email_verified),
                metadata = COALESCE($5, metadata),
                default_org_id = CASE WHEN $6 THEN $7 ELSE default_org_id END,
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, email, email_verified, metadata, default_org_id, disabled_at, created_at, updated_at
            "#,
        )
        .bind(id.as_uuid())
        .bind(email)
        .bind(password_hash)
        .bind(email_verified)
        .bind(&metadata)
        .bind(default_org_id.is_some())
        .bind(default_org_id.flatten().map(|id| id.into_uuid()))
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to update user");
            AuthError::Internal
        })?
        .ok_or(AuthError::UserNotFound)?;

        Ok(row.into())
    }

    async fn get_password_hash(&self, user_id: &UserId) -> Result<Option<String>, AuthError> {
        let row: Option<(Option<String>,)> = sqlx::query_as(
            r#"
            SELECT password_hash
            FROM reactor_auth.users
            WHERE id = $1
            "#,
        )
        .bind(user_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get password hash");
            AuthError::Internal
        })?;

        Ok(row.and_then(|(hash,)| hash))
    }

    async fn disable_user(&self, id: &UserId) -> Result<(), AuthError> {
        let result = sqlx::query(
            r#"
            UPDATE reactor_auth.users
            SET disabled_at = NOW(), updated_at = NOW()
            WHERE id = $1 AND disabled_at IS NULL
            "#,
        )
        .bind(id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to disable user");
            AuthError::Internal
        })?;

        if result.rows_affected() == 0 {
            return Err(AuthError::UserNotFound);
        }

        Ok(())
    }

    // ========== Sessions ==========

    async fn create_session(
        &self,
        id: SessionId,
        user_id: &UserId,
        amr: Vec<String>,
        ip: Option<&str>,
        user_agent: Option<&str>,
    ) -> Result<Session, AuthError> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"
            INSERT INTO reactor_auth.sessions (id, user_id, amr, ip, user_agent)
            VALUES ($1, $2, $3, $4::inet, $5)
            RETURNING id, user_id, amr, ip::text, user_agent, created_at, last_seen_at, revoked_at
            "#,
        )
        .bind(id.as_uuid())
        .bind(user_id.as_uuid())
        .bind(&amr)
        .bind(ip)
        .bind(user_agent)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create session");
            AuthError::Internal
        })?;

        Ok(row.into())
    }

    async fn find_session_by_id(&self, id: &SessionId) -> Result<Option<Session>, AuthError> {
        let row = sqlx::query_as::<_, SessionRow>(
            r#"
            SELECT id, user_id, amr, ip::text, user_agent, created_at, last_seen_at, revoked_at
            FROM reactor_auth.sessions
            WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find session");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn revoke_session(&self, id: &SessionId) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            UPDATE reactor_auth.sessions
            SET revoked_at = NOW()
            WHERE id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to revoke session");
            AuthError::Internal
        })?;

        Ok(())
    }

    async fn revoke_user_sessions(&self, user_id: &UserId) -> Result<u64, AuthError> {
        let result = sqlx::query(
            r#"
            UPDATE reactor_auth.sessions
            SET revoked_at = NOW()
            WHERE user_id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(user_id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to revoke user sessions");
            AuthError::Internal
        })?;

        Ok(result.rows_affected())
    }

    // ========== Refresh Tokens ==========

    async fn create_refresh_token(
        &self,
        id: reactor_core::ReactorId,
        session_id: &SessionId,
        token_hash: &[u8],
        expires_at: DateTime<Utc>,
    ) -> Result<RefreshToken, AuthError> {
        let row = sqlx::query_as::<_, RefreshTokenRow>(
            r#"
            INSERT INTO reactor_auth.refresh_tokens (id, session_id, token_hash, expires_at)
            VALUES ($1, $2, $3, $4)
            RETURNING id, session_id, token_hash, issued_at, expires_at, used_at, replaced_by
            "#,
        )
        .bind(id.as_uuid())
        .bind(session_id.as_uuid())
        .bind(token_hash)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create refresh token");
            AuthError::Internal
        })?;

        Ok(row.into())
    }

    async fn find_refresh_token_by_hash(
        &self,
        token_hash: &[u8],
    ) -> Result<Option<RefreshToken>, AuthError> {
        let row = sqlx::query_as::<_, RefreshTokenRow>(
            r#"
            SELECT id, session_id, token_hash, issued_at, expires_at, used_at, replaced_by
            FROM reactor_auth.refresh_tokens
            WHERE token_hash = $1
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find refresh token");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn rotate_refresh_token(
        &self,
        old_token_hash: &[u8],
        new_id: reactor_core::ReactorId,
        new_token_hash: &[u8],
        new_expires_at: DateTime<Utc>,
    ) -> Result<RefreshToken, AuthError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            tracing::error!(error = %e, "failed to begin transaction");
            AuthError::Internal
        })?;

        // Find the old token
        let old_token = sqlx::query_as::<_, RefreshTokenRow>(
            r#"
            SELECT id, session_id, token_hash, issued_at, expires_at, used_at, replaced_by
            FROM reactor_auth.refresh_tokens
            WHERE token_hash = $1
            FOR UPDATE
            "#,
        )
        .bind(old_token_hash)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find refresh token for rotation");
            AuthError::Internal
        })?
        .ok_or(AuthError::InvalidRefreshToken)?;

        // Check if already used (token reuse attack)
        if old_token.used_at.is_some() {
            // Token reuse detected - revoke the session
            sqlx::query(
                r#"
                UPDATE reactor_auth.sessions
                SET revoked_at = NOW()
                WHERE id = $1 AND revoked_at IS NULL
                "#,
            )
            .bind(old_token.session_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to revoke session on token reuse");
                AuthError::Internal
            })?;

            tx.commit().await.map_err(|e| {
                tracing::error!(error = %e, "failed to commit transaction");
                AuthError::Internal
            })?;

            return Err(AuthError::RefreshTokenReuse);
        }

        // Check if expired
        if old_token.expires_at < Utc::now() {
            return Err(AuthError::InvalidRefreshToken);
        }

        // Mark old token as used
        sqlx::query(
            r#"
            UPDATE reactor_auth.refresh_tokens
            SET used_at = NOW(), replaced_by = $2
            WHERE id = $1
            "#,
        )
        .bind(old_token.id)
        .bind(new_id.as_uuid())
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to mark old token as used");
            AuthError::Internal
        })?;

        // Create new token
        let new_token = sqlx::query_as::<_, RefreshTokenRow>(
            r#"
            INSERT INTO reactor_auth.refresh_tokens (id, session_id, token_hash, expires_at)
            VALUES ($1, $2, $3, $4)
            RETURNING id, session_id, token_hash, issued_at, expires_at, used_at, replaced_by
            "#,
        )
        .bind(new_id.as_uuid())
        .bind(old_token.session_id)
        .bind(new_token_hash)
        .bind(new_expires_at)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create new refresh token");
            AuthError::Internal
        })?;

        tx.commit().await.map_err(|e| {
            tracing::error!(error = %e, "failed to commit transaction");
            AuthError::Internal
        })?;

        Ok(new_token.into())
    }

    async fn check_refresh_token_reuse(&self, token_hash: &[u8]) -> Result<bool, AuthError> {
        let row: Option<(Option<DateTime<Utc>>,)> = sqlx::query_as(
            r#"
            SELECT used_at
            FROM reactor_auth.refresh_tokens
            WHERE token_hash = $1
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to check refresh token reuse");
            AuthError::Internal
        })?;

        Ok(row.map(|(used_at,)| used_at.is_some()).unwrap_or(false))
    }

    // ========== Organizations ==========

    async fn create_org(
        &self,
        id: OrgId,
        name: &str,
        slug: &str,
        metadata: serde_json::Value,
    ) -> Result<Org, AuthError> {
        let row = sqlx::query_as::<_, OrgRow>(
            r#"
            INSERT INTO reactor_auth.orgs (id, name, slug, metadata)
            VALUES ($1, $2, $3, $4)
            RETURNING id, name, slug, metadata, created_at, updated_at
            "#,
        )
        .bind(id.as_uuid())
        .bind(name)
        .bind(slug)
        .bind(&metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let Some(db_err) = e.as_database_error() {
                if db_err.constraint() == Some("orgs_slug_key") {
                    return AuthError::OrgSlugExists;
                }
            }
            tracing::error!(error = %e, "failed to create org");
            AuthError::Internal
        })?;

        Ok(row.into())
    }

    async fn find_org_by_id(&self, id: &OrgId) -> Result<Option<Org>, AuthError> {
        let row = sqlx::query_as::<_, OrgRow>(
            r#"
            SELECT id, name, slug, metadata, created_at, updated_at
            FROM reactor_auth.orgs
            WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find org by id");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn find_org_by_slug(&self, slug: &str) -> Result<Option<Org>, AuthError> {
        let row = sqlx::query_as::<_, OrgRow>(
            r#"
            SELECT id, name, slug, metadata, created_at, updated_at
            FROM reactor_auth.orgs
            WHERE slug = $1
            "#,
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find org by slug");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn update_org(
        &self,
        id: &OrgId,
        name: Option<&str>,
        slug: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Result<Org, AuthError> {
        let row = sqlx::query_as::<_, OrgRow>(
            r#"
            UPDATE reactor_auth.orgs
            SET
                name = COALESCE($2, name),
                slug = COALESCE($3, slug),
                metadata = COALESCE($4, metadata),
                updated_at = NOW()
            WHERE id = $1
            RETURNING id, name, slug, metadata, created_at, updated_at
            "#,
        )
        .bind(id.as_uuid())
        .bind(name)
        .bind(slug)
        .bind(&metadata)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            if let Some(db_err) = e.as_database_error() {
                if db_err.constraint() == Some("orgs_slug_key") {
                    return AuthError::OrgSlugExists;
                }
            }
            tracing::error!(error = %e, "failed to update org");
            AuthError::Internal
        })?
        .ok_or(AuthError::OrgNotFound)?;

        Ok(row.into())
    }

    async fn delete_org(&self, id: &OrgId) -> Result<(), AuthError> {
        let result = sqlx::query(
            r#"
            DELETE FROM reactor_auth.orgs WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to delete org");
            AuthError::Internal
        })?;

        if result.rows_affected() == 0 {
            return Err(AuthError::OrgNotFound);
        }

        Ok(())
    }

    async fn list_user_orgs(&self, user_id: &UserId) -> Result<Vec<Org>, AuthError> {
        let rows = sqlx::query_as::<_, OrgRow>(
            r#"
            SELECT o.id, o.name, o.slug, o.metadata, o.created_at, o.updated_at
            FROM reactor_auth.orgs o
            JOIN reactor_auth.memberships m ON m.org_id = o.id
            WHERE m.user_id = $1
            ORDER BY o.created_at DESC
            "#,
        )
        .bind(user_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list user orgs");
            AuthError::Internal
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    // ========== Roles ==========

    async fn create_role(
        &self,
        id: RoleId,
        org_id: &OrgId,
        name: &str,
        description: Option<&str>,
        is_system: bool,
        permissions: Vec<String>,
    ) -> Result<Role, AuthError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            tracing::error!(error = %e, "failed to begin transaction");
            AuthError::Internal
        })?;

        let row = sqlx::query_as::<_, RoleRow>(
            r#"
            INSERT INTO reactor_auth.roles (id, org_id, name, description, is_system)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, org_id, name, description, is_system, created_at
            "#,
        )
        .bind(id.as_uuid())
        .bind(org_id.as_uuid())
        .bind(name)
        .bind(description)
        .bind(is_system)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create role");
            AuthError::Internal
        })?;

        // Insert permissions
        for permission in &permissions {
            sqlx::query(
                r#"
                INSERT INTO reactor_auth.role_permissions (role_id, permission)
                VALUES ($1, $2)
                "#,
            )
            .bind(id.as_uuid())
            .bind(permission)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to insert role permission");
                AuthError::Internal
            })?;
        }

        tx.commit().await.map_err(|e| {
            tracing::error!(error = %e, "failed to commit transaction");
            AuthError::Internal
        })?;

        Ok(row.into())
    }

    async fn find_role_by_id(&self, id: &RoleId) -> Result<Option<Role>, AuthError> {
        let row = sqlx::query_as::<_, RoleRow>(
            r#"
            SELECT id, org_id, name, description, is_system, created_at
            FROM reactor_auth.roles
            WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find role by id");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn find_role_by_name(
        &self,
        org_id: &OrgId,
        name: &str,
    ) -> Result<Option<Role>, AuthError> {
        let row = sqlx::query_as::<_, RoleRow>(
            r#"
            SELECT id, org_id, name, description, is_system, created_at
            FROM reactor_auth.roles
            WHERE org_id = $1 AND name = $2
            "#,
        )
        .bind(org_id.as_uuid())
        .bind(name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find role by name");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn update_role(
        &self,
        id: &RoleId,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<Role, AuthError> {
        let row = sqlx::query_as::<_, RoleRow>(
            r#"
            UPDATE reactor_auth.roles
            SET
                name = COALESCE($2, name),
                description = COALESCE($3, description)
            WHERE id = $1 AND is_system = FALSE
            RETURNING id, org_id, name, description, is_system, created_at
            "#,
        )
        .bind(id.as_uuid())
        .bind(name)
        .bind(description)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to update role");
            AuthError::Internal
        })?
        .ok_or(AuthError::RoleNotFound)?;

        Ok(row.into())
    }

    async fn delete_role(&self, id: &RoleId) -> Result<(), AuthError> {
        let result = sqlx::query(
            r#"
            DELETE FROM reactor_auth.roles WHERE id = $1 AND is_system = FALSE
            "#,
        )
        .bind(id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to delete role");
            AuthError::Internal
        })?;

        if result.rows_affected() == 0 {
            return Err(AuthError::RoleNotFound);
        }

        Ok(())
    }

    async fn list_org_roles(&self, org_id: &OrgId) -> Result<Vec<Role>, AuthError> {
        let rows = sqlx::query_as::<_, RoleRow>(
            r#"
            SELECT id, org_id, name, description, is_system, created_at
            FROM reactor_auth.roles
            WHERE org_id = $1
            ORDER BY is_system DESC, name ASC
            "#,
        )
        .bind(org_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list org roles");
            AuthError::Internal
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn list_roles_with_permissions(
        &self,
        org_id: &OrgId,
    ) -> Result<Vec<(Role, Vec<String>)>, AuthError> {
        let roles = self.list_org_roles(org_id).await?;
        let mut result = Vec::with_capacity(roles.len());

        for role in roles {
            let permissions = self.get_role_permissions(&role.id).await?;
            result.push((role, permissions));
        }

        Ok(result)
    }

    async fn get_role_permissions(&self, role_id: &RoleId) -> Result<Vec<String>, AuthError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT permission
            FROM reactor_auth.role_permissions
            WHERE role_id = $1
            ORDER BY permission
            "#,
        )
        .bind(role_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get role permissions");
            AuthError::Internal
        })?;

        Ok(rows.into_iter().map(|(p,)| p).collect())
    }

    async fn set_role_permissions(
        &self,
        role_id: &RoleId,
        permissions: Vec<String>,
    ) -> Result<(), AuthError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            tracing::error!(error = %e, "failed to begin transaction");
            AuthError::Internal
        })?;

        // Delete existing permissions
        sqlx::query(
            r#"
            DELETE FROM reactor_auth.role_permissions WHERE role_id = $1
            "#,
        )
        .bind(role_id.as_uuid())
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to delete role permissions");
            AuthError::Internal
        })?;

        // Insert new permissions
        for permission in &permissions {
            sqlx::query(
                r#"
                INSERT INTO reactor_auth.role_permissions (role_id, permission)
                VALUES ($1, $2)
                "#,
            )
            .bind(role_id.as_uuid())
            .bind(permission)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to insert role permission");
                AuthError::Internal
            })?;
        }

        tx.commit().await.map_err(|e| {
            tracing::error!(error = %e, "failed to commit transaction");
            AuthError::Internal
        })?;

        Ok(())
    }

    // ========== Memberships ==========

    async fn create_membership(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
        role_id: &RoleId,
    ) -> Result<Membership, AuthError> {
        let row = sqlx::query_as::<_, MembershipRow>(
            r#"
            INSERT INTO reactor_auth.memberships (user_id, org_id, role_id)
            VALUES ($1, $2, $3)
            RETURNING user_id, org_id, role_id, joined_at
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(org_id.as_uuid())
        .bind(role_id.as_uuid())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let Some(db_err) = e.as_database_error() {
                if db_err.constraint() == Some("memberships_pkey") {
                    return AuthError::MembershipExists;
                }
            }
            tracing::error!(error = %e, "failed to create membership");
            AuthError::Internal
        })?;

        Ok(row.into())
    }

    async fn find_membership(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
    ) -> Result<Option<Membership>, AuthError> {
        let row = sqlx::query_as::<_, MembershipRow>(
            r#"
            SELECT user_id, org_id, role_id, joined_at
            FROM reactor_auth.memberships
            WHERE user_id = $1 AND org_id = $2
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(org_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find membership");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn update_membership_role(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
        role_id: &RoleId,
    ) -> Result<Membership, AuthError> {
        let row = sqlx::query_as::<_, MembershipRow>(
            r#"
            UPDATE reactor_auth.memberships
            SET role_id = $3
            WHERE user_id = $1 AND org_id = $2
            RETURNING user_id, org_id, role_id, joined_at
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(org_id.as_uuid())
        .bind(role_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to update membership role");
            AuthError::Internal
        })?
        .ok_or(AuthError::MembershipNotFound)?;

        Ok(row.into())
    }

    async fn delete_membership(&self, user_id: &UserId, org_id: &OrgId) -> Result<(), AuthError> {
        let result = sqlx::query(
            r#"
            DELETE FROM reactor_auth.memberships WHERE user_id = $1 AND org_id = $2
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(org_id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to delete membership");
            AuthError::Internal
        })?;

        if result.rows_affected() == 0 {
            return Err(AuthError::MembershipNotFound);
        }

        Ok(())
    }

    async fn list_org_members(
        &self,
        org_id: &OrgId,
    ) -> Result<Vec<(User, Membership, Role)>, AuthError> {
        let rows = sqlx::query_as::<_, OrgMemberRow>(
            r#"
            SELECT 
                u.id as user_id, u.email, u.email_verified, u.metadata as user_metadata, 
                u.default_org_id, u.disabled_at, u.created_at as user_created_at, u.updated_at as user_updated_at,
                m.org_id, m.role_id, m.joined_at,
                r.id as role_id_2, r.name as role_name, r.description as role_description, 
                r.is_system, r.created_at as role_created_at
            FROM reactor_auth.memberships m
            JOIN reactor_auth.users u ON u.id = m.user_id
            JOIN reactor_auth.roles r ON r.id = m.role_id
            WHERE m.org_id = $1
            ORDER BY m.joined_at ASC
            "#,
        )
        .bind(org_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list org members");
            AuthError::Internal
        })?;

        Ok(rows.into_iter().map(|row| row.into_tuple()).collect())
    }

    async fn count_members_with_role(
        &self,
        org_id: &OrgId,
        role_id: &RoleId,
    ) -> Result<u64, AuthError> {
        let (count,): (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM reactor_auth.memberships
            WHERE org_id = $1 AND role_id = $2
            "#,
        )
        .bind(org_id.as_uuid())
        .bind(role_id.as_uuid())
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to count members with role");
            AuthError::Internal
        })?;

        Ok(count as u64)
    }

    async fn get_user_permissions(
        &self,
        user_id: &UserId,
        org_id: &OrgId,
    ) -> Result<Vec<String>, AuthError> {
        let rows: Vec<(String,)> = sqlx::query_as(
            r#"
            SELECT rp.permission
            FROM reactor_auth.memberships m
            JOIN reactor_auth.role_permissions rp ON rp.role_id = m.role_id
            WHERE m.user_id = $1 AND m.org_id = $2
            ORDER BY rp.permission
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(org_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get user permissions");
            AuthError::Internal
        })?;

        Ok(rows.into_iter().map(|(p,)| p).collect())
    }

    // ========== Invitations ==========

    async fn create_invitation(
        &self,
        token_hash: &[u8],
        email: &str,
        org_id: &OrgId,
        role_id: &RoleId,
        expires_at: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        // Use the hash of the token as the ID
        let id = reactor_core::ReactorId::new();

        sqlx::query(
            r#"
            INSERT INTO reactor_auth.invitations (id, token_hash, email, org_id, role_id, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(id.as_uuid())
        .bind(token_hash)
        .bind(email)
        .bind(org_id.as_uuid())
        .bind(role_id.as_uuid())
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create invitation");
            AuthError::Internal
        })?;

        Ok(())
    }

    async fn find_invitation_by_hash(
        &self,
        token_hash: &[u8],
    ) -> Result<Option<Invitation>, AuthError> {
        let row = sqlx::query_as::<_, InvitationRow>(
            r#"
            SELECT id, email, org_id, role_id, created_at, expires_at
            FROM reactor_auth.invitations
            WHERE token_hash = $1 AND used_at IS NULL
            "#,
        )
        .bind(token_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find invitation");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn use_invitation(&self, token_hash: &[u8]) -> Result<(), AuthError> {
        let result = sqlx::query(
            r#"
            UPDATE reactor_auth.invitations
            SET used_at = NOW()
            WHERE token_hash = $1 AND used_at IS NULL
            "#,
        )
        .bind(token_hash)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to use invitation");
            AuthError::Internal
        })?;

        if result.rows_affected() == 0 {
            return Err(AuthError::InvitationNotFound);
        }

        Ok(())
    }

    async fn list_org_invitations(&self, org_id: &OrgId) -> Result<Vec<Invitation>, AuthError> {
        let rows = sqlx::query_as::<_, InvitationRow>(
            r#"
            SELECT id, email, org_id, role_id, created_at, expires_at
            FROM reactor_auth.invitations
            WHERE org_id = $1 AND used_at IS NULL AND expires_at > NOW()
            ORDER BY created_at DESC
            "#,
        )
        .bind(org_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list org invitations");
            AuthError::Internal
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn delete_invitation(&self, token_hash: &[u8]) -> Result<(), AuthError> {
        let result = sqlx::query(
            r#"
            DELETE FROM reactor_auth.invitations WHERE token_hash = $1
            "#,
        )
        .bind(token_hash)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to delete invitation");
            AuthError::Internal
        })?;

        if result.rows_affected() == 0 {
            return Err(AuthError::InvitationNotFound);
        }

        Ok(())
    }

    // ========== Signing Keys ==========

    async fn store_signing_key(&self, key: &SigningKey) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            INSERT INTO reactor_auth.signing_keys
                (kid, algorithm, private_key_pem, public_key_pem, created_at, activated_at, rotated_at, retired_at)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(&key.kid)
        .bind(&key.algorithm)
        .bind(&key.private_key_pem)
        .bind(&key.public_key_pem)
        .bind(key.created_at)
        .bind(key.activated_at)
        .bind(key.rotated_at)
        .bind(key.retired_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to store signing key");
            AuthError::Internal
        })?;

        Ok(())
    }

    async fn get_active_signing_key(&self) -> Result<Option<SigningKey>, AuthError> {
        let row = sqlx::query_as::<_, SigningKeyRow>(
            r#"
            SELECT kid, algorithm, private_key_pem, public_key_pem, created_at, activated_at, rotated_at, retired_at
            FROM reactor_auth.signing_keys
            WHERE rotated_at IS NULL AND retired_at IS NULL
            ORDER BY activated_at DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get active signing key");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn get_jwks_keys(&self) -> Result<Vec<SigningKey>, AuthError> {
        let rows = sqlx::query_as::<_, SigningKeyRow>(
            r#"
            SELECT kid, algorithm, private_key_pem, public_key_pem, created_at, activated_at, rotated_at, retired_at
            FROM reactor_auth.signing_keys
            WHERE retired_at IS NULL
            ORDER BY activated_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get JWKS keys");
            AuthError::Internal
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn rotate_signing_key(
        &self,
        old_kid: &str,
        new_key: &SigningKey,
    ) -> Result<(), AuthError> {
        let mut tx = self.pool.begin().await.map_err(|e| {
            tracing::error!(error = %e, "failed to begin transaction");
            AuthError::Internal
        })?;

        // Mark old key as rotated
        sqlx::query(
            r#"
            UPDATE reactor_auth.signing_keys
            SET rotated_at = NOW()
            WHERE kid = $1 AND rotated_at IS NULL
            "#,
        )
        .bind(old_kid)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to mark old key as rotated");
            AuthError::Internal
        })?;

        // Insert new key
        sqlx::query(
            r#"
            INSERT INTO reactor_auth.signing_keys
                (kid, algorithm, private_key_pem, public_key_pem, created_at, activated_at, rotated_at, retired_at)
            VALUES
                ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(&new_key.kid)
        .bind(&new_key.algorithm)
        .bind(&new_key.private_key_pem)
        .bind(&new_key.public_key_pem)
        .bind(new_key.created_at)
        .bind(new_key.activated_at)
        .bind(new_key.rotated_at)
        .bind(new_key.retired_at)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to insert new signing key");
            AuthError::Internal
        })?;

        tx.commit().await.map_err(|e| {
            tracing::error!(error = %e, "failed to commit transaction");
            AuthError::Internal
        })?;

        Ok(())
    }

    async fn retire_old_signing_keys(&self) -> Result<u64, AuthError> {
        let result = sqlx::query(
            r#"
            UPDATE reactor_auth.signing_keys
            SET retired_at = NOW()
            WHERE rotated_at IS NOT NULL
              AND retired_at IS NULL
              AND rotated_at < NOW() - INTERVAL '7 days'
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to retire old signing keys");
            AuthError::Internal
        })?;

        Ok(result.rows_affected())
    }

    // ========== Verification Tokens ==========

    async fn create_verification_token(
        &self,
        user_id: &UserId,
        token_hash: &[u8],
        token_type: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<super::VerificationToken, AuthError> {
        let id = reactor_core::ReactorId::new();
        let now = Utc::now();

        sqlx::query(
            r#"
            INSERT INTO reactor_auth.verification_tokens 
                (id, user_id, token_hash, token_type, created_at, expires_at)
            VALUES 
                ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(id.as_uuid())
        .bind(user_id.into_uuid())
        .bind(token_hash)
        .bind(token_type)
        .bind(now)
        .bind(expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create verification token");
            AuthError::Internal
        })?;

        Ok(super::VerificationToken {
            id,
            user_id: *user_id,
            token_type: token_type.to_string(),
            created_at: now,
            expires_at,
            used_at: None,
        })
    }

    async fn find_verification_token_by_hash(
        &self,
        token_hash: &[u8],
        token_type: &str,
    ) -> Result<Option<super::VerificationToken>, AuthError> {
        let row = sqlx::query_as::<_, VerificationTokenRow>(
            r#"
            SELECT id, user_id, token_type, created_at, expires_at, used_at
            FROM reactor_auth.verification_tokens
            WHERE token_hash = $1 AND token_type = $2 AND used_at IS NULL AND expires_at > NOW()
            "#,
        )
        .bind(token_hash)
        .bind(token_type)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find verification token");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn use_verification_token(&self, token_hash: &[u8]) -> Result<(), AuthError> {
        let result = sqlx::query(
            r#"
            UPDATE reactor_auth.verification_tokens
            SET used_at = NOW()
            WHERE token_hash = $1 AND used_at IS NULL
            "#,
        )
        .bind(token_hash)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to use verification token");
            AuthError::Internal
        })?;

        if result.rows_affected() == 0 {
            return Err(AuthError::InvalidToken);
        }

        Ok(())
    }

    async fn cleanup_verification_tokens(&self, user_id: &UserId) -> Result<u64, AuthError> {
        let result = sqlx::query(
            r#"
            DELETE FROM reactor_auth.verification_tokens
            WHERE user_id = $1 AND (used_at IS NOT NULL OR expires_at < NOW())
            "#,
        )
        .bind(user_id.into_uuid())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to cleanup verification tokens");
            AuthError::Internal
        })?;

        Ok(result.rows_affected())
    }

    // ========== API Keys ==========

    async fn create_api_key(
        &self,
        id: reactor_core::ReactorId,
        user_id: &UserId,
        name: &str,
        key_hash: &[u8],
        prefix: &str,
        scopes: Option<Vec<String>>,
    ) -> Result<ApiKey, AuthError> {
        let scopes_json = scopes.as_ref().map(|s| serde_json::json!(s));
        let row = sqlx::query_as::<_, ApiKeyRow>(
            r#"
            INSERT INTO reactor_auth.api_keys (id, user_id, name, key_hash, prefix, scopes)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, user_id, name, prefix, scopes, created_at, last_used_at, revoked_at
            "#,
        )
        .bind(id.as_uuid())
        .bind(user_id.as_uuid())
        .bind(name)
        .bind(key_hash)
        .bind(prefix)
        .bind(scopes_json)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create API key");
            AuthError::Internal
        })?;

        Ok(row.into())
    }

    async fn find_api_key_by_hash(&self, key_hash: &[u8]) -> Result<Option<ApiKey>, AuthError> {
        let row = sqlx::query_as::<_, ApiKeyRow>(
            r#"
            SELECT id, user_id, name, prefix, scopes, created_at, last_used_at, revoked_at
            FROM reactor_auth.api_keys
            WHERE key_hash = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(key_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find API key by hash");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn list_user_api_keys(&self, user_id: &UserId) -> Result<Vec<ApiKey>, AuthError> {
        let rows = sqlx::query_as::<_, ApiKeyRow>(
            r#"
            SELECT id, user_id, name, prefix, scopes, created_at, last_used_at, revoked_at
            FROM reactor_auth.api_keys
            WHERE user_id = $1 AND revoked_at IS NULL
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list user API keys");
            AuthError::Internal
        })?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    async fn revoke_api_key(
        &self,
        id: &reactor_core::ReactorId,
        user_id: &UserId,
    ) -> Result<(), AuthError> {
        let result = sqlx::query(
            r#"
            UPDATE reactor_auth.api_keys
            SET revoked_at = now()
            WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL
            "#,
        )
        .bind(id.as_uuid())
        .bind(user_id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to revoke API key");
            AuthError::Internal
        })?;

        if result.rows_affected() == 0 {
            return Err(AuthError::InvalidToken);
        }
        Ok(())
    }

    async fn touch_api_key(&self, id: &reactor_core::ReactorId) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            UPDATE reactor_auth.api_keys
            SET last_used_at = now()
            WHERE id = $1
            "#,
        )
        .bind(id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to touch API key");
            AuthError::Internal
        })?;

        Ok(())
    }

    // ========== Audit Events ==========

    async fn log_audit(&self, event: &super::AuditEvent) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            INSERT INTO reactor_auth.audit_events 
                (id, ts, actor_user_id, actor_apikey_id, org_id, event_type, resource, ip, user_agent, details)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8::inet, $9, $10)
            "#,
        )
        .bind(event.id.as_uuid())
        .bind(event.ts)
        .bind(event.actor_user_id.map(|id| id.into_uuid()))
        .bind(event.actor_apikey_id.map(|id| id.into_uuid()))
        .bind(event.org_id.map(|id| id.into_uuid()))
        .bind(&event.event_type)
        .bind(&event.resource)
        .bind(&event.ip)
        .bind(&event.user_agent)
        .bind(&event.details)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to log audit event");
            AuthError::Internal
        })?;

        Ok(())
    }

    // ========== Platform Operators ==========

    async fn grant_platform_role(
        &self,
        user_id: &UserId,
        role_name: &str,
        granted_by: Option<&UserId>,
    ) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            INSERT INTO reactor_auth.platform_memberships (user_id, role_id, granted_by)
            SELECT $1, pr.id, $3
            FROM reactor_auth.platform_roles pr
            WHERE pr.name = $2
            ON CONFLICT (user_id, role_id) DO UPDATE SET
                revoked_at = NULL,
                granted_by = $3,
                granted_at = NOW()
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(role_name)
        .bind(granted_by.map(|u| u.as_uuid()))
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to grant platform role");
            AuthError::Internal
        })?;

        Ok(())
    }

    async fn revoke_platform_role(
        &self,
        user_id: &UserId,
        role_name: &str,
    ) -> Result<(), AuthError> {
        sqlx::query(
            r#"
            UPDATE reactor_auth.platform_memberships pm
            SET revoked_at = NOW()
            FROM reactor_auth.platform_roles pr
            WHERE pm.role_id = pr.id
                AND pm.user_id = $1
                AND pr.name = $2
                AND pm.revoked_at IS NULL
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(role_name)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to revoke platform role");
            AuthError::Internal
        })?;

        Ok(())
    }

    async fn has_platform_role(
        &self,
        user_id: &UserId,
        role_name: &str,
    ) -> Result<bool, AuthError> {
        let result: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT 1
            FROM reactor_auth.platform_memberships pm
            JOIN reactor_auth.platform_roles pr ON pm.role_id = pr.id
            WHERE pm.user_id = $1
                AND pr.name = $2
                AND pm.revoked_at IS NULL
            "#,
        )
        .bind(user_id.as_uuid())
        .bind(role_name)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to check platform role");
            AuthError::Internal
        })?;

        Ok(result.is_some())
    }

    async fn list_platform_role_users(
        &self,
        role_name: &str,
    ) -> Result<Vec<UserId>, AuthError> {
        let rows: Vec<(uuid::Uuid,)> = sqlx::query_as(
            r#"
            SELECT pm.user_id
            FROM reactor_auth.platform_memberships pm
            JOIN reactor_auth.platform_roles pr ON pm.role_id = pr.id
            WHERE pr.name = $1
                AND pm.revoked_at IS NULL
            ORDER BY pm.granted_at ASC
            "#,
        )
        .bind(role_name)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list platform role users");
            AuthError::Internal
        })?;

        Ok(rows.into_iter().map(|(id,)| id.into()).collect())
    }

    async fn get_platform_permissions(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<(String, bool)>, AuthError> {
        let rows: Vec<(String, bool)> = sqlx::query_as(
            r#"
            SELECT prp.permission, prp.requires_step_up
            FROM reactor_auth.platform_memberships pm
            JOIN reactor_auth.platform_role_permissions prp ON pm.role_id = prp.role_id
            WHERE pm.user_id = $1
                AND pm.revoked_at IS NULL
            "#,
        )
        .bind(user_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get platform permissions");
            AuthError::Internal
        })?;

        Ok(rows)
    }

    async fn platform_operators_exist(&self) -> Result<bool, AuthError> {
        let result: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT 1
            FROM reactor_auth.platform_memberships pm
            JOIN reactor_auth.platform_roles pr ON pm.role_id = pr.id
            WHERE pr.name = 'platform_operator'
                AND pm.revoked_at IS NULL
            LIMIT 1
            "#,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to check platform operators exist");
            AuthError::Internal
        })?;

        Ok(result.is_some())
    }

    // ========== Authorization Codes (PKCE) ==========

    async fn create_authorization_code(
        &self,
        id: reactor_core::ReactorId,
        user_id: &UserId,
        client_id: &str,
        redirect_uri: &str,
        scopes: Vec<String>,
        code_hash: &[u8],
        code_challenge: &str,
        code_challenge_method: &str,
        nonce: Option<&str>,
        state: Option<&str>,
        expires_at: DateTime<Utc>,
        session_id: Option<&SessionId>,
    ) -> Result<super::AuthorizationCode, AuthError> {
        let scopes_json = serde_json::to_value(&scopes).map_err(|e| {
            tracing::error!(error = %e, "failed to serialize scopes");
            AuthError::Internal
        })?;

        let row = sqlx::query_as::<_, AuthorizationCodeRow>(
            r#"
            INSERT INTO reactor_auth.authorization_codes 
                (id, code_hash, user_id, client_id, redirect_uri, scopes, code_challenge, code_challenge_method, nonce, state, expires_at, session_id)
            VALUES 
                ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
            RETURNING id, user_id, client_id, redirect_uri, scopes, code_challenge, code_challenge_method, nonce, state, created_at, expires_at, used_at, session_id
            "#,
        )
        .bind(id.as_uuid())
        .bind(code_hash)
        .bind(user_id.as_uuid())
        .bind(client_id)
        .bind(redirect_uri)
        .bind(&scopes_json)
        .bind(code_challenge)
        .bind(code_challenge_method)
        .bind(nonce)
        .bind(state)
        .bind(expires_at)
        .bind(session_id.map(|s| s.as_uuid()))
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create authorization code");
            AuthError::Internal
        })?;

        Ok(row.into())
    }

    async fn find_authorization_code_by_hash(
        &self,
        code_hash: &[u8],
    ) -> Result<Option<super::AuthorizationCode>, AuthError> {
        let row = sqlx::query_as::<_, AuthorizationCodeRow>(
            r#"
            SELECT id, user_id, client_id, redirect_uri, scopes, code_challenge, code_challenge_method, nonce, state, created_at, expires_at, used_at, session_id
            FROM reactor_auth.authorization_codes
            WHERE code_hash = $1 AND used_at IS NULL AND expires_at > NOW()
            "#,
        )
        .bind(code_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find authorization code");
            AuthError::Internal
        })?;

        Ok(row.map(Into::into))
    }

    async fn use_authorization_code(
        &self,
        code_hash: &[u8],
        session_id: &SessionId,
    ) -> Result<(), AuthError> {
        let result = sqlx::query(
            r#"
            UPDATE reactor_auth.authorization_codes
            SET used_at = NOW(), session_id = $2
            WHERE code_hash = $1 AND used_at IS NULL AND expires_at > NOW()
            "#,
        )
        .bind(code_hash)
        .bind(session_id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to use authorization code");
            AuthError::Internal
        })?;

        if result.rows_affected() == 0 {
            return Err(AuthError::InvalidToken);
        }

        Ok(())
    }

    async fn cleanup_expired_authorization_codes(&self) -> Result<u64, AuthError> {
        let result = sqlx::query(
            r#"
            DELETE FROM reactor_auth.authorization_codes
            WHERE expires_at < NOW() OR used_at IS NOT NULL
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to cleanup expired authorization codes");
            AuthError::Internal
        })?;

        Ok(result.rows_affected())
    }
}

/// Row type for signing keys.
#[derive(sqlx::FromRow)]
struct SigningKeyRow {
    kid: String,
    algorithm: String,
    private_key_pem: String,
    public_key_pem: String,
    created_at: chrono::DateTime<chrono::Utc>,
    activated_at: chrono::DateTime<chrono::Utc>,
    rotated_at: Option<chrono::DateTime<chrono::Utc>>,
    retired_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<SigningKeyRow> for SigningKey {
    fn from(row: SigningKeyRow) -> Self {
        Self {
            kid: row.kid,
            algorithm: row.algorithm,
            private_key_pem: row.private_key_pem,
            public_key_pem: row.public_key_pem,
            created_at: row.created_at,
            activated_at: row.activated_at,
            rotated_at: row.rotated_at,
            retired_at: row.retired_at,
        }
    }
}

/// Row type for users.
#[derive(sqlx::FromRow)]
struct UserRow {
    id: uuid::Uuid,
    email: String,
    email_verified: bool,
    metadata: serde_json::Value,
    default_org_id: Option<uuid::Uuid>,
    disabled_at: Option<chrono::DateTime<chrono::Utc>>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id.into(),
            email: row.email,
            email_verified: row.email_verified,
            default_org_id: row.default_org_id.map(Into::into),
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
            disabled_at: row.disabled_at,
        }
    }
}

/// Row type for sessions.
#[derive(sqlx::FromRow)]
struct SessionRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    amr: Vec<String>,
    ip: Option<String>,
    user_agent: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_seen_at: chrono::DateTime<chrono::Utc>,
    revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<SessionRow> for Session {
    fn from(row: SessionRow) -> Self {
        Self {
            id: row.id.into(),
            user_id: row.user_id.into(),
            amr: row.amr,
            ip: row.ip,
            user_agent: row.user_agent,
            created_at: row.created_at,
            last_seen_at: row.last_seen_at,
            revoked_at: row.revoked_at,
        }
    }
}

/// Row type for refresh tokens.
#[derive(sqlx::FromRow)]
struct RefreshTokenRow {
    id: uuid::Uuid,
    session_id: uuid::Uuid,
    token_hash: Vec<u8>,
    issued_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
    used_at: Option<chrono::DateTime<chrono::Utc>>,
    replaced_by: Option<uuid::Uuid>,
}

impl From<RefreshTokenRow> for super::RefreshToken {
    fn from(row: RefreshTokenRow) -> Self {
        Self {
            id: row.id.into(),
            session_id: row.session_id.into(),
            token_hash: row.token_hash,
            issued_at: row.issued_at,
            expires_at: row.expires_at,
            used_at: row.used_at,
            replaced_by: row.replaced_by.map(Into::into),
        }
    }
}

/// Row type for organizations.
#[derive(sqlx::FromRow)]
struct OrgRow {
    id: uuid::Uuid,
    name: String,
    slug: String,
    metadata: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<OrgRow> for Org {
    fn from(row: OrgRow) -> Self {
        Self {
            id: row.id.into(),
            name: row.name,
            slug: row.slug,
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Row type for roles.
#[derive(sqlx::FromRow)]
struct RoleRow {
    id: uuid::Uuid,
    org_id: uuid::Uuid,
    name: String,
    description: Option<String>,
    is_system: bool,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl From<RoleRow> for Role {
    fn from(row: RoleRow) -> Self {
        Self {
            id: row.id.into(),
            org_id: row.org_id.into(),
            name: row.name,
            description: row.description,
            is_system: row.is_system,
            created_at: row.created_at,
        }
    }
}

/// Row type for memberships.
#[derive(sqlx::FromRow)]
struct MembershipRow {
    user_id: uuid::Uuid,
    org_id: uuid::Uuid,
    role_id: uuid::Uuid,
    joined_at: chrono::DateTime<chrono::Utc>,
}

impl From<MembershipRow> for Membership {
    fn from(row: MembershipRow) -> Self {
        Self {
            user_id: row.user_id.into(),
            org_id: row.org_id.into(),
            role_id: row.role_id.into(),
            joined_at: row.joined_at,
        }
    }
}

/// Row type for org members query (user + membership + role).
#[derive(sqlx::FromRow)]
struct OrgMemberRow {
    // User fields
    user_id: uuid::Uuid,
    email: String,
    email_verified: bool,
    user_metadata: serde_json::Value,
    default_org_id: Option<uuid::Uuid>,
    disabled_at: Option<chrono::DateTime<chrono::Utc>>,
    user_created_at: chrono::DateTime<chrono::Utc>,
    user_updated_at: chrono::DateTime<chrono::Utc>,
    // Membership fields
    org_id: uuid::Uuid,
    role_id: uuid::Uuid,
    joined_at: chrono::DateTime<chrono::Utc>,
    // Role fields
    role_id_2: uuid::Uuid,
    role_name: String,
    role_description: Option<String>,
    is_system: bool,
    role_created_at: chrono::DateTime<chrono::Utc>,
}

impl OrgMemberRow {
    fn into_tuple(self) -> (User, Membership, Role) {
        let user = User {
            id: self.user_id.into(),
            email: self.email,
            email_verified: self.email_verified,
            default_org_id: self.default_org_id.map(Into::into),
            metadata: self.user_metadata,
            created_at: self.user_created_at,
            updated_at: self.user_updated_at,
            disabled_at: self.disabled_at,
        };
        let membership = Membership {
            user_id: self.user_id.into(),
            org_id: self.org_id.into(),
            role_id: self.role_id.into(),
            joined_at: self.joined_at,
        };
        let role = Role {
            id: self.role_id_2.into(),
            org_id: self.org_id.into(),
            name: self.role_name,
            description: self.role_description,
            is_system: self.is_system,
            created_at: self.role_created_at,
        };
        (user, membership, role)
    }
}

/// Row type for invitations.
#[derive(sqlx::FromRow)]
struct InvitationRow {
    id: uuid::Uuid,
    email: String,
    org_id: uuid::Uuid,
    role_id: uuid::Uuid,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
}

impl From<InvitationRow> for Invitation {
    fn from(row: InvitationRow) -> Self {
        Self {
            id: row.id.into(),
            email: row.email,
            org_id: row.org_id.into(),
            role_id: row.role_id.into(),
            created_at: row.created_at,
            expires_at: row.expires_at,
        }
    }
}

/// Row type for verification tokens.
#[derive(sqlx::FromRow)]
struct VerificationTokenRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    token_type: String,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
    used_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<VerificationTokenRow> for super::VerificationToken {
    fn from(row: VerificationTokenRow) -> Self {
        Self {
            id: row.id.into(),
            user_id: row.user_id.into(),
            token_type: row.token_type,
            created_at: row.created_at,
            expires_at: row.expires_at,
            used_at: row.used_at,
        }
    }
}

/// Row type for API keys.
#[derive(sqlx::FromRow)]
struct ApiKeyRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    name: String,
    prefix: String,
    scopes: Option<serde_json::Value>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_used_at: Option<chrono::DateTime<chrono::Utc>>,
    revoked_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl From<ApiKeyRow> for ApiKey {
    fn from(row: ApiKeyRow) -> Self {
        let scopes = row.scopes.and_then(|v| {
            serde_json::from_value::<Vec<String>>(v).ok()
        });
        Self {
            id: row.id.into(),
            user_id: row.user_id.into(),
            name: row.name,
            prefix: row.prefix,
            scopes,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
            revoked_at: row.revoked_at,
        }
    }
}

/// Row type for authorization codes.
#[derive(sqlx::FromRow)]
struct AuthorizationCodeRow {
    id: uuid::Uuid,
    user_id: uuid::Uuid,
    client_id: String,
    redirect_uri: String,
    scopes: serde_json::Value,
    code_challenge: String,
    code_challenge_method: String,
    nonce: Option<String>,
    state: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: chrono::DateTime<chrono::Utc>,
    used_at: Option<chrono::DateTime<chrono::Utc>>,
    session_id: Option<uuid::Uuid>,
}

impl From<AuthorizationCodeRow> for super::AuthorizationCode {
    fn from(row: AuthorizationCodeRow) -> Self {
        let scopes = serde_json::from_value::<Vec<String>>(row.scopes).unwrap_or_default();
        Self {
            id: row.id.into(),
            user_id: row.user_id.into(),
            client_id: row.client_id,
            redirect_uri: row.redirect_uri,
            scopes,
            code_challenge: row.code_challenge,
            code_challenge_method: row.code_challenge_method,
            nonce: row.nonce,
            state: row.state,
            created_at: row.created_at,
            expires_at: row.expires_at,
            used_at: row.used_at,
            session_id: row.session_id.map(Into::into),
        }
    }
}
