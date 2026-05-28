//! Health check diagnostics for reactor-data-server.
//!
//! The `doctor` command checks:
//! - Database connectivity
//! - Metadata migration status
//! - App-role grants and _reactor_data separation
//! - Auth reachability (in microservices mode)
//! - Signing key availability

use anyhow::{Context, Result};
use reactor_data::{DataConfig, Deployment};
use sqlx::Row;

/// ANSI color codes for terminal output.
mod colors {
    pub const GREEN: &str = "\x1b[32m";
    pub const RED: &str = "\x1b[31m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
}

/// Run all health checks.
pub async fn run() -> Result<()> {
    println!(
        "{}reactor-data-server doctor{}",
        colors::BOLD,
        colors::RESET
    );
    println!("Version: {}\n", reactor_data::VERSION);

    let config = DataConfig::from_env().context("failed to load config")?;
    let mut all_passed = true;

    // 1. Check data database connectivity
    print!("Checking data database connectivity... ");
    match check_database(&config.database_url).await {
        Ok(version) => {
            println!("{}OK{} ({})", colors::GREEN, colors::RESET, version);
        }
        Err(e) => {
            println!("{}FAILED{}", colors::RED, colors::RESET);
            println!("  Error: {}", e);
            all_passed = false;
        }
    }

    // 2. Check metadata migrations
    print!("Checking metadata migrations... ");
    match check_metadata_migrations(&config.database_url).await {
        Ok(count) => {
            println!(
                "{}OK{} ({} tables in _reactor_data)",
                colors::GREEN,
                colors::RESET,
                count
            );
        }
        Err(e) => {
            println!("{}FAILED{}", colors::RED, colors::RESET);
            println!("  Error: {}", e);
            all_passed = false;
        }
    }

    // 3. Check _reactor_data schema isolation
    print!("Checking _reactor_data schema isolation... ");
    match check_schema_isolation(&config.database_url).await {
        Ok(()) => {
            println!("{}OK{}", colors::GREEN, colors::RESET);
        }
        Err(e) => {
            println!("{}WARNING{}", colors::YELLOW, colors::RESET);
            println!("  {}", e);
        }
    }

    // 4. Check auth connectivity based on deployment mode
    match config.deployment {
        Deployment::Monolith => {
            print!("Checking auth database (monolith mode)... ");
            if let Some(ref auth_db_url) = config.auth_database_url {
                match check_database(auth_db_url).await {
                    Ok(version) => {
                        println!("{}OK{} ({})", colors::GREEN, colors::RESET, version);
                    }
                    Err(e) => {
                        println!("{}FAILED{}", colors::RED, colors::RESET);
                        println!("  Error: {}", e);
                        all_passed = false;
                    }
                }
            } else {
                println!("{}SKIPPED{} (no auth_database_url)", colors::YELLOW, colors::RESET);
            }

            print!("Checking auth data key... ");
            if config.auth_data_key.is_some() {
                println!("{}OK{} (configured)", colors::GREEN, colors::RESET);
            } else {
                println!("{}MISSING{}", colors::RED, colors::RESET);
                all_passed = false;
            }
        }
        Deployment::Microservices => {
            print!("Checking auth server (microservices mode)... ");
            if let Some(ref auth_url) = config.auth_url {
                match check_auth_server(auth_url).await {
                    Ok(()) => {
                        println!("{}OK{}", colors::GREEN, colors::RESET);
                    }
                    Err(e) => {
                        println!("{}FAILED{}", colors::RED, colors::RESET);
                        println!("  Error: {}", e);
                        all_passed = false;
                    }
                }
            } else {
                println!("{}MISSING{} (no auth_url)", colors::RED, colors::RESET);
                all_passed = false;
            }
        }
    }

    // 5. Check signing keys (via auth)
    print!("Checking signing keys availability... ");
    match check_signing_keys(&config).await {
        Ok(count) => {
            if count > 0 {
                println!(
                    "{}OK{} ({} active keys)",
                    colors::GREEN,
                    colors::RESET,
                    count
                );
            } else {
                println!("{}WARNING{} (no active signing keys)", colors::YELLOW, colors::RESET);
            }
        }
        Err(e) => {
            println!("{}SKIPPED{}", colors::YELLOW, colors::RESET);
            println!("  {}", e);
        }
    }

    // Summary
    println!();
    if all_passed {
        println!(
            "{}All checks passed!{}",
            colors::GREEN,
            colors::RESET
        );
        Ok(())
    } else {
        println!(
            "{}Some checks failed. Please review the errors above.{}",
            colors::RED,
            colors::RESET
        );
        std::process::exit(1);
    }
}

/// Check database connectivity and return version.
async fn check_database(url: &str) -> Result<String> {
    let pool = sqlx::PgPool::connect(url)
        .await
        .context("failed to connect")?;

    let row = sqlx::query("SELECT version()")
        .fetch_one(&pool)
        .await
        .context("failed to query version")?;

    let version: String = row.get(0);
    // Extract just the version number
    let version = version
        .split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");

    pool.close().await;
    Ok(version)
}

/// Check that metadata migrations have been applied.
async fn check_metadata_migrations(url: &str) -> Result<i64> {
    let pool = sqlx::PgPool::connect(url)
        .await
        .context("failed to connect")?;

    // Check if _reactor_data schema exists
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) as count 
        FROM information_schema.tables 
        WHERE table_schema = '_reactor_data'
        "#,
    )
    .fetch_one(&pool)
    .await
    .context("failed to query schema")?;

    let count: i64 = row.get("count");
    pool.close().await;

    if count == 0 {
        anyhow::bail!("_reactor_data schema not found - run metadata migrations first");
    }

    Ok(count)
}

/// Check that _reactor_data is not accessible to regular users.
async fn check_schema_isolation(url: &str) -> Result<()> {
    let pool = sqlx::PgPool::connect(url)
        .await
        .context("failed to connect")?;

    // Check if there are any grants on _reactor_data to non-superusers
    let row = sqlx::query(
        r#"
        SELECT COUNT(*) as count
        FROM information_schema.role_table_grants
        WHERE table_schema = '_reactor_data'
          AND grantee NOT IN ('postgres', current_user)
        "#,
    )
    .fetch_one(&pool)
    .await
    .context("failed to check grants")?;

    let count: i64 = row.get("count");
    pool.close().await;

    if count > 0 {
        anyhow::bail!("found {} grants on _reactor_data to non-admin roles", count);
    }

    Ok(())
}

/// Check auth server health (microservices mode).
async fn check_auth_server(auth_url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let health_url = format!("{}/auth/v1/health", auth_url.trim_end_matches('/'));

    let response = client
        .get(&health_url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .context("failed to reach auth server")?;

    if response.status().is_success() {
        Ok(())
    } else {
        anyhow::bail!("auth server returned status {}", response.status());
    }
}

/// Check signing keys availability.
async fn check_signing_keys(config: &DataConfig) -> Result<i64> {
    match config.deployment {
        Deployment::Monolith => {
            if let Some(ref auth_db_url) = config.auth_database_url {
                let pool = sqlx::PgPool::connect(auth_db_url)
                    .await
                    .context("failed to connect to auth database")?;

                let row = sqlx::query(
                    r#"
                    SELECT COUNT(*) as count 
                    FROM reactor_auth.signing_keys 
                    WHERE status = 'active'
                    "#,
                )
                .fetch_one(&pool)
                .await
                .context("failed to query signing keys")?;

                let count: i64 = row.get("count");
                pool.close().await;
                Ok(count)
            } else {
                anyhow::bail!("no auth database configured");
            }
        }
        Deployment::Microservices => {
            // In microservices mode, we rely on the auth server being healthy
            // and assume it has signing keys if the health check passed
            anyhow::bail!("signing key check requires auth database access (monolith mode)");
        }
    }
}
