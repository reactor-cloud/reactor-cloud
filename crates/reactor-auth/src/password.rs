//! Password hashing with argon2id.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params,
};
use std::fmt;
use thiserror::Error;

/// Password hashing errors.
#[derive(Debug, Error)]
pub enum PasswordError {
    /// Hashing failed.
    #[error("hashing failed: {0}")]
    Hash(String),

    /// Verification failed.
    #[error("verification failed")]
    Verify,

    /// Invalid hash format.
    #[error("invalid hash format")]
    InvalidHash,
}

/// Password hasher with argon2id.
///
/// Parameters: m=64 MiB, t=3, p=1 (as specified in design doc §12)
pub struct PasswordHasherService {
    argon2: Argon2<'static>,
}

impl PasswordHasherService {
    /// Create a new password hasher with default parameters.
    pub fn new() -> Self {
        // m=64 MiB (65536 KiB), t=3 iterations, p=1 parallelism
        let params = Params::new(65536, 3, 1, None).expect("valid argon2 params");
        let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
        Self { argon2 }
    }

    /// Hash a password.
    pub fn hash(&self, password: &str) -> Result<String, PasswordError> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| PasswordError::Hash(e.to_string()))?;
        Ok(hash.to_string())
    }

    /// Verify a password against a hash.
    pub fn verify(&self, password: &str, hash: &str) -> Result<bool, PasswordError> {
        let parsed_hash = PasswordHash::new(hash).map_err(|_| PasswordError::InvalidHash)?;

        match self
            .argon2
            .verify_password(password.as_bytes(), &parsed_hash)
        {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(_) => Err(PasswordError::Verify),
        }
    }
}

impl Default for PasswordHasherService {
    fn default() -> Self {
        Self::new()
    }
}

/// Password policy validation errors.
#[derive(Debug, Clone)]
pub struct PasswordPolicyError(String);

impl fmt::Display for PasswordPolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for PasswordPolicyError {}

/// Password policy for validating password strength.
pub struct PasswordPolicy {
    /// Minimum password length.
    pub min_length: usize,
    /// Maximum password length (prevent DoS).
    pub max_length: usize,
}

impl Default for PasswordPolicy {
    fn default() -> Self {
        Self {
            min_length: 8,
            max_length: 128,
        }
    }
}

impl PasswordPolicy {
    /// Validate a password against the policy.
    pub fn validate(&self, password: &str) -> Result<(), PasswordPolicyError> {
        if password.len() < self.min_length {
            return Err(PasswordPolicyError(format!(
                "password must be at least {} characters",
                self.min_length
            )));
        }
        if password.len() > self.max_length {
            return Err(PasswordPolicyError(format!(
                "password must be at most {} characters",
                self.max_length
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let hasher = PasswordHasherService::new();
        let password = "correct horse battery staple";

        let hash = hasher.hash(password).unwrap();

        assert!(hasher.verify(password, &hash).unwrap());
        assert!(!hasher.verify("wrong password", &hash).unwrap());
    }

    #[test]
    fn test_different_hashes() {
        let hasher = PasswordHasherService::new();
        let password = "same password";

        let hash1 = hasher.hash(password).unwrap();
        let hash2 = hasher.hash(password).unwrap();

        // Due to random salt, hashes should differ
        assert_ne!(hash1, hash2);

        // But both should verify
        assert!(hasher.verify(password, &hash1).unwrap());
        assert!(hasher.verify(password, &hash2).unwrap());
    }

    #[test]
    fn test_invalid_hash() {
        let hasher = PasswordHasherService::new();
        let result = hasher.verify("password", "not a valid hash");
        assert!(result.is_err());
    }
}
