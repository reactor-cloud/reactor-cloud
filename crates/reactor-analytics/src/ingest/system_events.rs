//! System event canonicalizers.
//!
//! Converts incoming system events ($pageview, $identify, etc.) into stored
//! events with proper hot-column mapping.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::{ClientContext, IngestEvent, PageContext};
use crate::store::StoredEvent;

/// System event types.
pub mod event_types {
    /// Page view event.
    pub const PAGEVIEW: &str = "$pageview";
    /// Identify event (link anonymous to user).
    pub const IDENTIFY: &str = "$identify";
    /// Alias event (alias anonymous to user).
    pub const ALIAS: &str = "$alias";
    /// Session start event.
    pub const SESSION_START: &str = "$session_start";
    /// Session end event.
    pub const SESSION_END: &str = "$session_end";
    /// Autocapture event (click, submit).
    pub const AUTOCAPTURE: &str = "$autocapture";
    /// Error event.
    pub const ERROR: &str = "$error";
}

/// Canonicalize a $pageview event.
pub fn canonicalize_pageview(
    event: &IngestEvent,
    org_id: Uuid,
    project_id: Uuid,
    enrichment: &super::enrich::EnrichmentResult,
) -> StoredEvent {
    let now = Utc::now();
    let id = Uuid::now_v7();

    let (library_name, library_version) = extract_library(&event.context);

    StoredEvent {
        id,
        received_at: now,
        timestamp: event.timestamp.unwrap_or(now),
        org_id,
        project_id,
        event: event_types::PAGEVIEW.to_string(),
        anonymous_id: event.anonymous_id.clone().unwrap_or_default(),
        user_id: event.user_id.clone(),
        session_id: event.session_id.clone(),
        url: enrichment.url.clone().or_else(|| extract_url(&event.context)),
        path: enrichment.path.clone().or_else(|| extract_path(&event.context)),
        referrer_host: enrichment.referrer_host.clone(),
        utm_source: enrichment.utm_source.clone(),
        country: enrichment.country.clone(),
        device_type: enrichment.device_type.clone(),
        ingest_ip_h24: enrichment.ip_truncated.clone(),
        library_name,
        library_version,
        properties: event.properties.clone(),
        context: serde_json::to_value(&event.context).unwrap_or_default(),
    }
}

/// Canonicalize a $identify event.
pub fn canonicalize_identify(
    event: &IngestEvent,
    org_id: Uuid,
    project_id: Uuid,
    user_id: &str,
    enrichment: &super::enrich::EnrichmentResult,
) -> StoredEvent {
    let now = Utc::now();
    let id = Uuid::now_v7();

    let (library_name, library_version) = extract_library(&event.context);

    StoredEvent {
        id,
        received_at: now,
        timestamp: event.timestamp.unwrap_or(now),
        org_id,
        project_id,
        event: event_types::IDENTIFY.to_string(),
        anonymous_id: event.anonymous_id.clone().unwrap_or_default(),
        user_id: Some(user_id.to_string()),
        session_id: event.session_id.clone(),
        url: enrichment.url.clone(),
        path: enrichment.path.clone(),
        referrer_host: enrichment.referrer_host.clone(),
        utm_source: enrichment.utm_source.clone(),
        country: enrichment.country.clone(),
        device_type: enrichment.device_type.clone(),
        ingest_ip_h24: enrichment.ip_truncated.clone(),
        library_name,
        library_version,
        properties: event.properties.clone(),
        context: serde_json::to_value(&event.context).unwrap_or_default(),
    }
}

/// Canonicalize a $alias event.
pub fn canonicalize_alias(
    event: &IngestEvent,
    org_id: Uuid,
    project_id: Uuid,
    from_anonymous_id: &str,
    to_user_id: &str,
    enrichment: &super::enrich::EnrichmentResult,
) -> StoredEvent {
    let now = Utc::now();
    let id = Uuid::now_v7();

    let (library_name, library_version) = extract_library(&event.context);

    let mut props = event.properties.clone();
    if let Some(obj) = props.as_object_mut() {
        obj.insert("from_anonymous_id".to_string(), serde_json::json!(from_anonymous_id));
        obj.insert("to_user_id".to_string(), serde_json::json!(to_user_id));
    }

    StoredEvent {
        id,
        received_at: now,
        timestamp: event.timestamp.unwrap_or(now),
        org_id,
        project_id,
        event: event_types::ALIAS.to_string(),
        anonymous_id: from_anonymous_id.to_string(),
        user_id: Some(to_user_id.to_string()),
        session_id: event.session_id.clone(),
        url: enrichment.url.clone(),
        path: enrichment.path.clone(),
        referrer_host: None,
        utm_source: None,
        country: enrichment.country.clone(),
        device_type: enrichment.device_type.clone(),
        ingest_ip_h24: enrichment.ip_truncated.clone(),
        library_name,
        library_version,
        properties: props,
        context: serde_json::to_value(&event.context).unwrap_or_default(),
    }
}

/// Canonicalize a $session_start event.
pub fn canonicalize_session_start(
    event: &IngestEvent,
    org_id: Uuid,
    project_id: Uuid,
    enrichment: &super::enrich::EnrichmentResult,
) -> StoredEvent {
    let now = Utc::now();
    let id = Uuid::now_v7();

    let (library_name, library_version) = extract_library(&event.context);

    StoredEvent {
        id,
        received_at: now,
        timestamp: event.timestamp.unwrap_or(now),
        org_id,
        project_id,
        event: event_types::SESSION_START.to_string(),
        anonymous_id: event.anonymous_id.clone().unwrap_or_default(),
        user_id: event.user_id.clone(),
        session_id: event.session_id.clone(),
        url: enrichment.url.clone(),
        path: enrichment.path.clone(),
        referrer_host: enrichment.referrer_host.clone(),
        utm_source: enrichment.utm_source.clone(),
        country: enrichment.country.clone(),
        device_type: enrichment.device_type.clone(),
        ingest_ip_h24: enrichment.ip_truncated.clone(),
        library_name,
        library_version,
        properties: event.properties.clone(),
        context: serde_json::to_value(&event.context).unwrap_or_default(),
    }
}

/// Canonicalize a $session_end event.
pub fn canonicalize_session_end(
    event: &IngestEvent,
    org_id: Uuid,
    project_id: Uuid,
    enrichment: &super::enrich::EnrichmentResult,
) -> StoredEvent {
    let now = Utc::now();
    let id = Uuid::now_v7();

    let (library_name, library_version) = extract_library(&event.context);

    StoredEvent {
        id,
        received_at: now,
        timestamp: event.timestamp.unwrap_or(now),
        org_id,
        project_id,
        event: event_types::SESSION_END.to_string(),
        anonymous_id: event.anonymous_id.clone().unwrap_or_default(),
        user_id: event.user_id.clone(),
        session_id: event.session_id.clone(),
        url: enrichment.url.clone(),
        path: enrichment.path.clone(),
        referrer_host: None,
        utm_source: None,
        country: enrichment.country.clone(),
        device_type: enrichment.device_type.clone(),
        ingest_ip_h24: enrichment.ip_truncated.clone(),
        library_name,
        library_version,
        properties: event.properties.clone(),
        context: serde_json::to_value(&event.context).unwrap_or_default(),
    }
}

/// Canonicalize a $autocapture event.
pub fn canonicalize_autocapture(
    event: &IngestEvent,
    org_id: Uuid,
    project_id: Uuid,
    enrichment: &super::enrich::EnrichmentResult,
) -> StoredEvent {
    let now = Utc::now();
    let id = Uuid::now_v7();

    let (library_name, library_version) = extract_library(&event.context);

    StoredEvent {
        id,
        received_at: now,
        timestamp: event.timestamp.unwrap_or(now),
        org_id,
        project_id,
        event: event_types::AUTOCAPTURE.to_string(),
        anonymous_id: event.anonymous_id.clone().unwrap_or_default(),
        user_id: event.user_id.clone(),
        session_id: event.session_id.clone(),
        url: enrichment.url.clone(),
        path: enrichment.path.clone(),
        referrer_host: None,
        utm_source: None,
        country: enrichment.country.clone(),
        device_type: enrichment.device_type.clone(),
        ingest_ip_h24: enrichment.ip_truncated.clone(),
        library_name,
        library_version,
        properties: event.properties.clone(),
        context: serde_json::to_value(&event.context).unwrap_or_default(),
    }
}

/// Canonicalize a $error event.
pub fn canonicalize_error(
    event: &IngestEvent,
    org_id: Uuid,
    project_id: Uuid,
    enrichment: &super::enrich::EnrichmentResult,
) -> StoredEvent {
    let now = Utc::now();
    let id = Uuid::now_v7();

    let (library_name, library_version) = extract_library(&event.context);

    StoredEvent {
        id,
        received_at: now,
        timestamp: event.timestamp.unwrap_or(now),
        org_id,
        project_id,
        event: event_types::ERROR.to_string(),
        anonymous_id: event.anonymous_id.clone().unwrap_or_default(),
        user_id: event.user_id.clone(),
        session_id: event.session_id.clone(),
        url: enrichment.url.clone(),
        path: enrichment.path.clone(),
        referrer_host: None,
        utm_source: None,
        country: enrichment.country.clone(),
        device_type: enrichment.device_type.clone(),
        ingest_ip_h24: enrichment.ip_truncated.clone(),
        library_name,
        library_version,
        properties: event.properties.clone(),
        context: serde_json::to_value(&event.context).unwrap_or_default(),
    }
}

/// Extract library info from context.
fn extract_library(context: &ClientContext) -> (Option<String>, Option<String>) {
    context
        .library
        .as_ref()
        .map(|l| (Some(l.name.clone()), Some(l.version.clone())))
        .unwrap_or((None, None))
}

/// Extract URL from context.
fn extract_url(context: &ClientContext) -> Option<String> {
    context.page.as_ref().and_then(|p| p.url.clone())
}

/// Extract path from context.
fn extract_path(context: &ClientContext) -> Option<String> {
    context.page.as_ref().and_then(|p| p.path.clone())
}
