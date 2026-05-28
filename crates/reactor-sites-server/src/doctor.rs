//! Doctor command for health checks.

use anyhow::{Context, Result};
use reactor_sites::SitesConfig;

pub async fn run() -> Result<()> {
    println!("reactor-sites-server doctor");
    println!("===========================\n");

    let config = SitesConfig::from_env().context("failed to load config")?;

    println!("Checking database connection...");
    match sqlx::PgPool::connect(&config.database_url).await {
        Ok(pool) => {
            let row: (i64,) = sqlx::query_as("SELECT 1")
                .fetch_one(&pool)
                .await
                .context("database query failed")?;
            println!("  ✓ Database connection OK (test query returned {})", row.0);
            pool.close().await;
        }
        Err(e) => {
            println!("  ✗ Database connection FAILED: {}", e);
            return Err(e.into());
        }
    }

    println!("\nChecking functions service...");
    let functions = reactor_sites::dispatch::FunctionsClient::new(
        config.functions_url.clone(),
        config.functions_api_key.clone(),
    );
    match functions.health_check().await {
        Ok(()) => println!("  ✓ Functions service OK"),
        Err(e) => println!("  ✗ Functions service FAILED: {}", e),
    }

    println!("\nChecking storage service...");
    let storage = reactor_sites::dispatch::static_dispatch::StorageClient::new(
        config.storage_url.clone(),
        config.storage_api_key.clone(),
    );
    match storage.health_check().await {
        Ok(()) => println!("  ✓ Storage service OK"),
        Err(e) => println!("  ✗ Storage service FAILED: {}", e),
    }

    println!("\nChecking system bucket...");
    match storage.ensure_system_bucket().await {
        Ok(()) => println!("  ✓ System bucket '_reactor_sites' OK"),
        Err(e) => println!("  ✗ System bucket check FAILED: {}", e),
    }

    println!("\nEnabled frameworks:");
    for framework in reactor_sites::enabled_frameworks() {
        println!("  - {}", framework);
    }

    println!("\nDoctor checks complete.");
    Ok(())
}
