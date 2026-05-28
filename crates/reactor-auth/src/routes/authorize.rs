//! OAuth 2.0 Authorization endpoint with PKCE support.

use crate::error::AppError;
use crate::service::AuthService;
use crate::store::IdentityStore;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
};
use reactor_core::auth::AuthError;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use utoipa::{IntoParams, ToSchema};

/// Authorization request query parameters (OAuth 2.0 + PKCE).
#[derive(Debug, Deserialize, IntoParams)]
pub struct AuthorizeQuery {
    /// The client identifier.
    pub client_id: String,
    /// Must be "code" for authorization code flow.
    pub response_type: String,
    /// URI to redirect after authorization.
    pub redirect_uri: String,
    /// Requested scopes (space-separated).
    #[serde(default)]
    pub scope: Option<String>,
    /// CSRF protection state parameter.
    pub state: Option<String>,
    /// PKCE code challenge (base64url-encoded).
    pub code_challenge: String,
    /// PKCE code challenge method (must be S256).
    #[serde(default = "default_code_challenge_method")]
    pub code_challenge_method: String,
    /// Optional nonce for OIDC.
    pub nonce: Option<String>,
}

fn default_code_challenge_method() -> String {
    "S256".to_string()
}

/// Authorization response (redirect with code).
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthorizeResponse {
    /// The authorization code (short-lived).
    pub code: String,
    /// The state parameter (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

/// Error response for authorization failures.
#[derive(Debug, Serialize, ToSchema)]
pub struct AuthorizeError {
    /// Error code (RFC 6749).
    pub error: String,
    /// Human-readable description.
    pub error_description: String,
    /// State parameter (echoed back).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
}

/// Known client IDs for the authorization server.
const KNOWN_CLIENTS: &[&str] = &["reactor-cli", "reactor-web"];

/// GET /auth/v1/authorize
///
/// Initiates the OAuth 2.0 authorization code flow with PKCE.
/// If user is not logged in, redirects to login page.
/// If logged in, shows consent screen and issues authorization code.
#[utoipa::path(
    get,
    path = "/auth/v1/authorize",
    tag = "auth",
    params(AuthorizeQuery),
    responses(
        (status = 302, description = "Redirect to callback with authorization code"),
        (status = 400, description = "Invalid request parameters"),
        (status = 401, description = "User not authenticated"),
    )
)]
pub async fn authorize<S: IdentityStore>(
    State(_service): State<Arc<AuthService<S>>>,
    Query(query): Query<AuthorizeQuery>,
) -> Result<Response, AppError> {
    // Validate response_type
    if query.response_type != "code" {
        return Ok(authorize_error_redirect(
            &query.redirect_uri,
            "unsupported_response_type",
            "Only 'code' response_type is supported",
            query.state.as_deref(),
        ));
    }

    // Validate client_id
    if !KNOWN_CLIENTS.contains(&query.client_id.as_str()) {
        return Ok(authorize_error_redirect(
            &query.redirect_uri,
            "unauthorized_client",
            "Unknown client_id",
            query.state.as_deref(),
        ));
    }

    // Validate code_challenge_method
    if query.code_challenge_method != "S256" {
        return Ok(authorize_error_redirect(
            &query.redirect_uri,
            "invalid_request",
            "Only S256 code_challenge_method is supported",
            query.state.as_deref(),
        ));
    }

    // Validate code_challenge (must be base64url, 43 chars for SHA256)
    if query.code_challenge.len() < 43 {
        return Ok(authorize_error_redirect(
            &query.redirect_uri,
            "invalid_request",
            "Invalid code_challenge",
            query.state.as_deref(),
        ));
    }

    // Parse scopes
    let scopes: Vec<String> = query
        .scope
        .as_ref()
        .map(|s| s.split_whitespace().map(String::from).collect())
        .unwrap_or_else(|| vec!["openid".to_string()]);

    // For now, we render a simple login/consent page.
    // In production this would integrate with the session cookie or redirect to login.
    // The actual authorization code issuance happens after user authenticates.
    
    // Return a simple HTML form that will POST credentials and issue the code.
    // This is a simplified flow - production would use proper session management.
    let html = render_authorize_form(
        &query.client_id,
        &query.redirect_uri,
        &query.code_challenge,
        &query.code_challenge_method,
        query.state.as_deref(),
        query.nonce.as_deref(),
        &scopes,
    );

    Ok(Html(html).into_response())
}

/// POST /auth/v1/authorize
///
/// Handles the authorization form submission.
/// Authenticates user, issues authorization code, and redirects.
#[derive(Debug, Deserialize)]
pub struct AuthorizeFormRequest {
    /// User's email.
    pub email: String,
    /// User's password.
    pub password: String,
    /// Client ID.
    pub client_id: String,
    /// Redirect URI.
    pub redirect_uri: String,
    /// PKCE code challenge.
    pub code_challenge: String,
    /// PKCE code challenge method.
    pub code_challenge_method: String,
    /// State parameter.
    pub state: Option<String>,
    /// Nonce parameter.
    pub nonce: Option<String>,
    /// Requested scopes (comma-separated).
    pub scopes: String,
}

/// POST /auth/v1/authorize
pub async fn authorize_submit<S: IdentityStore>(
    State(service): State<Arc<AuthService<S>>>,
    axum::Form(form): axum::Form<AuthorizeFormRequest>,
) -> Result<Response, AppError> {
    // Authenticate user
    let auth_result = service
        .password_grant(&form.email, &form.password, None, None)
        .await;

    match auth_result {
        Ok(auth_response) => {
            // Parse scopes
            let scopes: Vec<String> = form
                .scopes
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            // Create authorization code
            let code = service
                .create_authorization_code(
                    auth_response.user.id,
                    &form.client_id,
                    &form.redirect_uri,
                    scopes,
                    &form.code_challenge,
                    &form.code_challenge_method,
                    form.nonce.as_deref(),
                    form.state.as_deref(),
                    Some(auth_response.session.id),
                )
                .await?;

            // Build redirect URL with code
            let mut redirect_url = form.redirect_uri.clone();
            let separator = if redirect_url.contains('?') { '&' } else { '?' };
            redirect_url.push(separator);
            redirect_url.push_str("code=");
            redirect_url.push_str(&code);
            if let Some(state) = form.state.as_ref() {
                redirect_url.push_str("&state=");
                redirect_url.push_str(state);
            }

            Ok(Redirect::to(&redirect_url).into_response())
        }
        Err(AuthError::InvalidCredentials) => {
            // Re-render form with error
            let scopes: Vec<String> = form
                .scopes
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let html = render_authorize_form_with_error(
                &form.client_id,
                &form.redirect_uri,
                &form.code_challenge,
                &form.code_challenge_method,
                form.state.as_deref(),
                form.nonce.as_deref(),
                &scopes,
                "Invalid email or password",
            );
            Ok((StatusCode::UNAUTHORIZED, Html(html)).into_response())
        }
        Err(e) => Err(AppError::Auth(e)),
    }
}

/// Build an error redirect URL.
fn authorize_error_redirect(
    redirect_uri: &str,
    error: &str,
    description: &str,
    state: Option<&str>,
) -> Response {
    let mut url = redirect_uri.to_string();
    let separator = if url.contains('?') { '&' } else { '?' };
    url.push(separator);
    url.push_str("error=");
    url.push_str(&url_encode(error));
    url.push_str("&error_description=");
    url.push_str(&url_encode(description));
    if let Some(s) = state {
        url.push_str("&state=");
        url.push_str(&url_encode(s));
    }
    Redirect::to(&url).into_response()
}

/// URL-encode a string for query parameters.
fn url_encode(s: &str) -> String {
    // Simple percent-encoding for query strings
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                String::from(b as char)
            }
            _ => format!("%{:02X}", b),
        })
        .collect()
}

/// Render the authorization form HTML.
fn render_authorize_form(
    client_id: &str,
    redirect_uri: &str,
    code_challenge: &str,
    code_challenge_method: &str,
    state: Option<&str>,
    nonce: Option<&str>,
    scopes: &[String],
) -> String {
    render_authorize_form_with_error(
        client_id,
        redirect_uri,
        code_challenge,
        code_challenge_method,
        state,
        nonce,
        scopes,
        "",
    )
}

/// Render the authorization form HTML with optional error message.
fn render_authorize_form_with_error(
    client_id: &str,
    redirect_uri: &str,
    code_challenge: &str,
    code_challenge_method: &str,
    state: Option<&str>,
    nonce: Option<&str>,
    scopes: &[String],
    error: &str,
) -> String {
    let scopes_str = scopes.join(",");
    let state_input = state
        .map(|s| format!(r#"<input type="hidden" name="state" value="{}" />"#, html_escape(s)))
        .unwrap_or_default();
    let nonce_input = nonce
        .map(|n| format!(r#"<input type="hidden" name="nonce" value="{}" />"#, html_escape(n)))
        .unwrap_or_default();
    let error_div = if error.is_empty() {
        String::new()
    } else {
        format!(
            r#"<div style="color: #dc2626; background: #fef2f2; padding: 12px; border-radius: 6px; margin-bottom: 16px;">{}</div>"#,
            html_escape(error)
        )
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sign in to Reactor</title>
    <style>
        * {{ margin: 0; padding: 0; box-sizing: border-box; }}
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; background: #0a0a0a; color: #fafafa; min-height: 100vh; display: flex; align-items: center; justify-content: center; }}
        .container {{ width: 100%; max-width: 400px; padding: 24px; }}
        .card {{ background: #171717; border: 1px solid #262626; border-radius: 12px; padding: 32px; }}
        h1 {{ font-size: 24px; font-weight: 600; margin-bottom: 8px; }}
        .subtitle {{ color: #a3a3a3; margin-bottom: 24px; }}
        .client-info {{ background: #262626; border-radius: 8px; padding: 12px; margin-bottom: 24px; }}
        .client-name {{ font-weight: 500; }}
        .scopes {{ font-size: 14px; color: #a3a3a3; margin-top: 4px; }}
        form {{ display: flex; flex-direction: column; gap: 16px; }}
        label {{ font-size: 14px; font-weight: 500; color: #d4d4d4; }}
        input[type="email"], input[type="password"] {{ width: 100%; padding: 12px; background: #262626; border: 1px solid #404040; border-radius: 6px; color: #fafafa; font-size: 14px; }}
        input:focus {{ outline: none; border-color: #3b82f6; }}
        button {{ padding: 12px 24px; background: #3b82f6; color: white; border: none; border-radius: 6px; font-size: 14px; font-weight: 500; cursor: pointer; transition: background 0.2s; }}
        button:hover {{ background: #2563eb; }}
    </style>
</head>
<body>
    <div class="container">
        <div class="card">
            <h1>Sign in to Reactor</h1>
            <p class="subtitle">to continue to {client_id}</p>
            {error_div}
            <div class="client-info">
                <div class="client-name">{client_id}</div>
                <div class="scopes">Requesting: {scopes_display}</div>
            </div>
            <form method="POST" action="/auth/v1/authorize">
                <input type="hidden" name="client_id" value="{client_id}" />
                <input type="hidden" name="redirect_uri" value="{redirect_uri}" />
                <input type="hidden" name="code_challenge" value="{code_challenge}" />
                <input type="hidden" name="code_challenge_method" value="{code_challenge_method}" />
                <input type="hidden" name="scopes" value="{scopes_str}" />
                {state_input}
                {nonce_input}
                <div>
                    <label for="email">Email</label>
                    <input type="email" id="email" name="email" required autofocus />
                </div>
                <div>
                    <label for="password">Password</label>
                    <input type="password" id="password" name="password" required />
                </div>
                <button type="submit">Sign in</button>
            </form>
        </div>
    </div>
</body>
</html>"#,
        client_id = html_escape(client_id),
        error_div = error_div,
        scopes_display = html_escape(&scopes.join(", ")),
        redirect_uri = html_escape(redirect_uri),
        code_challenge = html_escape(code_challenge),
        code_challenge_method = html_escape(code_challenge_method),
        scopes_str = html_escape(&scopes_str),
        state_input = state_input,
        nonce_input = nonce_input,
    )
}

/// Simple HTML escaping.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
