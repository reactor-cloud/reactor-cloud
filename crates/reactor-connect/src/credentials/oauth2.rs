//! OAuth2 PKCE flow implementation.

use crate::error::ConnectError;

/// Build OAuth2 authorization URL with PKCE.
pub fn build_authorize_url(
    authorize_url: &str,
    client_id: &str,
    redirect_uri: &str,
    scopes: &[String],
) -> Result<(String, String, String), ConnectError> {
    use oauth2::{AuthUrl, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope};

    let auth_url = AuthUrl::new(authorize_url.to_string())
        .map_err(|e| ConnectError::Internal(format!("Invalid authorize URL: {}", e)))?;

    let redirect = RedirectUrl::new(redirect_uri.to_string())
        .map_err(|e| ConnectError::Internal(format!("Invalid redirect URI: {}", e)))?;

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let client = oauth2::basic::BasicClient::new(
        ClientId::new(client_id.to_string()),
        None, // No client secret for PKCE public clients
        auth_url,
        None, // Token URL not needed for authorization URL
    )
    .set_redirect_uri(redirect);

    let mut auth_request = client.authorize_url(CsrfToken::new_random);
    auth_request = auth_request.set_pkce_challenge(pkce_challenge);

    for scope in scopes {
        auth_request = auth_request.add_scope(Scope::new(scope.clone()));
    }

    let (url, csrf_token) = auth_request.url();

    Ok((
        url.to_string(),
        csrf_token.secret().to_string(),
        pkce_verifier.secret().to_string(),
    ))
}

/// Exchange authorization code for tokens.
pub async fn exchange_code(
    token_url: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
    redirect_uri: &str,
    pkce_verifier: Option<&str>,
) -> Result<TokenResponse, ConnectError> {
    let client = reqwest::Client::new();
    
    let mut params = vec![
        ("grant_type", "authorization_code"),
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("code", code),
        ("redirect_uri", redirect_uri),
    ];

    if let Some(verifier) = pkce_verifier {
        params.push(("code_verifier", verifier));
    }

    let resp = client
        .post(token_url)
        .form(&params)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| ConnectError::OAuthCallbackFailed(e.to_string()))?;

    let token_resp: TokenResponse = resp
        .json()
        .await
        .map_err(|e| ConnectError::OAuthCallbackFailed(e.to_string()))?;

    Ok(token_resp)
}

/// Token response from OAuth2 provider.
#[derive(Debug, serde::Deserialize)]
pub struct TokenResponse {
    /// Access token.
    pub access_token: String,
    /// Token type.
    pub token_type: String,
    /// Expires in seconds.
    pub expires_in: Option<u64>,
    /// Refresh token.
    pub refresh_token: Option<String>,
    /// Scope.
    pub scope: Option<String>,
}
