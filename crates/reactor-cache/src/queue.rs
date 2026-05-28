//! Queue operations trait.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::time::Duration;

use crate::CacheError;

/// An item dequeued from a queue.
#[derive(Debug, Clone)]
pub struct QueueItem {
    /// Unique item ID.
    pub id: String,
    /// Receipt handle for ack/nack.
    pub receipt: String,
    /// Item data.
    pub data: Vec<u8>,
    /// When the item was enqueued.
    pub enqueued_at: DateTime<Utc>,
    /// Delivery attempt number.
    pub attempt: u32,
}

/// Queue operations.
#[async_trait]
pub trait QueueOperations: Send + Sync {
    /// Enqueue an item to a queue.
    ///
    /// Returns the item ID.
    async fn enqueue(
        &self,
        queue: &str,
        item: &[u8],
        delay: Option<Duration>,
    ) -> Result<String, CacheError>;

    /// Dequeue items from a queue.
    ///
    /// Items become invisible for the visibility timeout duration.
    /// Call `ack` to permanently remove, or `nack` to make visible again.
    async fn dequeue(
        &self,
        queue: &str,
        count: u32,
        visibility_timeout: Duration,
    ) -> Result<Vec<QueueItem>, CacheError>;

    /// Acknowledge successful processing of an item.
    ///
    /// Permanently removes the item from the queue.
    async fn ack(&self, queue: &str, receipt: &str) -> Result<(), CacheError>;

    /// Negative acknowledge - return item to queue.
    ///
    /// Makes the item visible again after an optional delay.
    async fn nack(
        &self,
        queue: &str,
        receipt: &str,
        delay: Option<Duration>,
    ) -> Result<(), CacheError>;

    /// Get the approximate number of items in a queue.
    async fn queue_len(&self, queue: &str) -> Result<u64, CacheError>;
}
