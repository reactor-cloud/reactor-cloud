//! Jobs capability composition.

use super::CapabilitySlot;
use crate::boot::SharedResources;
use crate::config::{JobsConfigSlice, ReactorConfig};
use crate::error::ServerError;
use reactor_cache::{LeaderElect, PostgresBackend};
use reactor_core::auth::AuthClient;
use reactor_core::primitives::vault::Vault;
use reactor_jobs::{Deployment, JobsConfig, JobsState};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;

/// Resolve a secret value, potentially from vault.
///
/// Supports:
/// - Direct value: `"my-secret-key"` - uses the value directly
/// - Vault reference: `"vault:secret-name"` - fetches from vault KV
async fn resolve_secret(
    config_value: &str,
    vault: &dyn Vault,
    tenant: &reactor_core::ProjectId,
    description: &str,
) -> Result<String, ServerError> {
    if config_value.starts_with("vault:") {
        let vault_key = &config_value[6..]; // Strip "vault:" prefix
        let secret = vault
            .get_secret(tenant, vault_key)
            .await
            .map_err(|e| ServerError::Boot(format!("failed to get {} from vault: {}", description, e)))?
            .ok_or_else(|| ServerError::Config(format!(
                "{} '{}' not found in vault", description, vault_key
            )))?;

        String::from_utf8(secret.data)
            .map_err(|_| ServerError::Config(format!("{} is not valid UTF-8", description)))
    } else {
        Ok(config_value.to_string())
    }
}

/// Build the jobs capability slot.
pub async fn build(
    shared: &SharedResources,
    config: &JobsConfigSlice,
    full_config: &ReactorConfig,
    auth_client: Arc<dyn AuthClient>,
) -> Result<CapabilitySlot<JobsState>, ServerError> {
    // Resolve webhook secret (potentially from vault)
    let tenant = full_config.project.project_id();
    let webhook_secret = resolve_secret(
        &config.webhook_secret,
        shared.vault.as_ref(),
        &tenant,
        "jobs webhook secret",
    ).await?;

    // Convert config slice to full JobsConfig
    let jobs_config = JobsConfig {
        database_url: String::new(), // Use shared pool
        bind: "0.0.0.0:8005".parse().unwrap(),
        deployment: Deployment::Monolith,
        functions_url: String::new(), // Use in-process functions
        functions_api_key: String::new(),
        data_url: None,
        data_api_key: None,
        worker_count: config.worker_count,
        scheduler_interval_ms: config.scheduler_interval_ms,
        default_timeout_ms: config.default_timeout_ms,
        max_timeout_ms: config.max_timeout_ms,
        webhook_secret,
        max_org_concurrent_runs: config.max_org_concurrent_runs,
        max_payload_bytes: config.max_payload_bytes,
        auth_url: None,
        internal_secret: None,
        auth_database_url: None,
        auth_data_key: None,
        metrics: false,
        log: "info".to_string(),
    };

    // Get cache backend as PostgresBackend
    let _cache = shared.cache.clone();
    // The cache is already a PostgresBackend in our setup
    // We need to downcast it or create a new one
    let pg_cache = Arc::new(PostgresBackend::new(shared.pg.clone()));

    // Build jobs state
    let state = JobsState::new(
        shared.pg.clone(),
        Arc::new(jobs_config.clone()),
        auth_client,
        pg_cache,
    );

    // Build the router
    let router = reactor_jobs::router(state.clone());

    // Spawn background tasks with leader election
    let mut tasks = Vec::new();

    // Scheduler task - only runs on leader
    let scheduler_state = state.clone();
    let scheduler_shutdown = shared.shutdown.clone();
    let scheduler_leader = shared.leader.clone();
    let scheduler_interval = Duration::from_millis(jobs_config.scheduler_interval_ms);
    let scheduler_task = tokio::spawn(run_leader_task(
        "jobs-scheduler",
        scheduler_leader,
        scheduler_shutdown,
        scheduler_state,
        scheduler_interval,
    ));
    tasks.push(scheduler_task);

    // Worker pool task - runs on all instances (workers coordinate via queue locking)
    let worker_state = state.clone();
    let worker_shutdown = shared.shutdown.clone();
    let worker_count = jobs_config.worker_count;
    let worker_task = tokio::spawn(async move {
        let pool = reactor_jobs::worker::WorkerPool::new(worker_count);
        pool.start(worker_state, worker_shutdown, Duration::from_secs(30))
            .await;
    });
    tasks.push(worker_task);

    tracing::info!("jobs capability composed (scheduler uses leader election)");

    Ok(CapabilitySlot {
        state,
        router,
        tasks,
    })
}

/// Run the jobs scheduler as a leader-elected task.
///
/// Only one instance will run the scheduler at a time. If this instance
/// is not the leader, it will wait and periodically retry.
async fn run_leader_task(
    task_name: &'static str,
    leader: Arc<dyn LeaderElect>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
    state: JobsState,
    interval: Duration,
) {
    loop {
        // Check shutdown first
        if *shutdown.borrow() {
            tracing::info!(task = task_name, "task shutdown before acquiring leadership");
            break;
        }

        // Try to acquire leadership
        match leader.try_acquire(task_name).await {
            Ok(_guard) => {
                tracing::info!(task = task_name, "acquired leadership, starting scheduler");

                // Run the scheduler while we hold leadership
                reactor_jobs::scheduler::start_scheduler(state.clone(), shutdown.clone(), interval)
                    .await;

                // Scheduler exited (likely due to shutdown)
                tracing::info!(task = task_name, "scheduler completed");
                break;
            }
            Err(e) => {
                tracing::debug!(
                    task = task_name,
                    error = %e,
                    "not leader, will retry"
                );

                // Wait before retrying
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(5)) => {}
                    _ = shutdown.changed() => {
                        if *shutdown.borrow() {
                            tracing::info!(task = task_name, "task shutdown while waiting for leadership");
                            break;
                        }
                    }
                }
            }
        }
    }
}
