//! Doctor command — health checks for reactor-jobs-server.

use anyhow::{Context, Result};
use reactor_cache::PostgresBackend;
use reactor_jobs::JobsConfig;

/// Run doctor checks.
pub async fn run() -> Result<()> {
    println!("reactor-jobs-server doctor");
    println!("==========================\n");

    let config = JobsConfig::from_env().context("failed to load config")?;

    let mut all_ok = true;

    // Check database connectivity
    print!("Database connection... ");
    match check_database(&config).await {
        Ok(_) => println!("OK"),
        Err(e) => {
            println!("FAILED: {}", e);
            all_ok = false;
        }
    }

    // Check cache backend
    print!("Cache backend... ");
    match check_cache(&config).await {
        Ok(_) => println!("OK"),
        Err(e) => {
            println!("FAILED: {}", e);
            all_ok = false;
        }
    }

    // Check reactor-functions connectivity
    print!("reactor-functions... ");
    match check_functions(&config).await {
        Ok(_) => println!("OK"),
        Err(e) => {
            println!("FAILED: {}", e);
            all_ok = false;
        }
    }

    // Check auth connectivity
    print!("Auth service... ");
    match check_auth(&config).await {
        Ok(_) => println!("OK"),
        Err(e) => {
            println!("FAILED: {}", e);
            all_ok = false;
        }
    }

    // Optional: Check reactor-data connectivity
    if config.data_url.is_some() {
        print!("reactor-data... ");
        match check_data(&config).await {
            Ok(_) => println!("OK"),
            Err(e) => {
                println!("FAILED: {}", e);
                all_ok = false;
            }
        }
    }

    println!();
    if all_ok {
        println!("All checks passed!");
        Ok(())
    } else {
        anyhow::bail!("Some checks failed");
    }
}

async fn check_database(config: &JobsConfig) -> Result<()> {
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .context("failed to connect")?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .context("query failed")?;

    // Check if migrations have been applied
    sqlx::query("SELECT 1 FROM _reactor_jobs.jobs LIMIT 0")
        .execute(&pool)
        .await
        .context("jobs table not found - run migrations")?;

    Ok(())
}

async fn check_cache(config: &JobsConfig) -> Result<()> {
    let pool = sqlx::PgPool::connect(&config.database_url)
        .await
        .context("failed to connect")?;

    let cache = PostgresBackend::new(pool);

    // Run a round-trip test
    use reactor_cache::QueueOperations;
    use std::time::Duration;

    let test_queue = "_doctor_test";
    let test_data = b"doctor_check";

    // Enqueue
    let _id = cache
        .enqueue(test_queue, test_data, None)
        .await
        .context("enqueue failed")?;

    // Dequeue
    let items = cache
        .dequeue(test_queue, 1, Duration::from_secs(5))
        .await
        .context("dequeue failed")?;

    if items.is_empty() {
        anyhow::bail!("dequeue returned no items");
    }

    // Ack
    cache
        .ack(test_queue, &items[0].receipt)
        .await
        .context("ack failed")?;

    Ok(())
}

async fn check_functions(config: &JobsConfig) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/fn/v1/health", config.functions_url);

    let response: reqwest::Response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .context("request failed")?;

    if !response.status().is_success() {
        anyhow::bail!("health check returned {}", response.status());
    }

    Ok(())
}

async fn check_auth(config: &JobsConfig) -> Result<()> {
    match config.deployment {
        reactor_jobs::Deployment::Monolith => {
            // Check auth database
            let auth_db_url = config
                .auth_database_url
                .as_ref()
                .context("auth_database_url not configured")?;

            let pool = sqlx::PgPool::connect(auth_db_url)
                .await
                .context("failed to connect to auth database")?;

            sqlx::query("SELECT 1")
                .execute(&pool)
                .await
                .context("auth database query failed")?;

            Ok(())
        }
        reactor_jobs::Deployment::Microservices => {
            // Check remote auth service
            let auth_url = config
                .auth_url
                .as_ref()
                .context("auth_url not configured")?;

            let client = reqwest::Client::new();
            let url = format!("{}/auth/v1/health", auth_url);

            let response: reqwest::Response = client
                .get(&url)
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await
                .context("request failed")?;

            if !response.status().is_success() {
                anyhow::bail!("health check returned {}", response.status());
            }

            Ok(())
        }
    }
}

async fn check_data(config: &JobsConfig) -> Result<()> {
    let data_url = config
        .data_url
        .as_ref()
        .context("data_url not configured")?;

    let client = reqwest::Client::new();
    let url = format!("{}/data/v1/health", data_url);

    let response: reqwest::Response = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .context("request failed")?;

    if !response.status().is_success() {
        anyhow::bail!("health check returned {}", response.status());
    }

    Ok(())
}
