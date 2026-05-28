//! Event ingestion pipeline.

pub mod batch;
pub mod enrich;
pub mod system_events;
pub mod validate;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub use batch::{BatchItem, Batcher, BatcherConfig, create_batcher_channel, to_stored_event};
pub use enrich::{EnrichmentResult, Enricher};

/// Incoming event from clients.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IngestEvent {
    /// Event name. Reserved names start with `$`.
    pub event: String,

    /// Client-provided anonymous ID.
    #[serde(default)]
    pub anonymous_id: Option<String>,

    /// User ID (set on authenticated ingestion or via $identify).
    #[serde(default)]
    pub user_id: Option<String>,

    /// Session ID (client rotates every 30min of inactivity).
    #[serde(default)]
    pub session_id: Option<String>,

    /// Client-provided event timestamp.
    #[serde(default)]
    pub timestamp: Option<DateTime<Utc>>,

    /// User-defined properties.
    #[serde(default)]
    pub properties: serde_json::Value,

    /// Client-provided context.
    #[serde(default)]
    pub context: ClientContext,
}

/// Client context.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ClientContext {
    /// Page context.
    #[serde(default)]
    pub page: Option<PageContext>,

    /// Screen context.
    #[serde(default)]
    pub screen: Option<ScreenContext>,

    /// Locale (BCP-47).
    #[serde(default)]
    pub locale: Option<String>,

    /// Timezone (IANA).
    #[serde(default)]
    pub timezone: Option<String>,

    /// Library info.
    #[serde(default)]
    pub library: Option<LibraryContext>,

    /// UTM parameters.
    #[serde(default)]
    pub utm: Option<UtmContext>,
}

/// Page context.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PageContext {
    /// Full URL.
    #[serde(default)]
    pub url: Option<String>,

    /// Path portion.
    #[serde(default)]
    pub path: Option<String>,

    /// Page title.
    #[serde(default)]
    pub title: Option<String>,

    /// Referrer URL.
    #[serde(default)]
    pub referrer: Option<String>,

    /// Search query string.
    #[serde(default)]
    pub search: Option<String>,

    /// Hash fragment.
    #[serde(default)]
    pub hash: Option<String>,
}

/// Screen context.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ScreenContext {
    /// Screen width.
    #[serde(default)]
    pub width: Option<u32>,

    /// Screen height.
    #[serde(default)]
    pub height: Option<u32>,

    /// Pixel density.
    #[serde(default)]
    pub density: Option<f32>,
}

/// Library context.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LibraryContext {
    /// Library name (e.g., "@reactor/analytics").
    pub name: String,

    /// Library version.
    pub version: String,
}

/// UTM context.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UtmContext {
    /// UTM source.
    #[serde(default)]
    pub source: Option<String>,

    /// UTM medium.
    #[serde(default)]
    pub medium: Option<String>,

    /// UTM campaign.
    #[serde(default)]
    pub campaign: Option<String>,

    /// UTM term.
    #[serde(default)]
    pub term: Option<String>,

    /// UTM content.
    #[serde(default)]
    pub content: Option<String>,
}

impl IngestEvent {
    /// Generate a new event ID.
    pub fn generate_id() -> Uuid {
        Uuid::now_v7()
    }
}
