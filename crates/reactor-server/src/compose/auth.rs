//! Auth capability composition.

use super::CapabilitySlot;
use crate::boot::AuthBundle;
use crate::error::ServerError;

/// Build the auth capability slot.
pub fn build(auth_bundle: AuthBundle) -> Result<CapabilitySlot<reactor_auth::AuthState>, ServerError> {
    let state = auth_bundle.state;

    // Build the router using reactor_auth's router factory
    let router = reactor_auth::router(state.clone());

    // Auth has no background tasks at v0
    let tasks = Vec::new();

    tracing::info!("auth capability composed");

    Ok(CapabilitySlot {
        state,
        router,
        tasks,
    })
}
