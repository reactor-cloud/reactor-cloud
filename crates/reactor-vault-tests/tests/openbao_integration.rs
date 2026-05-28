//! Integration tests for OpenBao vault backend.
//!
//! These tests require Docker to be running.

use reactor_core::primitives::vault::{SecretValue, Vault};
use reactor_vault::OpenBaoVault;
use reactor_vault_tests::{test_data, test_project_id, OpenBaoTestContainer};

/// Initialize a vault with transit and KV engines enabled.
async fn setup_vault(container_info: &OpenBaoTestContainer) -> OpenBaoVault {
    let config = container_info.config();
    let vault = OpenBaoVault::new(config)
        .await
        .expect("Failed to create vault client");

    // Enable transit engine if not already enabled
    let client = reqwest::Client::new();
    let _ = client
        .post(format!("{}/v1/sys/mounts/transit", container_info.address()))
        .header("X-Vault-Token", container_info.root_token())
        .json(&serde_json::json!({ "type": "transit" }))
        .send()
        .await;

    vault
}

#[tokio::test]
async fn test_transit_encrypt_decrypt_roundtrip() {
    let (container_info, _container) = OpenBaoTestContainer::start().await;
    let vault = setup_vault(&container_info).await;
    let tenant = test_project_id();

    let plaintext = b"Hello, World! This is a test message.".to_vec();

    // Encrypt
    let ciphertext = vault
        .encrypt(&tenant, "test-key", &plaintext)
        .await
        .expect("Encryption failed");

    // The ciphertext should be different from plaintext
    assert_ne!(ciphertext.data.as_bytes(), &plaintext[..]);

    // Decrypt
    let decrypted = vault
        .decrypt(&tenant, "test-key", &ciphertext)
        .await
        .expect("Decryption failed");

    assert_eq!(decrypted, plaintext);
}

#[tokio::test]
async fn test_transit_different_keys_produce_different_ciphertext() {
    let (container_info, _container) = OpenBaoTestContainer::start().await;
    let vault = setup_vault(&container_info).await;
    let tenant = test_project_id();

    let plaintext = b"Same plaintext, different keys".to_vec();

    let ciphertext1 = vault
        .encrypt(&tenant, "key-1", &plaintext)
        .await
        .expect("Encryption with key-1 failed");

    let ciphertext2 = vault
        .encrypt(&tenant, "key-2", &plaintext)
        .await
        .expect("Encryption with key-2 failed");

    // Different keys should produce different ciphertext
    assert_ne!(ciphertext1.data, ciphertext2.data);
}

#[tokio::test]
async fn test_transit_key_rotation() {
    let (container_info, _container) = OpenBaoTestContainer::start().await;
    let vault = setup_vault(&container_info).await;
    let tenant = test_project_id();

    let plaintext = b"Data encrypted before rotation".to_vec();

    // Encrypt with initial key version
    let ciphertext_v1 = vault
        .encrypt(&tenant, "rotate-test", &plaintext)
        .await
        .expect("Initial encryption failed");

    // Rotate key
    let new_version = vault
        .rotate_key(&tenant, "rotate-test")
        .await
        .expect("Key rotation failed");

    assert!(new_version > 1);

    // Old ciphertext should still decrypt
    let decrypted = vault
        .decrypt(&tenant, "rotate-test", &ciphertext_v1)
        .await
        .expect("Decryption of old ciphertext failed");

    assert_eq!(decrypted, plaintext);

    // New encryption should use new key version
    let ciphertext_v2 = vault
        .encrypt(&tenant, "rotate-test", &plaintext)
        .await
        .expect("Encryption after rotation failed");

    // Ciphertext should be different (different key version)
    assert_ne!(ciphertext_v1.data, ciphertext_v2.data);
}

#[tokio::test]
async fn test_kv_secret_roundtrip() {
    let (container_info, _container) = OpenBaoTestContainer::start().await;
    let vault = setup_vault(&container_info).await;
    let tenant = test_project_id();

    let secret_name = "test/secret/path";
    let secret_data = b"super-secret-value".to_vec();

    // Put secret
    vault
        .put_secret(&tenant, secret_name, SecretValue::new(secret_data.clone()))
        .await
        .expect("Put secret failed");

    // Get secret
    let retrieved = vault
        .get_secret(&tenant, secret_name)
        .await
        .expect("Get secret failed")
        .expect("Secret not found");

    assert_eq!(retrieved.data, secret_data);
}

#[tokio::test]
async fn test_kv_secret_update() {
    let (container_info, _container) = OpenBaoTestContainer::start().await;
    let vault = setup_vault(&container_info).await;
    let tenant = test_project_id();

    let secret_name = "test/updateable";

    // Initial value
    vault
        .put_secret(
            &tenant,
            secret_name,
            SecretValue::new(b"initial-value".to_vec()),
        )
        .await
        .expect("Initial put failed");

    let v1 = vault
        .get_secret(&tenant, secret_name)
        .await
        .expect("Get v1 failed")
        .expect("Secret not found");

    assert_eq!(v1.data, b"initial-value");

    // Update
    vault
        .put_secret(
            &tenant,
            secret_name,
            SecretValue::new(b"updated-value".to_vec()),
        )
        .await
        .expect("Update put failed");

    let v2 = vault
        .get_secret(&tenant, secret_name)
        .await
        .expect("Get v2 failed")
        .expect("Secret not found");

    assert_eq!(v2.data, b"updated-value");
}

#[tokio::test]
async fn test_kv_secret_delete() {
    let (container_info, _container) = OpenBaoTestContainer::start().await;
    let vault = setup_vault(&container_info).await;
    let tenant = test_project_id();

    let secret_name = "test/deletable";

    // Create secret
    vault
        .put_secret(
            &tenant,
            secret_name,
            SecretValue::new(b"to-be-deleted".to_vec()),
        )
        .await
        .expect("Put failed");

    // Verify exists
    let exists = vault
        .get_secret(&tenant, secret_name)
        .await
        .expect("Get failed");
    assert!(exists.is_some());

    // Delete
    vault
        .delete_secret(&tenant, secret_name)
        .await
        .expect("Delete failed");

    // Verify deleted
    let deleted = vault
        .get_secret(&tenant, secret_name)
        .await
        .expect("Get after delete failed");
    assert!(deleted.is_none());
}

#[tokio::test]
async fn test_kv_list_secrets() {
    let (container_info, _container) = OpenBaoTestContainer::start().await;
    let vault = setup_vault(&container_info).await;
    let tenant = test_project_id();

    // Create multiple secrets
    for i in 0..5 {
        vault
            .put_secret(
                &tenant,
                &format!("list-test/secret-{}", i),
                SecretValue::new(format!("value-{}", i).into_bytes()),
            )
            .await
            .expect(&format!("Put secret-{} failed", i));
    }

    // List secrets
    let secrets = vault.list_secrets(&tenant).await.expect("List failed");

    assert!(secrets.len() >= 5);
}

#[tokio::test]
async fn test_large_data_roundtrip() {
    let (container_info, _container) = OpenBaoTestContainer::start().await;
    let vault = setup_vault(&container_info).await;
    let tenant = test_project_id();

    // Test with 1MB of data
    let large_data = test_data(1024 * 1024);

    // Transit encryption
    let ciphertext = vault
        .encrypt(&tenant, "large-data-key", &large_data)
        .await
        .expect("Large data encryption failed");

    let decrypted = vault
        .decrypt(&tenant, "large-data-key", &ciphertext)
        .await
        .expect("Large data decryption failed");

    assert_eq!(decrypted, large_data);

    // KV storage
    vault
        .put_secret(&tenant, "large-secret", SecretValue::new(large_data.clone()))
        .await
        .expect("Large secret put failed");

    let retrieved = vault
        .get_secret(&tenant, "large-secret")
        .await
        .expect("Large secret get failed")
        .expect("Large secret not found");

    assert_eq!(retrieved.data, large_data);
}

#[tokio::test]
async fn test_vault_health() {
    let (container_info, _container) = OpenBaoTestContainer::start().await;
    let vault = setup_vault(&container_info).await;

    assert!(vault.is_healthy().await);
    assert!(!vault.is_sealed().await);
}

// Property-based tests would require async proptest support
// For now, we use simple parameterized tests

#[tokio::test]
async fn test_various_plaintext_sizes() {
    let (container_info, _container) = OpenBaoTestContainer::start().await;
    let vault = setup_vault(&container_info).await;
    let tenant = test_project_id();

    let sizes = [1, 16, 64, 256, 1024, 4096, 16384];

    for size in sizes {
        let plaintext = test_data(size);

        let ciphertext = vault
            .encrypt(&tenant, "size-test", &plaintext)
            .await
            .expect(&format!("Encryption failed for size {}", size));

        let decrypted = vault
            .decrypt(&tenant, "size-test", &ciphertext)
            .await
            .expect(&format!("Decryption failed for size {}", size));

        assert_eq!(
            decrypted, plaintext,
            "Roundtrip failed for size {}",
            size
        );
    }
}

#[tokio::test]
async fn test_tenant_isolation() {
    let (container_info, _container) = OpenBaoTestContainer::start().await;
    let vault = setup_vault(&container_info).await;

    let tenant1 = test_project_id();
    let tenant2 = test_project_id();

    // Store secret under tenant1
    vault
        .put_secret(
            &tenant1,
            "isolated-secret",
            SecretValue::new(b"tenant1-data".to_vec()),
        )
        .await
        .expect("Put for tenant1 failed");

    // Store different secret under tenant2
    vault
        .put_secret(
            &tenant2,
            "isolated-secret",
            SecretValue::new(b"tenant2-data".to_vec()),
        )
        .await
        .expect("Put for tenant2 failed");

    // Verify isolation
    let t1_secret = vault
        .get_secret(&tenant1, "isolated-secret")
        .await
        .expect("Get tenant1 failed")
        .expect("tenant1 secret not found");

    let t2_secret = vault
        .get_secret(&tenant2, "isolated-secret")
        .await
        .expect("Get tenant2 failed")
        .expect("tenant2 secret not found");

    assert_eq!(t1_secret.data, b"tenant1-data");
    assert_eq!(t2_secret.data, b"tenant2-data");
}
