//! Analytics client for use in function/job/site runtimes.
//!
//! Provides an in-process client backed by a tokio mpsc channel for direct
//! delivery to the batcher, with HTTP fallback for lambda/isolated runtimes.

use crate::error::AnalyticsError;
use crate::ingest::BatchItem;
use crate::store::StoredEvent;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::mpsc;
use uuid::Uuid;

/// Event to be tracked.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackEvent {
    /// Event name.
    pub event: String,
    /// Event properties.
    #[serde(default)]
    pub properties: HashMap<String, Value>,
    /// User ID (if identified).
    pub user_id: Option<String>,
    /// Anonymous ID.
    pub anonymous_id: Option<String>,
    /// Session ID.
    pub session_id: Option<String>,
    /// Timestamp (ISO 8601). Auto-filled if omitted.
    pub timestamp: Option<String>,
}

impl TrackEvent {
    /// Create a new track event.
    pub fn new(event: impl Into<String>) -> Self {
        Self {
            event: event.into(),
            properties: HashMap::new(),
            user_id: None,
            anonymous_id: None,
            session_id: None,
            timestamp: None,
        }
    }

    /// Set a property.
    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }

    /// Set properties from a map.
    pub fn with_properties(mut self, props: HashMap<String, Value>) -> Self {
        self.properties.extend(props);
        self
    }

    /// Set the user ID.
    pub fn with_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set the anonymous ID.
    pub fn with_anonymous_id(mut self, anon_id: impl Into<String>) -> Self {
        self.anonymous_id = Some(anon_id.into());
        self
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }
}

/// Analytics client configuration.
#[derive(Debug, Clone)]
pub struct AnalyticsClientConfig {
    /// Organization ID.
    pub org_id: Uuid,
    /// Project ID.
    pub project_id: Uuid,
    /// Default anonymous ID (for server-side tracking).
    pub default_anonymous_id: Option<String>,
    /// Default user ID (for authenticated requests).
    pub default_user_id: Option<String>,
    /// HTTP endpoint for fallback mode.
    pub http_endpoint: Option<String>,
    /// Project key for HTTP authentication.
    pub project_key: Option<String>,
}

/// Analytics client for runtime injection.
///
/// Uses in-process channel when available (monolith mode), falls back to HTTP
/// for isolated runtimes (lambda, edge).
#[derive(Clone)]
pub struct AnalyticsClient {
    config: AnalyticsClientConfig,
    /// In-process sender (None in HTTP-only mode).
    sender: Option<mpsc::Sender<BatchItem>>,
    /// HTTP client for fallback mode.
    http_client: Option<reqwest::Client>,
}

impl std::fmt::Debug for AnalyticsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AnalyticsClient")
            .field("org_id", &self.config.org_id)
            .field("project_id", &self.config.project_id)
            .field("mode", &if self.sender.is_some() { "in-process" } else { "http" })
            .finish()
    }
}

impl AnalyticsClient {
    /// Create a new in-process analytics client.
    ///
    /// Uses the batcher's channel directly for zero-copy event delivery.
    pub fn new_in_process(
        config: AnalyticsClientConfig,
        sender: mpsc::Sender<BatchItem>,
    ) -> Self {
        Self {
            config,
            sender: Some(sender),
            http_client: None,
        }
    }

    /// Create an HTTP-only analytics client.
    ///
    /// Used for lambda/isolated runtimes that can't share the batcher channel.
    pub fn new_http(config: AnalyticsClientConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            config,
            sender: None,
            http_client: Some(http_client),
        }
    }

    /// Check if running in in-process mode.
    pub fn is_in_process(&self) -> bool {
        self.sender.is_some()
    }

    /// Track an event.
    pub async fn track(&self, event: TrackEvent) -> Result<(), AnalyticsError> {
        if self.sender.is_some() {
            self.track_in_process(event).await
        } else {
            self.track_http(event).await
        }
    }

    /// Track an event with just name and properties (convenience method).
    pub async fn track_simple(
        &self,
        event: impl Into<String>,
        properties: HashMap<String, Value>,
    ) -> Result<(), AnalyticsError> {
        let mut track_event = TrackEvent::new(event);
        track_event.properties = properties;
        
        // Apply defaults
        if track_event.anonymous_id.is_none() {
            track_event.anonymous_id = self.config.default_anonymous_id.clone();
        }
        if track_event.user_id.is_none() {
            track_event.user_id = self.config.default_user_id.clone();
        }

        self.track(track_event).await
    }

    /// Identify a user.
    pub async fn identify(
        &self,
        user_id: impl Into<String>,
        traits: HashMap<String, Value>,
    ) -> Result<(), AnalyticsError> {
        let event = TrackEvent::new("$identify")
            .with_user_id(user_id)
            .with_properties(traits);
        self.track(event).await
    }

    /// Track a pageview (for Sites runtime).
    pub async fn page(
        &self,
        path: impl Into<String>,
        properties: HashMap<String, Value>,
    ) -> Result<(), AnalyticsError> {
        let mut event = TrackEvent::new("$pageview")
            .with_property("path", path.into())
            .with_properties(properties);
        
        if event.anonymous_id.is_none() {
            event.anonymous_id = self.config.default_anonymous_id.clone();
        }

        self.track(event).await
    }

    // --- Internal methods ---

    async fn track_in_process(&self, event: TrackEvent) -> Result<(), AnalyticsError> {
        let sender = self.sender.as_ref().unwrap();
        let now = Utc::now();

        // Parse timestamp or use current time
        let timestamp = event.timestamp
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(now);

        // Create StoredEvent directly
        let stored_event = StoredEvent {
            id: Uuid::now_v7(),
            received_at: now,
            timestamp,
            org_id: self.config.org_id,
            project_id: self.config.project_id,
            event: event.event,
            anonymous_id: event.anonymous_id
                .or_else(|| self.config.default_anonymous_id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            user_id: event.user_id.or_else(|| self.config.default_user_id.clone()),
            session_id: event.session_id,
            url: None,
            path: None,
            referrer_host: None,
            utm_source: None,
            country: None,
            device_type: None,
            ingest_ip_h24: None,
            library_name: Some("reactor-analytics-runtime".to_string()),
            library_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            properties: serde_json::to_value(event.properties).unwrap_or_default(),
            context: serde_json::json!({}),
        };

        // Create batch item
        let batch_item = BatchItem { event: stored_event };

        // Send to batcher (non-blocking try_send to avoid blocking the runtime)
        match sender.try_send(batch_item) {
            Ok(()) => Ok(()),
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Channel full - event dropped (acceptable under backpressure)
                tracing::warn!(
                    project_id = %self.config.project_id,
                    "Analytics channel full, event dropped"
                );
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                Err(AnalyticsError::Internal("Analytics batcher channel closed".into()))
            }
        }
    }

    async fn track_http(&self, event: TrackEvent) -> Result<(), AnalyticsError> {
        let client = self.http_client.as_ref()
            .ok_or_else(|| AnalyticsError::Internal("HTTP client not configured".into()))?;
        
        let endpoint = self.config.http_endpoint.as_ref()
            .ok_or_else(|| AnalyticsError::Internal("HTTP endpoint not configured".into()))?;
        
        let project_key = self.config.project_key.as_ref()
            .ok_or_else(|| AnalyticsError::Internal("Project key not configured for HTTP mode".into()))?;

        // Apply defaults
        let anonymous_id = event.anonymous_id
            .or_else(|| self.config.default_anonymous_id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let user_id = event.user_id.or_else(|| self.config.default_user_id.clone());
        let timestamp = event.timestamp.unwrap_or_else(|| Utc::now().to_rfc3339());

        // Build request body
        let body = serde_json::json!({
            "events": [{
                "event": event.event,
                "anonymous_id": anonymous_id,
                "user_id": user_id,
                "session_id": event.session_id,
                "timestamp": timestamp,
                "properties": event.properties,
                "context": {
                    "library": {
                        "name": "reactor-analytics-runtime",
                        "version": env!("CARGO_PKG_VERSION"),
                    }
                }
            }]
        });

        // Send request
        let response = client
            .post(format!("{}/batch", endpoint))
            .header("Content-Type", "application/json")
            .header("X-Reactor-Project-Key", project_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| AnalyticsError::Internal(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AnalyticsError::Internal(format!(
                "HTTP request failed with status {}: {}",
                status, body
            )));
        }

        Ok(())
    }
}

/// Builder for creating an AnalyticsClient in the runtime context.
#[derive(Debug, Default)]
pub struct AnalyticsClientBuilder {
    org_id: Option<Uuid>,
    project_id: Option<Uuid>,
    default_anonymous_id: Option<String>,
    default_user_id: Option<String>,
    http_endpoint: Option<String>,
    project_key: Option<String>,
    sender: Option<mpsc::Sender<BatchItem>>,
}

impl AnalyticsClientBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the organization ID.
    pub fn org_id(mut self, org_id: Uuid) -> Self {
        self.org_id = Some(org_id);
        self
    }

    /// Set the project ID.
    pub fn project_id(mut self, project_id: Uuid) -> Self {
        self.project_id = Some(project_id);
        self
    }

    /// Set the default anonymous ID.
    pub fn default_anonymous_id(mut self, anon_id: impl Into<String>) -> Self {
        self.default_anonymous_id = Some(anon_id.into());
        self
    }

    /// Set the default user ID.
    pub fn default_user_id(mut self, user_id: impl Into<String>) -> Self {
        self.default_user_id = Some(user_id.into());
        self
    }

    /// Set up in-process mode with batcher channel.
    pub fn in_process(mut self, sender: mpsc::Sender<BatchItem>) -> Self {
        self.sender = Some(sender);
        self
    }

    /// Set up HTTP fallback mode.
    pub fn http_fallback(
        mut self,
        endpoint: impl Into<String>,
        project_key: impl Into<String>,
    ) -> Self {
        self.http_endpoint = Some(endpoint.into());
        self.project_key = Some(project_key.into());
        self
    }

    /// Build the analytics client.
    pub fn build(self) -> Result<AnalyticsClient, AnalyticsError> {
        let org_id = self.org_id
            .ok_or_else(|| AnalyticsError::Internal("org_id is required".into()))?;
        let project_id = self.project_id
            .ok_or_else(|| AnalyticsError::Internal("project_id is required".into()))?;

        let config = AnalyticsClientConfig {
            org_id,
            project_id,
            default_anonymous_id: self.default_anonymous_id,
            default_user_id: self.default_user_id,
            http_endpoint: self.http_endpoint.clone(),
            project_key: self.project_key.clone(),
        };

        if let Some(sender) = self.sender {
            Ok(AnalyticsClient::new_in_process(config, sender))
        } else if self.http_endpoint.is_some() {
            Ok(AnalyticsClient::new_http(config))
        } else {
            Err(AnalyticsError::Internal(
                "Either in-process sender or HTTP endpoint must be configured".into()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_event_builder() {
        let event = TrackEvent::new("button_clicked")
            .with_property("button_id", "signup")
            .with_user_id("user_123")
            .with_anonymous_id("anon_456");

        assert_eq!(event.event, "button_clicked");
        assert_eq!(event.properties.get("button_id").unwrap(), "signup");
        assert_eq!(event.user_id, Some("user_123".to_string()));
        assert_eq!(event.anonymous_id, Some("anon_456".to_string()));
    }

    #[tokio::test]
    async fn test_in_process_client() {
        let (tx, mut rx) = mpsc::channel(100);
        
        let client = AnalyticsClientBuilder::new()
            .org_id(Uuid::new_v4())
            .project_id(Uuid::new_v4())
            .default_anonymous_id("test_anon")
            .in_process(tx)
            .build()
            .unwrap();

        assert!(client.is_in_process());

        // Track an event
        let event = TrackEvent::new("test_event")
            .with_property("key", "value");
        client.track(event).await.unwrap();

        // Verify it was sent
        let batch_item = rx.recv().await.unwrap();
        assert_eq!(batch_item.event.event, "test_event");
    }
}
