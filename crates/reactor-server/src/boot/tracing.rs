//! Unified tracing initialization.
//!
//! Sets up tracing-subscriber with a filter and formatter based on configuration.

use crate::config::TracingConfig;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the tracing subscriber.
///
/// The filter can be overridden by the `RUST_LOG` environment variable.
/// Format can be "json" (structured) or "pretty" (human-readable).
pub fn init(config: &TracingConfig) {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.filter));

    let subscriber = tracing_subscriber::registry().with(filter);

    match config.fmt.as_str() {
        "pretty" => {
            subscriber
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
        _ => {
            // Default to JSON
            subscriber
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        }
    }

    tracing::debug!(filter = %config.filter, fmt = %config.fmt, "tracing initialized");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_config_defaults() {
        let config = TracingConfig::default();
        assert_eq!(config.filter, "info");
        assert_eq!(config.fmt, "json");
    }
}
