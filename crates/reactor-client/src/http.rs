//! HTTP client with auth, retries, and request-id propagation.

use crate::error::{ClientError, ClientResult};
use crate::{ApiError, ApiSuccess};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;
use tracing::{debug, warn};
use url::Url;

/// Configuration for the Reactor client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Base URL of the Reactor server.
    pub endpoint: Url,
    /// Optional bearer token for authentication.
    pub token: Option<String>,
    /// Optional organization ID/slug to include in requests.
    pub org: Option<String>,
    /// Number of retries for retriable errors (default: 3).
    pub retries: u32,
    /// Request timeout (default: 30s).
    pub timeout: Duration,
    /// User-Agent string.
    pub user_agent: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            endpoint: Url::parse("http://127.0.0.1:8080").unwrap(),
            token: None,
            org: None,
            retries: 3,
            timeout: Duration::from_secs(30),
            user_agent: format!("reactor-client/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

impl ClientConfig {
    /// Create a new config with the given endpoint.
    pub fn new(endpoint: Url) -> Self {
        Self {
            endpoint,
            ..Default::default()
        }
    }

    /// Set the authentication token.
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Set the organization.
    pub fn with_org(mut self, org: impl Into<String>) -> Self {
        self.org = Some(org.into());
        self
    }

    /// Set the number of retries.
    pub fn with_retries(mut self, retries: u32) -> Self {
        self.retries = retries;
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

/// HTTP client for Reactor server APIs.
#[derive(Debug, Clone)]
pub struct Client {
    inner: reqwest::Client,
    config: ClientConfig,
}

impl Client {
    /// Create a new client with the given configuration.
    pub fn new(config: ClientConfig) -> ClientResult<Self> {
        let mut headers = HeaderMap::new();

        if let Some(ref token) = config.token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {}", token))
                    .map_err(|_| ClientError::Auth("invalid token format".into()))?,
            );
        }

        if let Some(ref org) = config.org {
            headers.insert(
                "X-Reactor-Org",
                HeaderValue::from_str(org)
                    .map_err(|_| ClientError::Validation("invalid org format".into()))?,
            );
        }

        let inner = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(config.timeout)
            .user_agent(&config.user_agent)
            .build()?;

        Ok(Self { inner, config })
    }

    /// Get the base endpoint URL.
    pub fn endpoint(&self) -> &Url {
        &self.config.endpoint
    }

    /// Build a URL for the given path.
    pub fn url(&self, path: &str) -> ClientResult<Url> {
        self.config.endpoint.join(path).map_err(Into::into)
    }

    /// Perform a GET request and deserialize the response.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> ClientResult<T> {
        self.request_with_retry::<T, ()>(reqwest::Method::GET, path, None)
            .await
    }

    /// Perform a GET request and return the raw response text.
    pub async fn get_text(&self, path: &str) -> ClientResult<String> {
        let url = self.url(path)?;
        let request_id = uuid::Uuid::now_v7().to_string();

        let response = self
            .inner
            .get(url)
            .header("X-Request-Id", &request_id)
            .send()
            .await?;

        let status = response.status().as_u16();
        if status >= 200 && status < 300 {
            response.text().await.map_err(Into::into)
        } else {
            let text = response.text().await.unwrap_or_default();
            Err(ClientError::Server {
                code: format!("HTTP_{}", status),
                message: text,
                hint: None,
                status,
            })
        }
    }

    /// Perform a POST request with a JSON body.
    pub async fn post<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> ClientResult<T> {
        self.request_with_retry(reqwest::Method::POST, path, Some(body))
            .await
    }

    /// Perform a PUT request with a JSON body.
    pub async fn put<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> ClientResult<T> {
        self.request_with_retry(reqwest::Method::PUT, path, Some(body))
            .await
    }

    /// Perform a DELETE request.
    pub async fn delete<T: DeserializeOwned>(&self, path: &str) -> ClientResult<T> {
        self.request_with_retry::<T, ()>(reqwest::Method::DELETE, path, None)
            .await
    }

    /// Perform a PATCH request with a JSON body.
    pub async fn patch<T: DeserializeOwned, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> ClientResult<T> {
        self.request_with_retry(reqwest::Method::PATCH, path, Some(body))
            .await
    }

    /// Perform a POST request with an empty body.
    pub async fn post_empty<T: DeserializeOwned>(&self, path: &str) -> ClientResult<T> {
        self.request_with_retry::<T, ()>(reqwest::Method::POST, path, None)
            .await
    }

    /// Upload a multipart form (for bundle uploads).
    pub async fn post_multipart<T: DeserializeOwned>(
        &self,
        path: &str,
        form: reqwest::multipart::Form,
    ) -> ClientResult<T> {
        let url = self.url(path)?;
        let request_id = uuid::Uuid::now_v7().to_string();

        let response = self
            .inner
            .post(url)
            .header("X-Request-Id", &request_id)
            .multipart(form)
            .send()
            .await?;

        self.handle_response(response).await
    }

    /// Perform a POST request and return the raw response for SSE streaming.
    ///
    /// Used for streaming endpoints like AI chat completions.
    pub async fn post_sse<B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> ClientResult<reqwest::Response> {
        let url = self.url(path)?;
        let request_id = uuid::Uuid::now_v7().to_string();

        let response = self
            .inner
            .post(url)
            .header("X-Request-Id", &request_id)
            .header(CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .await?;

        let status = response.status().as_u16();
        if status >= 200 && status < 300 {
            Ok(response)
        } else {
            let text = response.text().await?;

            // Try to parse as ApiError envelope
            if let Ok(api_error) = serde_json::from_str::<crate::ApiError>(&text) {
                return Err(ClientError::from_api_error(api_error.error, status));
            }

            Err(ClientError::Server {
                code: format!("HTTP_{}", status),
                message: text,
                hint: None,
                status,
            })
        }
    }

    /// Perform a request with automatic retries for retriable errors.
    async fn request_with_retry<T: DeserializeOwned, B: Serialize>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
    ) -> ClientResult<T> {
        let url = self.url(path)?;
        let is_idempotent = matches!(
            method,
            reqwest::Method::GET | reqwest::Method::PUT | reqwest::Method::DELETE
        );
        let max_retries = if is_idempotent { self.config.retries } else { 0 };

        let mut last_error = None;
        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(100 * 2u64.pow(attempt - 1));
                debug!(attempt, delay_ms = delay.as_millis(), "retrying request");
                tokio::time::sleep(delay).await;
            }

            let request_id = uuid::Uuid::now_v7().to_string();
            let mut request = self
                .inner
                .request(method.clone(), url.clone())
                .header("X-Request-Id", &request_id);

            if let Some(b) = body {
                request = request.header(CONTENT_TYPE, "application/json").json(b);
            }

            match request.send().await {
                Ok(response) => match self.handle_response(response).await {
                    Ok(data) => return Ok(data),
                    Err(e) if e.is_retriable() && attempt < max_retries => {
                        warn!(attempt, error = %e, "retriable error");
                        last_error = Some(e);
                    }
                    Err(e) => return Err(e),
                },
                Err(e) if e.is_connect() || e.is_timeout() => {
                    let err = ClientError::Network(e);
                    if attempt < max_retries {
                        warn!(attempt, error = %err, "network error, will retry");
                        last_error = Some(err);
                    } else {
                        return Err(err);
                    }
                }
                Err(e) => return Err(ClientError::Network(e)),
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ClientError::InvalidResponse("unknown error after retries".into())
        }))
    }

    /// Handle a response, deserializing success or extracting error details.
    async fn handle_response<T: DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> ClientResult<T> {
        let status = response.status().as_u16();

        if status >= 200 && status < 300 {
            let text = response.text().await?;
            if text.is_empty() {
                return Err(ClientError::InvalidResponse("empty response body".into()));
            }

            // Try to parse as ApiSuccess envelope first
            if let Ok(envelope) = serde_json::from_str::<ApiSuccess<T>>(&text) {
                return Ok(envelope.data);
            }

            // Fall back to direct deserialization
            serde_json::from_str(&text).map_err(|e| {
                ClientError::InvalidResponse(format!("failed to parse response: {}", e))
            })
        } else {
            let text = response.text().await?;

            // Try to parse as ApiError envelope
            if let Ok(api_error) = serde_json::from_str::<ApiError>(&text) {
                return Err(ClientError::from_api_error(api_error.error, status));
            }

            // Fall back to generic error
            Err(ClientError::Server {
                code: format!("HTTP_{}", status),
                message: text,
                hint: None,
                status,
            })
        }
    }
}
