//! Doctor command for health checks.

use anyhow::{Context, Result};
use reactor_storage::StorageConfig;

/// Run health checks.
pub async fn run() -> Result<()> {
    println!("reactor-storage-server doctor");
    println!("==============================\n");

    let config = StorageConfig::from_env().context("failed to load config")?;

    // Check database connectivity
    print!("Checking database connection... ");
    match sqlx::PgPool::connect(&config.database_url).await {
        Ok(pool) => {
            // Try a simple query
            match sqlx::query("SELECT 1").fetch_one(&pool).await {
                Ok(_) => println!("OK"),
                Err(e) => {
                    println!("FAILED");
                    println!("  Error executing query: {}", e);
                }
            }
        }
        Err(e) => {
            println!("FAILED");
            println!("  Error: {}", e);
        }
    }

    // Check auth database if monolith mode
    if let Some(ref auth_url) = config.auth_database_url {
        print!("Checking auth database connection... ");
        match sqlx::PgPool::connect(auth_url).await {
            Ok(pool) => {
                match sqlx::query("SELECT 1").fetch_one(&pool).await {
                    Ok(_) => println!("OK"),
                    Err(e) => {
                        println!("FAILED");
                        println!("  Error executing query: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("FAILED");
                println!("  Error: {}", e);
            }
        }
    }

    // Check auth service if microservices mode
    if let Some(ref auth_url) = config.auth_url {
        print!("Checking auth service connection... ");
        let client = reqwest::Client::new();
        let health_url = format!("{}/auth/v1/health", auth_url.trim_end_matches('/'));
        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => println!("OK"),
            Ok(resp) => {
                println!("FAILED");
                println!("  Status: {}", resp.status());
            }
            Err(e) => {
                println!("FAILED");
                println!("  Error: {}", e);
            }
        }
    }

    // Check filesystem storage path if configured
    if let Some(ref path) = config.fs_base_path {
        print!("Checking filesystem storage path... ");
        let path = std::path::Path::new(path);
        if path.exists() {
            if path.is_dir() {
                // Try to write a test file
                let test_file = path.join(".doctor-test");
                match std::fs::write(&test_file, "test") {
                    Ok(_) => {
                        let _ = std::fs::remove_file(&test_file);
                        println!("OK (writable)");
                    }
                    Err(e) => {
                        println!("FAILED");
                        println!("  Path exists but not writable: {}", e);
                    }
                }
            } else {
                println!("FAILED");
                println!("  Path exists but is not a directory");
            }
        } else {
            println!("FAILED");
            println!("  Path does not exist");
        }
    }

    println!("\nDoctor checks complete.");
    Ok(())
}
