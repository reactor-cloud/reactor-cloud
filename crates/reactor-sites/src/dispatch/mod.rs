//! Request dispatch to static files and functions.

mod function_dispatch;
mod prerender;
pub mod static_dispatch;

pub use function_dispatch::FunctionsClient;
pub use static_dispatch::{StaticDispatcher, StorageClient};

use crate::bundle::CacheRules;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Route decision after matching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouteDecision {
    /// Serve a static file.
    StaticFile {
        /// Storage key for the file.
        storage_key: String,
        /// Cache rules.
        cache: CacheRules,
        /// Content type hint.
        content_type: Option<String>,
    },
    /// Invoke a function.
    Function {
        /// Function ID in reactor-functions.
        function_id: Uuid,
        /// Function name (for invocation via HTTP API).
        function_name: String,
        /// Sub-path to pass to the function.
        sub_path: String,
    },
    /// HTTP redirect.
    Redirect {
        /// Redirect location.
        location: String,
        /// HTTP status code (301, 302, 307, 308).
        status: u16,
        /// Whether this is permanent.
        permanent: bool,
    },
    /// Prerendered content with ISR.
    Prerender {
        /// Storage key for the prerendered HTML.
        storage_key: String,
        /// Revalidate interval.
        revalidate_after: Option<Duration>,
        /// Fallback if not in cache.
        fallback: Option<Box<RouteDecision>>,
    },
    /// No route matched.
    NotFound,
}
