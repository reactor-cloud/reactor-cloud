//! Token refresh background worker.

use crate::state::ConnectState;
use crate::store::ConnectStore;
use std::time::Duration;
use tokio::sync::watch;

/// Start the token refresh background worker.
///
/// This worker periodically checks for OAuth2 tokens that are about to expire
/// and refreshes them before they become invalid.
pub async fn start_refresh_worker<S: ConnectStore>(
    state: ConnectState<S>,
    mut shutdown: watch::Receiver<bool>,
    interval: Duration,
) {
    let mut interval_timer = tokio::time::interval(interval);

    loop {
        tokio::select! {
            _ = interval_timer.tick() => {
                if let Err(e) = refresh_expiring_tokens(&state).await {
                    tracing::error!(error = %e, "Token refresh worker error");
                }
            }
            _ = shutdown.changed() => {
                if *shutdown.borrow() {
                    tracing::info!("Token refresh worker shutting down");
                    break;
                }
            }
        }
    }
}

async fn refresh_expiring_tokens<S: ConnectStore>(
    _state: &ConnectState<S>,
) -> Result<(), crate::error::ConnectError> {
    // TODO: Implement token refresh
    // 1. Query instances with credential_state = 'ready' and OAuth2 credentials
    // 2. Load credentials from vault
    // 3. Check if expires_at - 5min < now
    // 4. If yes, call token endpoint with refresh_token
    // 5. Update vault with new tokens
    // 6. Update instance updated_at

    tracing::debug!("Token refresh check (placeholder)");
    Ok(())
}
