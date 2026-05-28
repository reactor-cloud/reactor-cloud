//! Request extractors for authentication.

use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use reactor_core::auth::{AuthError, Claims};
use reactor_core::error::ErrorResponse;
use std::sync::Arc;

/// Bearer token extractor that validates the JWT and provides claims.
pub struct AuthBearer<S: IdentityStore> {
    /// The verified claims from the JWT.
    pub claims: Claims,
    /// Marker for the store type.
    _marker: std::marker::PhantomData<S>,
}

impl<S: IdentityStore> AuthBearer<S> {
    /// Get the claims.
    pub fn claims(&self) -> &Claims {
        &self.claims
    }
}

#[async_trait]
impl<S: IdentityStore> FromRequestParts<Arc<AuthService<S>>> for AuthBearer<S> {
    type Rejection = AuthRejection;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AuthService<S>>,
    ) -> Result<Self, Self::Rejection> {
        // Get Authorization header
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .ok_or(AuthRejection(AuthError::Unauthorized))?
            .to_str()
            .map_err(|_| AuthRejection(AuthError::Unauthorized))?;

        // Parse Bearer token
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AuthRejection(AuthError::Unauthorized))?;

        // Verify token
        let claims = state.verify_token(token).await.map_err(AuthRejection)?;

        Ok(Self {
            claims,
            _marker: std::marker::PhantomData,
        })
    }
}

/// Rejection type for authentication failures.
pub struct AuthRejection(pub AuthError);

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.0.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let response = ErrorResponse::new(self.0.code(), self.0.to_string(), self.0.status_code());
        (status, Json(response)).into_response()
    }
}
