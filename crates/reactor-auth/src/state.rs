//! Application state for reactor-auth.

use crate::config::AuthConfig;
use crate::crypto::ColumnEncryptor;
use crate::email::{EmailSender, NoopSender, SmtpSender};
use crate::service::AuthService;
use crate::store::PgIdentityStore;
use crate::token::KeyringManager;
use reactor_core::auth::AuthError;
use sqlx::PgPool;
use std::sync::Arc;

/// Shared application state for the auth service.
#[derive(Clone)]
pub struct AuthState {
    /// Database connection pool.
    pub pool: PgPool,

    /// Configuration.
    pub config: Arc<AuthConfig>,

    /// The auth service.
    pub service: Arc<AuthService<PgIdentityStore>>,

    /// Keyring manager for signing keys.
    pub keyring: Arc<KeyringManager<PgIdentityStore>>,

    /// The identity store.
    pub store: Arc<PgIdentityStore>,
}

impl AuthState {
    /// Create a new auth state from a pool and config.
    ///
    /// This builds all internal components (store, encryptor, keyring, email sender, service).
    pub fn from_pool(pool: PgPool, config: AuthConfig) -> Result<Self, AuthError> {
        let config = Arc::new(config);

        // Create the identity store
        let store = Arc::new(PgIdentityStore::new(pool.clone()));

        // Create the column encryptor
        let encryptor = ColumnEncryptor::new(&config.data_key).map_err(|e| {
            tracing::error!(error = %e, "failed to create column encryptor");
            AuthError::Internal
        })?;

        // Create the keyring manager
        let keyring = Arc::new(KeyringManager::new(store.clone(), encryptor));

        // Create the email sender (SMTP if configured, else Noop)
        let email_sender: Arc<dyn EmailSender> = if let Some(ref smtp) = config.smtp {
            match SmtpSender::new(smtp) {
                Ok(sender) => Arc::new(sender),
                Err(e) => {
                    tracing::warn!(error = %e, "failed to create SMTP sender, falling back to noop");
                    Arc::new(NoopSender)
                }
            }
        } else {
            Arc::new(NoopSender)
        };

        // Create the auth service
        let service = Arc::new(AuthService::new(
            store.clone(),
            keyring.clone(),
            email_sender,
            config.clone(),
        ));

        Ok(Self {
            pool,
            config,
            service,
            keyring,
            store,
        })
    }

    /// Create a new auth state with pre-built components (for testing).
    pub fn new(
        pool: PgPool,
        config: Arc<AuthConfig>,
        service: Arc<AuthService<PgIdentityStore>>,
        keyring: Arc<KeyringManager<PgIdentityStore>>,
        store: Arc<PgIdentityStore>,
    ) -> Self {
        Self {
            pool,
            config,
            service,
            keyring,
            store,
        }
    }
}
