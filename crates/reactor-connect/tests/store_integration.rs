//! Integration tests for PgConnectStore using testcontainers.
//!
//! These tests verify the store operations work correctly against a real Postgres instance.

use reactor_connect::store::{
    ConnectStore, NewInstance, PgConnectStore,
};
use sqlx::PgPool;
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;
use uuid::Uuid;

async fn setup_postgres() -> (ContainerAsync<Postgres>, PgPool) {
    let container = Postgres::default().start().await.unwrap();
    let port = container.get_host_port_ipv4(5432).await.unwrap();
    let connection_string = format!("postgres://postgres:postgres@127.0.0.1:{}/postgres", port);

    let pool = PgPool::connect(&connection_string).await.unwrap();

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();

    (container, pool)
}

fn test_org_id() -> Uuid {
    Uuid::new_v4()
}

// =============================================================================
// Instance Tests
// =============================================================================

#[tokio::test]
async fn test_create_and_get_instance() {
    let (_container, pool) = setup_postgres().await;
    let store = PgConnectStore::new(pool);

    let org_id = test_org_id();
    let new_instance = NewInstance {
        type_id: "stripe".to_string(),
        name: "stripe-prod".to_string(),
        config_json: serde_json::json!({}),
    };

    let instance = store.create_instance(&org_id, &new_instance).await.unwrap();
    assert_eq!(instance.name, "stripe-prod");
    assert_eq!(instance.type_id, "stripe");
    assert_eq!(instance.org_id, org_id);

    // Get by ID
    let fetched = store.get_instance_by_id(&instance.id).await.unwrap().unwrap();
    assert_eq!(fetched.id, instance.id);
    assert_eq!(fetched.name, "stripe-prod");
}

#[tokio::test]
async fn test_list_instances() {
    let (_container, pool) = setup_postgres().await;
    let store = PgConnectStore::new(pool);

    let org_id = test_org_id();

    // Create multiple instances
    for i in 0..3 {
        let instance = NewInstance {
            type_id: "stripe".to_string(),
            name: format!("stripe-{}", i),
            config_json: serde_json::json!({}),
        };
        store.create_instance(&org_id, &instance).await.unwrap();
    }

    let instances = store.list_instances(&org_id).await.unwrap();
    assert_eq!(instances.len(), 3);
}

#[tokio::test]
async fn test_delete_instance() {
    let (_container, pool) = setup_postgres().await;
    let store = PgConnectStore::new(pool);

    let org_id = test_org_id();
    let instance = store
        .create_instance(&org_id, &NewInstance {
            type_id: "stripe".to_string(),
            name: "delete-me".to_string(),
            config_json: serde_json::json!({}),
        })
        .await
        .unwrap();

    store.delete_instance(&instance.id).await.unwrap();

    let fetched = store.get_instance_by_id(&instance.id).await.unwrap();
    assert!(fetched.is_none());
}

#[tokio::test]
async fn test_get_instance_by_name() {
    let (_container, pool) = setup_postgres().await;
    let store = PgConnectStore::new(pool);

    let org_id = test_org_id();
    let instance = store
        .create_instance(&org_id, &NewInstance {
            type_id: "slack".to_string(),
            name: "slack-workspace".to_string(),
            config_json: serde_json::json!({"team_id": "T12345"}),
        })
        .await
        .unwrap();

    // Get by name
    let fetched = store.get_instance(&org_id, "slack-workspace").await.unwrap().unwrap();
    assert_eq!(fetched.id, instance.id);
    assert_eq!(fetched.type_id, "slack");
}

#[tokio::test]
async fn test_unique_instance_name_per_org() {
    let (_container, pool) = setup_postgres().await;
    let store = PgConnectStore::new(pool);

    let org_id = test_org_id();

    // Create first instance
    store
        .create_instance(&org_id, &NewInstance {
            type_id: "github".to_string(),
            name: "github-prod".to_string(),
            config_json: serde_json::json!({}),
        })
        .await
        .unwrap();

    // Try to create duplicate - should fail
    let result = store
        .create_instance(&org_id, &NewInstance {
            type_id: "github".to_string(),
            name: "github-prod".to_string(),
            config_json: serde_json::json!({}),
        })
        .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_same_name_different_orgs() {
    let (_container, pool) = setup_postgres().await;
    let store = PgConnectStore::new(pool);

    let org1 = test_org_id();
    let org2 = test_org_id();

    // Create instance in org1
    let instance1 = store
        .create_instance(&org1, &NewInstance {
            type_id: "linear".to_string(),
            name: "linear-main".to_string(),
            config_json: serde_json::json!({}),
        })
        .await
        .unwrap();

    // Create instance with same name in org2 - should succeed
    let instance2 = store
        .create_instance(&org2, &NewInstance {
            type_id: "linear".to_string(),
            name: "linear-main".to_string(),
            config_json: serde_json::json!({}),
        })
        .await
        .unwrap();

    assert_ne!(instance1.id, instance2.id);
    assert_eq!(instance1.name, instance2.name);
}

// =============================================================================
// Migration Tests
// =============================================================================

#[tokio::test]
async fn test_migrate_idempotent() {
    let (_container, pool) = setup_postgres().await;

    // Run migrations multiple times (already ran once in setup_postgres)
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();

    let store = PgConnectStore::new(pool);

    // Should still work
    let org_id = test_org_id();
    let instance = store
        .create_instance(&org_id, &NewInstance {
            type_id: "stripe".to_string(),
            name: "idempotent-test".to_string(),
            config_json: serde_json::json!({}),
        })
        .await
        .unwrap();

    assert_eq!(instance.name, "idempotent-test");
}
