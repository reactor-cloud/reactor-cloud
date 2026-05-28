//! Doctor command for health checks.

use anyhow::{Context, Result};
use reactor_functions::FunctionsConfig;

/// Run health checks.
pub async fn run() -> Result<()> {
    println!("reactor-functions-server doctor");
    println!("================================\n");

    let config = FunctionsConfig::from_env().context("failed to load config")?;

    // Check database connectivity
    print!("Checking database connection... ");
    match sqlx::PgPool::connect(&config.database_url).await {
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

    // Check storage service
    print!("Checking storage service connection... ");
    let client = reqwest::Client::new();
    let health_url = format!(
        "{}/storage/v1/health",
        config.storage_url.trim_end_matches('/')
    );
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

    // Check workdir
    print!("Checking workdir... ");
    let path = std::path::Path::new(&config.workdir);
    if path.exists() {
        if path.is_dir() {
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
        println!("WARN (does not exist, will be created)");
    }

    // Check enabled runtimes
    println!("\nRuntime checks:");

    #[cfg(feature = "runtime-wasm")]
    {
        print!("  wasm (wasmtime): ");
        // TODO: PR 6 - Verify wasmtime version
        println!("OK (feature enabled)");
    }

    #[cfg(feature = "runtime-bun")]
    {
        print!("  bun: ");
        match std::process::Command::new(&config.bun_bin)
            .arg("--version")
            .output()
        {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout);
                println!("OK ({})", version.trim());
            }
            Ok(_) => {
                println!("FAILED");
                println!("    bun --version returned non-zero exit");
            }
            Err(e) => {
                println!("FAILED");
                println!("    Error running bun: {}", e);
            }
        }
    }

    #[cfg(feature = "runtime-lambda")]
    {
        print!("  lambda: ");
        // TODO: PR 12 - Verify AWS credentials via STS GetCallerIdentity
        if config.lambda_role_arn.is_some()
            && config.lambda_bundle_s3_bucket.is_some()
            && config.lambda_lwa_layer_arn.is_some()
        {
            println!("OK (config present, AWS credentials not verified)");
        } else {
            println!("WARN (missing lambda config)");
            if config.lambda_role_arn.is_none() {
                println!("    Missing: REACTOR_FUNCTIONS_LAMBDA_ROLE_ARN");
            }
            if config.lambda_bundle_s3_bucket.is_none() {
                println!("    Missing: REACTOR_FUNCTIONS_LAMBDA_BUNDLE_S3_BUCKET");
            }
            if config.lambda_lwa_layer_arn.is_none() {
                println!("    Missing: REACTOR_FUNCTIONS_LAMBDA_LWA_LAYER_ARN");
            }
        }
    }

    println!("\nDoctor checks complete.");
    Ok(())
}
