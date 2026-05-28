//! Usage event emission for reactor-analytics.
//!
//! This module provides the bridge between AI usage events and the analytics
//! system. Usage events are emitted as `ai.usage` track events with token
//! counts and model information as properties.

use crate::ext::UsageEvent;
use serde_json::json;
use std::collections::HashMap;

/// Usage store trait for emitting usage events.
///
/// The default implementation logs events. In a full deployment,
/// this would emit to reactor-analytics via its ingest client.
pub trait UsageStore: Send + Sync {
    /// Record a usage event.
    fn record(&self, event: &UsageEvent);
}

/// Logging implementation of UsageStore.
#[derive(Debug, Clone, Default)]
pub struct LoggingUsageStore;

impl LoggingUsageStore {
    /// Create a new logging usage store.
    pub fn new() -> Self {
        Self
    }
}

impl UsageStore for LoggingUsageStore {
    fn record(&self, event: &UsageEvent) {
        tracing::info!(
            model_id = %event.model_id,
            user_id = ?event.user_id,
            tokens_in = event.tokens_in,
            tokens_out = event.tokens_out,
            "ai.usage"
        );
    }
}

/// Create analytics properties from an AI usage event.
///
/// This converts the usage event into a property map suitable for
/// the analytics TrackEvent format.
pub fn usage_event_to_properties(event: &UsageEvent) -> HashMap<String, serde_json::Value> {
    let mut props = HashMap::new();
    props.insert("model_id".to_string(), json!(event.model_id));
    props.insert("tokens_in".to_string(), json!(event.tokens_in));
    props.insert("tokens_out".to_string(), json!(event.tokens_out));
    props.insert("tokens_total".to_string(), json!(event.tokens_in + event.tokens_out));
    if let Some(ref user_id) = event.user_id {
        props.insert("metering_user_id".to_string(), json!(user_id));
    }
    props
}

/// Event name used for AI usage events in analytics.
pub const AI_USAGE_EVENT: &str = "ai.usage";
