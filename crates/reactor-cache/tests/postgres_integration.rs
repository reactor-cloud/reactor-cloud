//! Integration tests for PostgresBackend using testcontainers.

use reactor_cache::{KvOperations, PostgresBackend, QueueOperations};
use sqlx::PgPool;
use std::time::Duration;
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;

async fn setup_postgres() -> (ContainerAsync<Postgres>, PgPool) {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&connection_string).await.unwrap();

    (container, pool)
}

// =============================================================================
// Queue Operations Tests
// =============================================================================

#[tokio::test]
async fn test_enqueue_dequeue() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);
    backend.migrate().await.unwrap();

    // Enqueue an item
    let queue = "test-queue";
    let data = b"hello world";
    let id = backend.enqueue(queue, data, None).await.unwrap();
    assert!(!id.is_empty());

    // Dequeue the item
    let items = backend
        .dequeue(queue, 1, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].data, data);
    assert_eq!(items[0].attempt, 1);

    // Ack the item
    backend.ack(queue, &items[0].receipt).await.unwrap();

    // Queue should be empty now
    let items = backend
        .dequeue(queue, 1, Duration::from_secs(30))
        .await
        .unwrap();
    assert!(items.is_empty());
}

#[tokio::test]
async fn test_enqueue_with_delay() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);
    backend.migrate().await.unwrap();

    let queue = "test-delay-queue";
    let data = b"delayed message";

    // Enqueue with 2 second delay
    backend
        .enqueue(queue, data, Some(Duration::from_secs(2)))
        .await
        .unwrap();

    // Should not be visible immediately
    let items = backend
        .dequeue(queue, 1, Duration::from_secs(30))
        .await
        .unwrap();
    assert!(items.is_empty());

    // Wait for delay
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Now should be visible
    let items = backend
        .dequeue(queue, 1, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].data, data);
}

#[tokio::test]
async fn test_nack_requeue() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);
    backend.migrate().await.unwrap();

    let queue = "test-nack-queue";
    let data = b"retry me";

    backend.enqueue(queue, data, None).await.unwrap();

    // Dequeue
    let items = backend
        .dequeue(queue, 1, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    let first_attempt = items[0].attempt;

    // Nack to requeue immediately
    backend.nack(queue, &items[0].receipt, None).await.unwrap();

    // Dequeue again - should get the same item with incremented attempt
    let items = backend
        .dequeue(queue, 1, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].data, data);
    assert_eq!(items[0].attempt, first_attempt + 1);
}

#[tokio::test]
async fn test_queue_len() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);
    backend.migrate().await.unwrap();

    let queue = "test-len-queue";

    assert_eq!(backend.queue_len(queue).await.unwrap(), 0);

    backend.enqueue(queue, b"item1", None).await.unwrap();
    assert_eq!(backend.queue_len(queue).await.unwrap(), 1);

    backend.enqueue(queue, b"item2", None).await.unwrap();
    assert_eq!(backend.queue_len(queue).await.unwrap(), 2);

    // Dequeue and ack one
    let items = backend
        .dequeue(queue, 1, Duration::from_secs(30))
        .await
        .unwrap();
    backend.ack(queue, &items[0].receipt).await.unwrap();

    assert_eq!(backend.queue_len(queue).await.unwrap(), 1);
}

#[tokio::test]
async fn test_dequeue_multiple() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);
    backend.migrate().await.unwrap();

    let queue = "test-multi-queue";

    // Enqueue 5 items
    for i in 0..5 {
        backend
            .enqueue(queue, format!("item-{}", i).as_bytes(), None)
            .await
            .unwrap();
    }

    // Dequeue 3 at once
    let items = backend
        .dequeue(queue, 3, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(items.len(), 3);

    // Dequeue remaining 2
    let items = backend
        .dequeue(queue, 10, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(items.len(), 2);
}

// =============================================================================
// KV Operations Tests
// =============================================================================

#[tokio::test]
async fn test_kv_set_get() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);
    backend.migrate().await.unwrap();

    let key = "test-key";
    let value = b"test-value";

    // Key doesn't exist initially
    assert!(backend.get(key).await.unwrap().is_none());

    // Set and get
    backend.set(key, value, None).await.unwrap();
    let result = backend.get(key).await.unwrap();
    assert_eq!(result, Some(value.to_vec()));
}

#[tokio::test]
async fn test_kv_overwrite() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);
    backend.migrate().await.unwrap();

    let key = "overwrite-key";

    backend.set(key, b"value1", None).await.unwrap();
    assert_eq!(backend.get(key).await.unwrap(), Some(b"value1".to_vec()));

    backend.set(key, b"value2", None).await.unwrap();
    assert_eq!(backend.get(key).await.unwrap(), Some(b"value2".to_vec()));
}

#[tokio::test]
async fn test_kv_delete() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);
    backend.migrate().await.unwrap();

    let key = "delete-key";

    backend.set(key, b"value", None).await.unwrap();
    assert!(backend.exists(key).await.unwrap());

    let deleted = backend.del(key).await.unwrap();
    assert!(deleted);
    assert!(!backend.exists(key).await.unwrap());

    // Delete non-existent key
    let deleted = backend.del(key).await.unwrap();
    assert!(!deleted);
}

#[tokio::test]
async fn test_kv_ttl() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);
    backend.migrate().await.unwrap();

    let key = "ttl-key";

    // Set with 1 second TTL
    backend
        .set(key, b"expiring", Some(Duration::from_secs(1)))
        .await
        .unwrap();
    assert!(backend.exists(key).await.unwrap());

    // Wait for expiry
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Should be expired
    assert!(backend.get(key).await.unwrap().is_none());
    assert!(!backend.exists(key).await.unwrap());
}

#[tokio::test]
async fn test_kv_expire() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);
    backend.migrate().await.unwrap();

    let key = "expire-key";

    // Set without TTL
    backend.set(key, b"value", None).await.unwrap();
    assert!(backend.exists(key).await.unwrap());

    // Add TTL of 1 second
    let updated = backend.expire(key, Duration::from_secs(1)).await.unwrap();
    assert!(updated);

    // Still exists
    assert!(backend.exists(key).await.unwrap());

    // Wait for expiry
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Should be expired
    assert!(!backend.exists(key).await.unwrap());
}

#[tokio::test]
async fn test_kv_exists() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);
    backend.migrate().await.unwrap();

    let key = "exists-key";

    assert!(!backend.exists(key).await.unwrap());

    backend.set(key, b"value", None).await.unwrap();
    assert!(backend.exists(key).await.unwrap());

    backend.del(key).await.unwrap();
    assert!(!backend.exists(key).await.unwrap());
}

// =============================================================================
// Health Check Tests
// =============================================================================

#[tokio::test]
async fn test_health_check() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);

    // Health check should work even without migrations
    backend.health_check().await.unwrap();
}

#[tokio::test]
async fn test_migrate_idempotent() {
    let (_container, pool) = setup_postgres().await;
    let backend = PostgresBackend::new(pool);

    // Running migrations multiple times should be idempotent
    backend.migrate().await.unwrap();
    backend.migrate().await.unwrap();
    backend.migrate().await.unwrap();

    // Should still work
    backend.set("test", b"value", None).await.unwrap();
}
