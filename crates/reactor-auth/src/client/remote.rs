//! Remote AuthClient implementation.
//!
//! This client makes HTTP requests to a remote reactor-auth service.
//! Used in distributed deployments where auth is a separate microservice.

use async_trait::async_trait;
use reactor_core::auth::{AuthClient, AuthCtx, AuthError, Claims, Jwks, OrgRef, User};
use reactor_core::id::UserId;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};

/// Remote AuthClient that calls reactor-auth via HTTP.
pub struct RemoteAuthClient {
    client: reqwest::Client,
    base_url: String,
}

impl RemoteAuthClient {
    /// Create a new remote auth client.
    pub fn new(base_url: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");

        Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    fn auth_header(token: &str) -> Result<HeaderMap, AuthError> {
        let mut headers = HeaderMap::new();
        let value = HeaderValue::from_str(&format!("Bearer {}", token))
            .map_err(|_| AuthError::InvalidToken)?;
        headers.insert(AUTHORIZATION, value);
        Ok(headers)
    }
}

#[async_trait]
impl AuthClient for RemoteAuthClient {
    async fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
        // Use the resolve_ctx endpoint which returns claims in the AuthCtx
        let ctx = self.resolve_ctx(token, None).await?;
        Ok(ctx.claims)
    }

    async fn resolve_ctx(
        &self,
        token: &str,
        requested_org: Option<&OrgRef>,
    ) -> Result<AuthCtx, AuthError> {
        let mut headers = Self::auth_header(token)?;

        // Pass OrgRef (UUID or slug) to the server; resolution happens there
        if let Some(org_ref) = requested_org {
            headers.insert(
                "x-reactor-org",
                HeaderValue::from_str(&org_ref.to_string()).map_err(|_| AuthError::Internal)?,
            );
        }

        let url = format!("{}/_internal/resolve_ctx", self.base_url);

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "failed to call resolve_ctx");
                AuthError::Internal
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(match status {
                401 => AuthError::Unauthorized,
                403 => AuthError::PermissionDenied,
                404 => AuthError::UserNotFound,
                _ => AuthError::Internal,
            });
        }

        #[derive(serde::Deserialize)]
        struct ResolveCtxResponse {
            ctx: AuthCtx,
        }

        let res: ResolveCtxResponse = response.json().await.map_err(|e| {
            tracing::error!(error = %e, "failed to parse resolve_ctx response");
            AuthError::Internal
        })?;

        Ok(res.ctx)
    }

    async fn get_user(&self, user_id: &UserId) -> Result<User, AuthError> {
        let url = format!("{}/auth/v1/users/{}", self.base_url, user_id);

        let response = self.client.get(&url).send().await.map_err(|e| {
            tracing::error!(error = %e, "failed to get user");
            AuthError::Internal
        })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(match status {
                404 => AuthError::UserNotFound,
                _ => AuthError::Internal,
            });
        }

        response.json().await.map_err(|e| {
            tracing::error!(error = %e, "failed to parse user response");
            AuthError::Internal
        })
    }

    async fn check_permission(&self, ctx: &AuthCtx, permission: &str) -> Result<bool, AuthError> {
        // For remote client, we already have the ctx with permissions pre-loaded
        // So we can just check locally
        Ok(ctx.has_permission(permission))
    }

    async fn jwks(&self) -> Result<Jwks, AuthError> {
        let url = format!("{}/auth/v1/keys", self.base_url);

        let response = self.client.get(&url).send().await.map_err(|e| {
            tracing::error!(error = %e, "failed to get JWKS");
            AuthError::Internal
        })?;

        if !response.status().is_success() {
            return Err(AuthError::Internal);
        }

        response.json().await.map_err(|e| {
            tracing::error!(error = %e, "failed to parse JWKS response");
            AuthError::Internal
        })
    }
}
