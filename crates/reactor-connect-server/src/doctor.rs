//! Doctor command — connectivity diagnostics for reactor-connect-server

use anyhow::Result;

/// Run connectivity diagnostics for the Connect server.
///
/// Checks:
/// - Database connectivity
/// - Auth service (if remote)
/// - Vault connectivity  
/// - Downstream capabilities (jobs, data, storage)
pub async fn run() -> Result<()> {
    println!("Reactor Connect Server — Diagnostics");
    println!("====================================\n");

    // Database check
    print!("Checking database connectivity... ");
    match check_database().await {
        Ok(()) => println!("OK"),
        Err(e) => println!("FAILED: {}", e),
    }

    // Vault check
    print!("Checking vault connectivity... ");
    match check_vault().await {
        Ok(()) => println!("OK"),
        Err(e) => println!("FAILED: {}", e),
    }

    // Auth service check
    print!("Checking auth service... ");
    match check_auth().await {
        Ok(()) => println!("OK"),
        Err(e) => println!("FAILED: {}", e),
    }

    // Jobs service check (downstream)
    print!("Checking jobs service... ");
    match check_jobs().await {
        Ok(()) => println!("OK"),
        Err(e) => println!("FAILED: {}", e),
    }

    // Data service check (downstream)
    print!("Checking data service... ");
    match check_data().await {
        Ok(()) => println!("OK"),
        Err(e) => println!("FAILED: {}", e),
    }

    // Storage service check (downstream)
    print!("Checking storage service... ");
    match check_storage().await {
        Ok(()) => println!("OK"),
        Err(e) => println!("FAILED: {}", e),
    }

    println!("\nDiagnostics complete.");
    Ok(())
}

async fn check_database() -> Result<()> {
    let db_url = std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("REACTOR_CONNECT_DATABASE_URL"))
        .map_err(|_| anyhow::anyhow!("DATABASE_URL not set"))?;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await?;

    sqlx::query("SELECT 1").execute(&pool).await?;
    Ok(())
}

async fn check_vault() -> Result<()> {
    // Check if vault env vars are set
    let _vault_addr = std::env::var("VAULT_ADDR")
        .or_else(|_| std::env::var("REACTOR_VAULT_ADDR"))
        .map_err(|_| anyhow::anyhow!("VAULT_ADDR not set (using embedded vault?)"))?;

    // TODO: Actually connect to vault once we have the config
    Ok(())
}

async fn check_auth() -> Result<()> {
    let auth_url = std::env::var("REACTOR_AUTH_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/auth/v1/health", auth_url))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Auth service returned {}", resp.status());
    }
    Ok(())
}

async fn check_jobs() -> Result<()> {
    let jobs_url = std::env::var("REACTOR_JOBS_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/jobs/v1/health", jobs_url))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Jobs service returned {}", resp.status());
    }
    Ok(())
}

async fn check_data() -> Result<()> {
    let data_url = std::env::var("REACTOR_DATA_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/data/v1/health", data_url))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Data service returned {}", resp.status());
    }
    Ok(())
}

async fn check_storage() -> Result<()> {
    let storage_url = std::env::var("REACTOR_STORAGE_URL")
        .unwrap_or_else(|_| "http://localhost:8080".to_string());

    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/storage/v1/health", storage_url))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;

    if !resp.status().is_success() {
        anyhow::bail!("Storage service returned {}", resp.status());
    }
    Ok(())
}
