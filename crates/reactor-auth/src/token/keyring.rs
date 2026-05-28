//! Signing key management and rotation.

use crate::crypto::{DataEncryptor, ColumnEncryptor};
use crate::store::{IdentityStore, SigningKey};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reactor_core::auth::{JsonWebKey, Jwks};
use reactor_core::ReactorId;
use rsa::pkcs1::{DecodeRsaPrivateKey, EncodeRsaPrivateKey, EncodeRsaPublicKey};
use rsa::pkcs8::LineEnding;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;

/// Key management errors.
#[derive(Debug, Error)]
pub enum KeyError {
    /// RSA key generation failed.
    #[error("key generation failed: {0}")]
    Generation(String),

    /// Key encoding failed.
    #[error("key encoding failed: {0}")]
    Encoding(String),

    /// Key decoding failed.
    #[error("key decoding failed: {0}")]
    Decoding(String),

    /// No active signing key available.
    #[error("no active signing key")]
    NoActiveKey,

    /// Encryption error.
    #[error("encryption error: {0}")]
    Encryption(#[from] crate::crypto::CryptoError),

    /// Store error.
    #[error("store error: {0}")]
    Store(#[from] reactor_core::auth::AuthError),
}

/// RSA key pair for JWT signing.
#[derive(Clone)]
pub struct KeyPair {
    /// Key ID.
    pub kid: String,
    /// Private key.
    pub private_key: RsaPrivateKey,
    /// Public key.
    pub public_key: RsaPublicKey,
}

impl KeyPair {
    /// Generate a new RSA-2048 key pair.
    pub fn generate() -> Result<Self, KeyError> {
        let mut rng = rand::thread_rng();
        let private_key =
            RsaPrivateKey::new(&mut rng, 2048).map_err(|e| KeyError::Generation(e.to_string()))?;
        let public_key = RsaPublicKey::from(&private_key);
        let kid = format!("k_{}", ReactorId::new());

        Ok(Self {
            kid,
            private_key,
            public_key,
        })
    }

    /// Export public key to PEM format.
    pub fn public_key_pem(&self) -> Result<String, KeyError> {
        self.public_key
            .to_pkcs1_pem(LineEnding::LF)
            .map_err(|e| KeyError::Encoding(e.to_string()))
    }

    /// Export private key to PEM format.
    pub fn private_key_pem(&self) -> Result<String, KeyError> {
        self.private_key
            .to_pkcs1_pem(LineEnding::LF)
            .map(|s| s.to_string())
            .map_err(|e| KeyError::Encoding(e.to_string()))
    }

    /// Create a KeyPair from PEM strings.
    pub fn from_pem(kid: String, private_key_pem: &str) -> Result<Self, KeyError> {
        let private_key = RsaPrivateKey::from_pkcs1_pem(private_key_pem)
            .map_err(|e| KeyError::Decoding(e.to_string()))?;
        let public_key = RsaPublicKey::from(&private_key);

        Ok(Self {
            kid,
            private_key,
            public_key,
        })
    }

    /// Convert to a JsonWebKey (public key only).
    pub fn to_jwk(&self) -> Result<JsonWebKey, KeyError> {
        let n = self.public_key.n();
        let e = self.public_key.e();

        let n_bytes = n.to_bytes_be();
        let e_bytes = e.to_bytes_be();

        Ok(JsonWebKey::rsa(
            self.kid.clone(),
            URL_SAFE_NO_PAD.encode(&n_bytes),
            URL_SAFE_NO_PAD.encode(&e_bytes),
        ))
    }

    /// Create a SigningKey record for storage (sync version for backwards compatibility).
    pub fn to_signing_key(&self, encryptor: &ColumnEncryptor) -> Result<SigningKey, KeyError> {
        let private_pem = self.private_key_pem()?;
        let encrypted_private = encryptor.encrypt_string(&private_pem)?;

        Ok(SigningKey {
            kid: self.kid.clone(),
            algorithm: "RS256".to_string(),
            private_key_pem: encrypted_private,
            public_key_pem: self.public_key_pem()?,
            created_at: chrono::Utc::now(),
            activated_at: chrono::Utc::now(),
            rotated_at: None,
            retired_at: None,
        })
    }

    /// Load a KeyPair from a SigningKey record (sync version for backwards compatibility).
    pub fn from_signing_key(
        key: &SigningKey,
        encryptor: &ColumnEncryptor,
    ) -> Result<Self, KeyError> {
        let private_pem = encryptor.decrypt_string(&key.private_key_pem)?;
        Self::from_pem(key.kid.clone(), &private_pem)
    }

    /// Create a SigningKey record for storage (async version).
    pub async fn to_signing_key_async(
        &self,
        encryptor: &dyn DataEncryptor,
    ) -> Result<SigningKey, KeyError> {
        let private_pem = self.private_key_pem()?;
        let encrypted_private = encryptor.encrypt_string(&private_pem).await?;

        Ok(SigningKey {
            kid: self.kid.clone(),
            algorithm: "RS256".to_string(),
            private_key_pem: encrypted_private,
            public_key_pem: self.public_key_pem()?,
            created_at: chrono::Utc::now(),
            activated_at: chrono::Utc::now(),
            rotated_at: None,
            retired_at: None,
        })
    }

    /// Load a KeyPair from a SigningKey record (async version).
    pub async fn from_signing_key_async(
        key: &SigningKey,
        encryptor: &dyn DataEncryptor,
    ) -> Result<Self, KeyError> {
        let private_pem = encryptor.decrypt_string(&key.private_key_pem).await?;
        Self::from_pem(key.kid.clone(), &private_pem)
    }
}

/// Cached keyring for JWT signing and verification.
#[derive(Clone)]
pub struct Keyring {
    /// Active key for signing.
    active: Option<KeyPair>,
    /// All non-retired keys (for verification).
    all_keys: Vec<KeyPair>,
    /// Last refresh time.
    last_refresh: chrono::DateTime<chrono::Utc>,
}

impl Keyring {
    /// Create an empty keyring.
    pub fn empty() -> Self {
        Self {
            active: None,
            all_keys: Vec::new(),
            last_refresh: chrono::Utc::now(),
        }
    }

    /// Get the active signing key.
    pub fn active_key(&self) -> Option<&KeyPair> {
        self.active.as_ref()
    }

    /// Find a key by ID.
    pub fn find_key(&self, kid: &str) -> Option<&KeyPair> {
        self.all_keys.iter().find(|k| k.kid == kid)
    }

    /// Convert to JWKS.
    pub fn to_jwks(&self) -> Result<Jwks, KeyError> {
        let mut jwks = Jwks::new();
        for key in &self.all_keys {
            jwks.add_key(key.to_jwk()?);
        }
        Ok(jwks)
    }

    /// Check if the keyring needs refresh.
    pub fn needs_refresh(&self, refresh_interval: std::time::Duration) -> bool {
        let elapsed = chrono::Utc::now()
            .signed_duration_since(self.last_refresh)
            .to_std()
            .unwrap_or(refresh_interval);
        elapsed >= refresh_interval
    }
}

/// Thread-safe keyring manager.
///
/// Supports both sync (`ColumnEncryptor`) and async (`DataEncryptor`) encryption.
pub struct KeyringManager<S: IdentityStore> {
    store: Arc<S>,
    encryptor: Arc<dyn DataEncryptor>,
    keyring: Arc<RwLock<Keyring>>,
}

impl<S: IdentityStore> KeyringManager<S> {
    /// Create a new keyring manager with a sync encryptor (backwards compatible).
    pub fn new(store: Arc<S>, encryptor: ColumnEncryptor) -> Self {
        Self {
            store,
            encryptor: Arc::new(encryptor),
            keyring: Arc::new(RwLock::new(Keyring::empty())),
        }
    }

    /// Create a new keyring manager with an async encryptor.
    pub fn with_encryptor(store: Arc<S>, encryptor: Arc<dyn DataEncryptor>) -> Self {
        Self {
            store,
            encryptor,
            keyring: Arc::new(RwLock::new(Keyring::empty())),
        }
    }

    /// Refresh the keyring from the store.
    pub async fn refresh(&self) -> Result<(), KeyError> {
        let keys = self.store.get_jwks_keys().await?;

        let mut key_pairs = Vec::with_capacity(keys.len());
        let mut active: Option<KeyPair> = None;

        for key in keys {
            let kp = KeyPair::from_signing_key_async(&key, self.encryptor.as_ref()).await?;
            if key.rotated_at.is_none() {
                active = Some(KeyPair::from_signing_key_async(&key, self.encryptor.as_ref()).await?);
            }
            key_pairs.push(kp);
        }

        let mut keyring = self.keyring.write().await;
        keyring.active = active;
        keyring.all_keys = key_pairs;
        keyring.last_refresh = chrono::Utc::now();

        Ok(())
    }

    /// Get a read lock on the keyring.
    pub async fn keyring(&self) -> tokio::sync::RwLockReadGuard<'_, Keyring> {
        self.keyring.read().await
    }

    /// Ensure the keyring has an active key, generating one if needed.
    pub async fn ensure_active_key(&self) -> Result<(), KeyError> {
        {
            let keyring = self.keyring.read().await;
            if keyring.active.is_some() {
                return Ok(());
            }
        }

        // Check store
        if let Some(key) = self.store.get_active_signing_key().await? {
            let kp = KeyPair::from_signing_key_async(&key, self.encryptor.as_ref()).await?;
            let mut keyring = self.keyring.write().await;
            keyring
                .all_keys
                .push(KeyPair::from_signing_key_async(&key, self.encryptor.as_ref()).await?);
            keyring.active = Some(kp);
            return Ok(());
        }

        // Generate a new key
        tracing::info!("generating initial signing key");
        let kp = KeyPair::generate()?;
        let signing_key = kp.to_signing_key_async(self.encryptor.as_ref()).await?;
        self.store.store_signing_key(&signing_key).await?;

        let mut keyring = self.keyring.write().await;
        keyring
            .all_keys
            .push(KeyPair::from_signing_key_async(&signing_key, self.encryptor.as_ref()).await?);
        keyring.active = Some(kp);

        Ok(())
    }

    /// Rotate the signing key.
    pub async fn rotate(&self) -> Result<String, KeyError> {
        let old_kid = {
            let keyring = self.keyring.read().await;
            keyring
                .active
                .as_ref()
                .map(|k| k.kid.clone())
                .ok_or(KeyError::NoActiveKey)?
        };

        let new_kp = KeyPair::generate()?;
        let new_signing_key = new_kp.to_signing_key_async(self.encryptor.as_ref()).await?;
        let new_kid = new_kp.kid.clone();

        self.store
            .rotate_signing_key(&old_kid, &new_signing_key)
            .await?;

        self.refresh().await?;

        tracing::info!(old_kid = %old_kid, new_kid = %new_kid, "rotated signing key");

        Ok(new_kid)
    }

    /// Retire old keys.
    pub async fn retire_old_keys(&self) -> Result<u64, KeyError> {
        let count = self.store.retire_old_signing_keys().await?;
        if count > 0 {
            self.refresh().await?;
            tracing::info!(count = count, "retired old signing keys");
        }
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as BASE64;

    fn test_encryptor() -> ColumnEncryptor {
        ColumnEncryptor::new(&BASE64.encode(&[0u8; 32])).unwrap()
    }

    #[test]
    fn test_key_pair_generation() {
        let kp = KeyPair::generate().unwrap();
        assert!(kp.kid.starts_with("k_"));
    }

    #[test]
    fn test_key_pair_pem_roundtrip() {
        let kp = KeyPair::generate().unwrap();
        let private_pem = kp.private_key_pem().unwrap();
        let public_pem = kp.public_key_pem().unwrap();

        let kp2 = KeyPair::from_pem(kp.kid.clone(), &private_pem).unwrap();
        assert_eq!(kp.kid, kp2.kid);
        assert_eq!(kp2.public_key_pem().unwrap(), public_pem);
    }

    #[test]
    fn test_signing_key_encryption_roundtrip() {
        let encryptor = test_encryptor();
        let kp = KeyPair::generate().unwrap();

        let signing_key = kp.to_signing_key(&encryptor).unwrap();
        let kp2 = KeyPair::from_signing_key(&signing_key, &encryptor).unwrap();

        assert_eq!(kp.kid, kp2.kid);
    }

    #[test]
    fn test_to_jwk() {
        let kp = KeyPair::generate().unwrap();
        let jwk = kp.to_jwk().unwrap();

        assert_eq!(jwk.kty, "RSA");
        assert_eq!(jwk.kid.as_deref(), Some(kp.kid.as_str()));
        assert_eq!(jwk.alg.as_deref(), Some("RS256"));
        assert!(jwk.n.is_some());
        assert!(jwk.e.is_some());
    }
}
