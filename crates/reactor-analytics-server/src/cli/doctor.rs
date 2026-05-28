//! Health check and configuration diagnostics.

use anyhow::{Context, Result};
use figment::{providers::Env, Figment};
use reactor_analytics::config::AnalyticsConfig;
use sqlx::postgres::PgPoolOptions;
use std::time::Instant;

/// Run diagnostics on the analytics server configuration.
pub async fn run() -> Result<()> {
    println!("reactor-analytics-server doctor\n");
    println!("Checking configuration and connectivity...\n");

    let mut all_ok = true;

    // Check configuration
    print!("  Configuration: ");
    let config = match load_config() {
        Ok(config) => {
            println!("OK");
            Some(config)
        }
        Err(e) => {
            println!("ERROR - {}", e);
            all_ok = false;
            None
        }
    };

    // Check database connectivity
    if let Some(ref config) = config {
        print!("  Database: ");
        match check_database(&config.database_url).await {
            Ok(latency) => {
                println!("OK ({:.2}ms)", latency);
            }
            Err(e) => {
                println!("ERROR - {}", e);
                all_ok = false;
            }
        }
    }

    // Check auth service (if configured)
    if let Some(ref config) = config {
        if let Some(ref auth_url) = config.auth_url {
            print!("  Auth service: ");
            match check_auth_service(auth_url).await {
                Ok(latency) => {
                    println!("OK ({:.2}ms)", latency);
                }
                Err(e) => {
                    println!("ERROR - {}", e);
                    all_ok = false;
                }
            }
        } else {
            println!("  Auth service: NOT CONFIGURED");
            all_ok = false;
        }
    }

    // Check geo database (if configured)
    if let Some(ref config) = config {
        print!("  Geo database: ");
        if let Some(ref geo_path) = config.geo_db_path {
            if geo_path.exists() {
                println!("OK ({})", geo_path.display());
            } else {
                println!("ERROR - file not found: {}", geo_path.display());
                all_ok = false;
            }
        } else {
            println!("NOT CONFIGURED (geo enrichment disabled)");
        }
    }

    // Summary
    println!();
    if all_ok {
        println!("All checks passed!");
        Ok(())
    } else {
        anyhow::bail!("Some checks failed")
    }
}

fn load_config() -> Result<AnalyticsConfig> {
    Figment::new()
        .merge(Env::prefixed("REACTOR_ANALYTICS_").split("_"))
        .merge(Env::prefixed("REACTOR_").split("_"))
        .extract()
        .context("failed to load configuration")
}

async fn check_database(url: &str) -> Result<f64> {
    let start = Instant::now();

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(url)
        .await
        .context("failed to connect")?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .context("failed to execute query")?;

    pool.close().await;

    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

async fn check_auth_service(url: &str) -> Result<f64> {
    let start = Instant::now();

    let client = reqwest::Client::new();
    let health_url = format!("{}/auth/v1/health", url.trim_end_matches('/'));

    let response = client
        .get(&health_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .context("failed to connect")?;

    if !response.status().is_success() {
        anyhow::bail!("health check returned {}", response.status());
    }

    Ok(start.elapsed().as_secs_f64() * 1000.0)
}
