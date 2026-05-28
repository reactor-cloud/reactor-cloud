//! Cross-capability integration test harness.
//!
//! Tests the full function lifecycle:
//! signup → create function → upload bundle → deploy → promote → invoke → assert audit + invocations
//!
//! Matrix: {wasm, bun} × {InProcess, Remote} × {Fs, S3}
//! Lambda lane gated on LocalStack availability.

use std::sync::Arc;

/// Test configuration.
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct TestConfig {
    runtime: RuntimeType,
    auth_mode: AuthMode,
    storage_backend: StorageBackend,
}

/// Runtime type for the test.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum RuntimeType {
    Wasm,
    Bun,
    Lambda,
}

/// Auth mode for the test.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum AuthMode {
    InProcess,
    Remote,
}

/// Storage backend for the test.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum StorageBackend {
    Filesystem,
    S3,
}

/// Generate test matrix configurations.
#[allow(dead_code)]
fn generate_test_matrix() -> Vec<TestConfig> {
    let mut configs = Vec::new();

    for runtime in [RuntimeType::Wasm, RuntimeType::Bun] {
        for auth_mode in [AuthMode::InProcess, AuthMode::Remote] {
            for storage in [StorageBackend::Filesystem, StorageBackend::S3] {
                configs.push(TestConfig {
                    runtime,
                    auth_mode,
                    storage_backend: storage,
                });
            }
        }
    }

    // Lambda tests are gated on LocalStack availability
    if std::env::var("LOCALSTACK_ENDPOINT").is_ok() {
        for auth_mode in [AuthMode::InProcess, AuthMode::Remote] {
            for storage in [StorageBackend::Filesystem, StorageBackend::S3] {
                configs.push(TestConfig {
                    runtime: RuntimeType::Lambda,
                    auth_mode,
                    storage_backend: storage,
                });
            }
        }
    }

    configs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matrix_generation() {
        let configs = generate_test_matrix();
        // Without LocalStack: 2 runtimes × 2 auth modes × 2 storage backends = 8
        assert!(configs.len() >= 8);
    }

    // TODO: Implement full integration tests
    // These tests require:
    // 1. testcontainers for Postgres
    // 2. A mock or real storage service
    // 3. Auth service or in-process auth
    // 4. The function server running
    //
    // Test flow:
    // 1. Start containers (Postgres, optionally LocalStack)
    // 2. Run migrations
    // 3. Create auth user + org
    // 4. Create function
    // 5. Upload bundle
    // 6. Deploy
    // 7. Promote
    // 8. Invoke
    // 9. Assert audit_events table has records
    // 10. Assert invocations table has records

    #[tokio::test]
    #[ignore = "requires testcontainers setup"]
    async fn test_wasm_inprocess_fs() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup"]
    async fn test_wasm_inprocess_s3() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup"]
    async fn test_wasm_remote_fs() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup"]
    async fn test_wasm_remote_s3() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup and bun"]
    async fn test_bun_inprocess_fs() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup and bun"]
    async fn test_bun_inprocess_s3() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup and bun"]
    async fn test_bun_remote_fs() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires testcontainers setup and bun"]
    async fn test_bun_remote_s3() {
        // TODO: Implement
    }

    #[tokio::test]
    #[ignore = "requires LocalStack"]
    async fn test_lambda_inprocess_fs() {
        // TODO: Implement - gated on LOCALSTACK_ENDPOINT
    }

    #[tokio::test]
    #[ignore = "requires LocalStack"]
    async fn test_lambda_inprocess_s3() {
        // TODO: Implement - gated on LOCALSTACK_ENDPOINT
    }

    #[tokio::test]
    #[ignore = "requires LocalStack"]
    async fn test_lambda_remote_fs() {
        // TODO: Implement - gated on LOCALSTACK_ENDPOINT
    }

    #[tokio::test]
    #[ignore = "requires LocalStack"]
    async fn test_lambda_remote_s3() {
        // TODO: Implement - gated on LOCALSTACK_ENDPOINT
    }
}
