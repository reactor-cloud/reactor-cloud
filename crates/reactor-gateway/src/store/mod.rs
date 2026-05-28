//! Storage backends for the routing table.

#[cfg(feature = "postgres")]
pub mod pg;

#[cfg(feature = "postgres")]
pub use pg::{spawn_notification_forwarder, PgRoutingStore, PgNotificationListener};
