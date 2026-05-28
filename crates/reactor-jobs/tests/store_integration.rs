//! Integration tests for PgJobsStore using testcontainers.

use reactor_jobs::{
    manifest::{BackoffStrategy, TriggerKind},
    store::{
        JobsStore, NewEvent, NewJob, NewRun, NewStep, NewTrigger, PgJobsStore, RunStatus,
        StepStatus,
    },
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

    (container, pool)
}

fn test_org_id() -> Uuid {
    Uuid::new_v4()
}

// =============================================================================
// Job Tests
// =============================================================================

#[tokio::test]
async fn test_create_and_get_job() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();
    let new_job = NewJob {
        org_id,
        name: "test-job".to_string(),
        function_name: "test-function".to_string(),
        description: Some("Test job description".to_string()),
        retry_max_attempts: 3,
        retry_backoff: BackoffStrategy::Exponential,
        retry_initial_delay_ms: 1000,
        retry_max_delay_ms: 60000,
        max_concurrency: 5,
        timeout_ms: 300000,
    };

    let job = store.create_job(&new_job).await.unwrap();
    assert_eq!(job.name, "test-job");
    assert_eq!(job.function_name, "test-function");
    assert_eq!(job.org_id, org_id);

    // Get by name
    let fetched = store.get_job(org_id, "test-job").await.unwrap().unwrap();
    assert_eq!(fetched.id, job.id);

    // Get by ID
    let fetched = store.get_job_by_id(job.id).await.unwrap().unwrap();
    assert_eq!(fetched.name, "test-job");
}

#[tokio::test]
async fn test_list_jobs() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();

    // Create multiple jobs
    for i in 0..3 {
        let job = NewJob {
            org_id,
            name: format!("job-{}", i),
            function_name: format!("func-{}", i),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Linear,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        };
        store.create_job(&job).await.unwrap();
    }

    let jobs = store.list_jobs(org_id).await.unwrap();
    assert_eq!(jobs.len(), 3);
}

#[tokio::test]
async fn test_delete_job() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();
    let job = store
        .create_job(&NewJob {
            org_id,
            name: "delete-me".to_string(),
            function_name: "func".to_string(),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Exponential,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        })
        .await
        .unwrap();

    store.delete_job(job.id).await.unwrap();

    let fetched = store.get_job(org_id, "delete-me").await.unwrap();
    assert!(fetched.is_none());
}

// =============================================================================
// Trigger Tests
// =============================================================================

#[tokio::test]
async fn test_create_cron_trigger() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();
    let job = store
        .create_job(&NewJob {
            org_id,
            name: "cron-job".to_string(),
            function_name: "func".to_string(),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Exponential,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        })
        .await
        .unwrap();

    let trigger = store
        .create_trigger(&NewTrigger {
            job_id: job.id,
            kind: TriggerKind::Cron,
            config_json: serde_json::json!({"schedule": "0 9 * * *"}),
            webhook_token: None,
            next_trigger_at: Some(chrono::Utc::now() + chrono::Duration::hours(1)),
        })
        .await
        .unwrap();

    assert_eq!(trigger.kind, "cron");
    assert!(trigger.enabled);

    // List triggers
    let triggers = store.get_triggers(job.id).await.unwrap();
    assert_eq!(triggers.len(), 1);
}

#[tokio::test]
async fn test_webhook_trigger() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();
    let job = store
        .create_job(&NewJob {
            org_id,
            name: "webhook-job".to_string(),
            function_name: "func".to_string(),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Exponential,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        })
        .await
        .unwrap();

    let token = "test-webhook-token-12345";
    let trigger = store
        .create_trigger(&NewTrigger {
            job_id: job.id,
            kind: TriggerKind::Webhook,
            config_json: serde_json::json!({}),
            webhook_token: Some(token.to_string()),
            next_trigger_at: None,
        })
        .await
        .unwrap();

    // Get by webhook token
    let fetched = store
        .get_trigger_by_webhook_token(token)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fetched.id, trigger.id);
}

// =============================================================================
// Run Tests
// =============================================================================

#[tokio::test]
async fn test_create_and_manage_run() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();
    let job = store
        .create_job(&NewJob {
            org_id,
            name: "run-job".to_string(),
            function_name: "func".to_string(),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Exponential,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        })
        .await
        .unwrap();

    let run = store
        .create_run(&NewRun {
            job_id: job.id,
            org_id,
            trigger_id: None,
            trigger_kind: TriggerKind::Manual,
            payload_json: serde_json::json!({"key": "value"}),
            max_attempts: 3,
        })
        .await
        .unwrap();

    assert_eq!(run.status, "pending");
    assert_eq!(run.attempt, 1);

    // Update status to running
    store
        .update_run_status(run.id, RunStatus::Running, None, None)
        .await
        .unwrap();

    let run = store.get_run(run.id).await.unwrap().unwrap();
    assert_eq!(run.status, "running");

    // Update to succeeded
    store
        .update_run_status(run.id, RunStatus::Succeeded, None, None)
        .await
        .unwrap();

    let run = store.get_run(run.id).await.unwrap().unwrap();
    assert_eq!(run.status, "succeeded");
}

#[tokio::test]
async fn test_run_sleeping() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();
    let job = store
        .create_job(&NewJob {
            org_id,
            name: "sleep-job".to_string(),
            function_name: "func".to_string(),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Exponential,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        })
        .await
        .unwrap();

    let run = store
        .create_run(&NewRun {
            job_id: job.id,
            org_id,
            trigger_id: None,
            trigger_kind: TriggerKind::Manual,
            payload_json: serde_json::json!({}),
            max_attempts: 3,
        })
        .await
        .unwrap();

    // Set sleeping with wakeup time in the past
    let wakeup = chrono::Utc::now() - chrono::Duration::seconds(1);
    store.set_run_sleeping(run.id, wakeup).await.unwrap();

    let run = store.get_run(run.id).await.unwrap().unwrap();
    assert_eq!(run.status, "sleeping");

    // List sleeping runs that are due
    let due_runs = store.list_sleeping_runs_due(chrono::Utc::now()).await.unwrap();
    assert_eq!(due_runs.len(), 1);
}

#[tokio::test]
async fn test_count_active_runs() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();
    let job = store
        .create_job(&NewJob {
            org_id,
            name: "concurrent-job".to_string(),
            function_name: "func".to_string(),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Exponential,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        })
        .await
        .unwrap();

    // Create 3 runs
    for _ in 0..3 {
        store
            .create_run(&NewRun {
                job_id: job.id,
                org_id,
                trigger_id: None,
                trigger_kind: TriggerKind::Manual,
                payload_json: serde_json::json!({}),
                max_attempts: 3,
            })
            .await
            .unwrap();
    }

    // All pending = active
    let count = store.count_active_runs(job.id).await.unwrap();
    assert_eq!(count, 3);

    let org_count = store.count_active_runs_for_org(org_id).await.unwrap();
    assert_eq!(org_count, 3);
}

// =============================================================================
// Step Tests
// =============================================================================

#[tokio::test]
async fn test_steps() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();
    let job = store
        .create_job(&NewJob {
            org_id,
            name: "step-job".to_string(),
            function_name: "func".to_string(),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Exponential,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        })
        .await
        .unwrap();

    let run = store
        .create_run(&NewRun {
            job_id: job.id,
            org_id,
            trigger_id: None,
            trigger_kind: TriggerKind::Manual,
            payload_json: serde_json::json!({}),
            max_attempts: 3,
        })
        .await
        .unwrap();

    // Create step (starts in running status per implementation)
    let step = store
        .create_step(&NewStep {
            run_id: run.id,
            name: "fetch-data".to_string(),
            input_json: Some(serde_json::json!({"url": "http://example.com"})),
        })
        .await
        .unwrap();

    assert_eq!(step.name, "fetch-data");
    assert_eq!(step.status, "running"); // Steps start in running status

    // Get step by name
    let fetched = store.get_step(run.id, "fetch-data").await.unwrap().unwrap();
    assert_eq!(fetched.id, step.id);

    // Update step to completed with output
    let output = serde_json::json!({"data": "result"});
    store
        .update_step(step.id, StepStatus::Completed, Some(&output), None)
        .await
        .unwrap();

    let fetched = store.get_step(run.id, "fetch-data").await.unwrap().unwrap();
    assert_eq!(fetched.status, "completed");
    assert_eq!(fetched.output_json, Some(output));

    // List steps
    let steps = store.list_steps(run.id).await.unwrap();
    assert_eq!(steps.len(), 1);
}

// =============================================================================
// State Tests
// =============================================================================

#[tokio::test]
async fn test_state() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();
    let job = store
        .create_job(&NewJob {
            org_id,
            name: "state-job".to_string(),
            function_name: "func".to_string(),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Exponential,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        })
        .await
        .unwrap();

    let run = store
        .create_run(&NewRun {
            job_id: job.id,
            org_id,
            trigger_id: None,
            trigger_kind: TriggerKind::Manual,
            payload_json: serde_json::json!({}),
            max_attempts: 3,
        })
        .await
        .unwrap();

    // Set state
    let value = serde_json::json!({"counter": 42});
    store.set_state(run.id, "my-key", &value).await.unwrap();

    // Get state
    let fetched = store.get_state(run.id, "my-key").await.unwrap().unwrap();
    assert_eq!(fetched, value);

    // List state
    let all_state = store.list_state(run.id).await.unwrap();
    assert_eq!(all_state.len(), 1);

    // Delete state
    store.delete_state(run.id, "my-key").await.unwrap();
    let fetched = store.get_state(run.id, "my-key").await.unwrap();
    assert!(fetched.is_none());
}

// =============================================================================
// Event Tests
// =============================================================================

#[tokio::test]
async fn test_events() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();
    let job = store
        .create_job(&NewJob {
            org_id,
            name: "event-job".to_string(),
            function_name: "func".to_string(),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Exponential,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        })
        .await
        .unwrap();

    let run = store
        .create_run(&NewRun {
            job_id: job.id,
            org_id,
            trigger_id: None,
            trigger_kind: TriggerKind::Manual,
            payload_json: serde_json::json!({}),
            max_attempts: 3,
        })
        .await
        .unwrap();

    // Emit event
    let event = store
        .emit_event(&NewEvent {
            org_id,
            topic: "orders.created".to_string(),
            payload_json: serde_json::json!({"order_id": "123"}),
            emitted_by_run_id: Some(run.id),
        })
        .await
        .unwrap();

    assert_eq!(event.topic, "orders.created");
    assert!(event.consumed_at.is_none());

    // List pending events
    let pending = store.list_pending_events("orders.created", 10).await.unwrap();
    assert_eq!(pending.len(), 1);

    // Consume event
    store.consume_event(event.id, run.id).await.unwrap();

    // Should no longer be pending
    let pending = store.list_pending_events("orders.created", 10).await.unwrap();
    assert!(pending.is_empty());
}

// =============================================================================
// DLQ Tests
// =============================================================================

#[tokio::test]
async fn test_dlq() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);
    store.migrate().await.unwrap();

    let org_id = test_org_id();
    let job = store
        .create_job(&NewJob {
            org_id,
            name: "dlq-job".to_string(),
            function_name: "func".to_string(),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Exponential,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        })
        .await
        .unwrap();

    let run = store
        .create_run(&NewRun {
            job_id: job.id,
            org_id,
            trigger_id: None,
            trigger_kind: TriggerKind::Manual,
            payload_json: serde_json::json!({"data": "test"}),
            max_attempts: 3,
        })
        .await
        .unwrap();

    // First update the run status with an error (move_to_dlq copies from run)
    store
        .update_run_status(
            run.id,
            RunStatus::Failed,
            Some("MAX_ATTEMPTS"),
            Some("Max attempts exceeded"),
        )
        .await
        .unwrap();

    // Move to DLQ
    store.move_to_dlq(run.id, "Max attempts exceeded").await.unwrap();

    // List DLQ
    let dlq = store.list_dlq(job.id, 10).await.unwrap();
    assert_eq!(dlq.len(), 1);
    assert_eq!(dlq[0].error_message, Some("Max attempts exceeded".to_string()));
    assert_eq!(dlq[0].error_code, Some("MAX_ATTEMPTS".to_string()));

    // Retry from DLQ (creates new run)
    let new_run_id = store.retry_from_dlq(dlq[0].id).await.unwrap();
    assert_ne!(new_run_id, run.id);

    // DLQ entry should be deleted
    let dlq = store.list_dlq(job.id, 10).await.unwrap();
    assert!(dlq.is_empty());
}

// =============================================================================
// Migration Tests
// =============================================================================

#[tokio::test]
async fn test_migrate_idempotent() {
    let (_container, pool) = setup_postgres().await;
    let store = PgJobsStore::new(pool);

    // Run migrations multiple times
    store.migrate().await.unwrap();
    store.migrate().await.unwrap();
    store.migrate().await.unwrap();

    // Should still work
    let org_id = test_org_id();
    let job = store
        .create_job(&NewJob {
            org_id,
            name: "idempotent-job".to_string(),
            function_name: "func".to_string(),
            description: None,
            retry_max_attempts: 3,
            retry_backoff: BackoffStrategy::Exponential,
            retry_initial_delay_ms: 1000,
            retry_max_delay_ms: 60000,
            max_concurrency: 10,
            timeout_ms: 300000,
        })
        .await
        .unwrap();

    assert_eq!(job.name, "idempotent-job");
}
