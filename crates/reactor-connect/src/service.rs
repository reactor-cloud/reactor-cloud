//! Connect service orchestration.

use crate::error::ConnectError;
use crate::state::{ConnectCtx, ConnectState};
use crate::store::ConnectStore;

/// The Connect service orchestrates connector operations.
///
/// This is the main entry point for business logic, called by route handlers.
pub struct ConnectService<'a, S: ConnectStore + Clone + 'static> {
    state: &'a ConnectState<S>,
    ctx: &'a ConnectCtx,
}

impl<'a, S: ConnectStore + Clone + 'static> ConnectService<'a, S> {
    /// Create a new service instance for a request.
    pub fn new(state: &'a ConnectState<S>, ctx: &'a ConnectCtx) -> Self {
        Self { state, ctx }
    }

    /// Get the state.
    pub fn state(&self) -> &ConnectState<S> {
        self.state
    }

    /// Get the context.
    pub fn ctx(&self) -> &ConnectCtx {
        self.ctx
    }

    /// Invoke an action on a connector instance.
    pub async fn invoke_action(
        &self,
        instance_name: &str,
        action_name: &str,
        input: serde_json::Value,
        idempotency_key: Option<String>,
        dry_run: bool,
    ) -> Result<serde_json::Value, ConnectError> {
        // 1. Load instance
        let instance = self
            .state
            .store
            .get_instance(self.ctx.active_org(), instance_name)
            .await?
            .ok_or_else(|| ConnectError::InstanceNotFound(instance_name.to_string()))?;

        // 2. Check credentials
        if instance.credential_state != "ready" {
            return Err(ConnectError::CredentialsNotConfigured(
                instance_name.to_string(),
            ));
        }

        // 3. Get connector descriptor
        let descriptor = self.state.runtime.descriptor(&instance.type_id).await?;

        // 4. Find action
        let action = descriptor
            .actions
            .iter()
            .find(|a| a.name == action_name)
            .ok_or_else(|| ConnectError::ActionNotFound(action_name.to_string()))?;

        // 5. Check dry-run support
        if dry_run {
            use crate::descriptor::DryRunSupport;
            match action.dry_run {
                DryRunSupport::Unsupported => {
                    return Err(ConnectError::DryRunNotSupported(action_name.to_string()));
                }
                DryRunSupport::Native | DryRunSupport::Synthesized => {
                    // OK to proceed
                }
            }
        }

        // 6. Check idempotency
        if let Some(key) = &idempotency_key {
            let cache_key = format!(
                "connect:idempotency:{}:{}:{}",
                self.ctx.active_org(),
                instance.id,
                key
            );
            if let Ok(Some(_)) = self.state.cache.get(&cache_key).await {
                // Return cached result
                // TODO: Store and return actual cached response
                tracing::debug!(idempotency_key = %key, "Idempotency key hit");
            }
        }

        // 7. Load credentials from vault
        let _credentials = self.load_credentials(&instance).await?;

        // 8. Invoke via runtime
        let opts = crate::runtime::ActionOpts {
            dry_run,
            idempotency_key: idempotency_key.clone(),
        };

        let result = self
            .state
            .runtime
            .invoke_action(&instance.type_id, &instance.config_json, action_name, &input, &opts)
            .await?;

        // 9. Store idempotency key
        if let Some(key) = &idempotency_key {
            let cache_key = format!(
                "connect:idempotency:{}:{}:{}",
                self.ctx.active_org(),
                instance.id,
                key
            );
            let ttl = action
                .idempotency
                .as_ref()
                .map(|h| std::time::Duration::from_secs(h.ttl_seconds))
                .unwrap_or(std::time::Duration::from_secs(86400));
            let _ = self
                .state
                .cache
                .set(&cache_key, &[], Some(ttl))
                .await;
        }

        // 10. Record invocation
        let _ = self
            .state
            .store
            .record_invocation(&crate::store::ActionInvocationRecord {
                id: uuid::Uuid::now_v7(),
                instance_id: instance.id,
                org_id: *self.ctx.active_org(),
                action_name: action_name.to_string(),
                input_hash: None,
                idempotency_key,
                dry_run,
                status: "succeeded".to_string(),
                duration_ms: None,
                error_code: None,
                error_message: None,
                created_at: chrono::Utc::now(),
            })
            .await;

        Ok(result)
    }

    /// Load credentials for an instance from vault.
    async fn load_credentials(
        &self,
        instance: &crate::store::Instance,
    ) -> Result<crate::credentials::Credentials, ConnectError> {
        let vault_ref = instance
            .vault_ref
            .as_ref()
            .ok_or_else(|| ConnectError::CredentialsNotConfigured(instance.name.clone()))?;

        // Parse vault ref to extract the secret name
        // Format: tenant/{project_id}/connect/{org_id}/instances/{instance_id}
        let secret_name = vault_ref
            .strip_prefix("tenant/")
            .and_then(|s| s.split_once('/'))
            .map(|(_, rest)| rest)
            .unwrap_or(vault_ref);

        // Use a nil project ID for now - in production this comes from tenant context
        let project_id = reactor_core::ProjectId::nil();

        let secret = self
            .state
            .vault
            .get_secret(&project_id, secret_name)
            .await
            .map_err(|e| ConnectError::Vault(e.to_string()))?
            .ok_or_else(|| ConnectError::CredentialsNotConfigured(instance.name.clone()))?;

        let credentials: crate::credentials::Credentials =
            serde_json::from_slice(&secret.data).map_err(|e| ConnectError::Vault(e.to_string()))?;

        Ok(credentials)
    }
}
