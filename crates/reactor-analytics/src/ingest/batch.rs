//! Background event batcher.
//!
//! Receives events from the ingestion handler via a channel, buffers them,
//! and writes to the database in batches for throughput.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::mpsc;
use tokio::time::interval;

use super::IngestEvent;
use crate::config::AnalyticsConfig;
use crate::store::{AnalyticsStore, StoredEvent};

/// Batcher configuration.
#[derive(Debug, Clone)]
pub struct BatcherConfig {
    /// Flush interval in milliseconds.
    pub flush_interval_ms: u64,
    /// Maximum events per batch.
    pub max_batch_size: usize,
}

impl From<&AnalyticsConfig> for BatcherConfig {
    fn from(config: &AnalyticsConfig) -> Self {
        Self {
            flush_interval_ms: config.batch_interval_ms,
            max_batch_size: config.batch_max_rows,
        }
    }
}

/// Background batcher for event ingestion.
pub struct Batcher<S: AnalyticsStore> {
    store: Arc<S>,
    config: BatcherConfig,
    receiver: mpsc::Receiver<BatchItem>,
    buffer: Vec<StoredEvent>,
}

/// Item sent to the batcher.
#[derive(Debug)]
pub struct BatchItem {
    /// The stored event (already enriched).
    pub event: StoredEvent,
}

impl<S: AnalyticsStore> Batcher<S> {
    /// Create a new batcher.
    pub fn new(
        store: Arc<S>,
        config: BatcherConfig,
        receiver: mpsc::Receiver<BatchItem>,
    ) -> Self {
        let buffer_capacity = config.max_batch_size;
        Self {
            store,
            config,
            receiver,
            buffer: Vec::with_capacity(buffer_capacity),
        }
    }

    /// Run the batcher loop.
    ///
    /// This consumes the batcher and runs until the channel is closed.
    pub async fn run(mut self) {
        let mut flush_interval = interval(Duration::from_millis(self.config.flush_interval_ms));

        loop {
            tokio::select! {
                // Receive events from channel
                item = self.receiver.recv() => {
                    match item {
                        Some(item) => {
                            self.buffer.push(item.event);

                            // Flush if buffer is full
                            if self.buffer.len() >= self.config.max_batch_size {
                                self.flush().await;
                            }
                        }
                        None => {
                            // Channel closed, flush remaining and exit
                            if !self.buffer.is_empty() {
                                self.flush().await;
                            }
                            tracing::info!("batcher channel closed, exiting");
                            break;
                        }
                    }
                }

                // Periodic flush
                _ = flush_interval.tick() => {
                    if !self.buffer.is_empty() {
                        self.flush().await;
                    }
                }
            }
        }
    }

    /// Flush buffered events to the store.
    async fn flush(&mut self) {
        if self.buffer.is_empty() {
            return;
        }

        let batch_size = self.buffer.len();
        let events = std::mem::take(&mut self.buffer);
        self.buffer = Vec::with_capacity(self.config.max_batch_size);

        let start = std::time::Instant::now();

        match self.store.write_events(&events).await {
            Ok(outcome) => {
                let elapsed = start.elapsed();
                tracing::debug!(
                    batch_size = batch_size,
                    accepted = outcome.accepted,
                    rejected = outcome.rejected.len(),
                    elapsed_ms = elapsed.as_millis() as u64,
                    "flushed event batch"
                );

                // Record metrics
                metrics::counter!("analytics_events_written_total").increment(outcome.accepted as u64);
                metrics::counter!("analytics_events_rejected_total").increment(outcome.rejected.len() as u64);
                metrics::histogram!("analytics_batch_flush_duration_ms").record(elapsed.as_millis() as f64);
            }
            Err(e) => {
                tracing::error!(error = %e, batch_size = batch_size, "failed to flush event batch");
                metrics::counter!("analytics_batch_flush_errors_total").increment(1);

                // TODO: Consider retry logic or dead-letter queue
            }
        }
    }
}

/// Create a batcher channel and return the sender.
pub fn create_batcher_channel(config: &AnalyticsConfig) -> (mpsc::Sender<BatchItem>, mpsc::Receiver<BatchItem>) {
    mpsc::channel(config.batch_queue_depth)
}

/// Convert an IngestEvent to a StoredEvent for storage.
pub fn to_stored_event(
    event: IngestEvent,
    org_id: uuid::Uuid,
    project_id: uuid::Uuid,
    enrichment: &super::enrich::EnrichmentResult,
) -> StoredEvent {
    let now = Utc::now();
    let id = uuid::Uuid::now_v7();

    // Extract library info from context
    let (library_name, library_version) = event
        .context
        .library
        .as_ref()
        .map(|l| (Some(l.name.clone()), Some(l.version.clone())))
        .unwrap_or((None, None));

    StoredEvent {
        id,
        received_at: now,
        timestamp: event.timestamp.unwrap_or(now),
        org_id,
        project_id,
        event: event.event,
        anonymous_id: event.anonymous_id.unwrap_or_else(|| "unknown".to_string()),
        user_id: event.user_id,
        session_id: event.session_id,
        url: enrichment.url.clone(),
        path: enrichment.path.clone(),
        referrer_host: enrichment.referrer_host.clone(),
        utm_source: enrichment.utm_source.clone(),
        country: enrichment.country.clone(),
        device_type: enrichment.device_type.clone(),
        ingest_ip_h24: enrichment.ip_truncated.clone(),
        library_name,
        library_version,
        properties: event.properties,
        context: serde_json::to_value(&event.context).unwrap_or_default(),
    }
}
