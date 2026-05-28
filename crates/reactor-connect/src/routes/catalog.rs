//! Catalog endpoints.

use crate::descriptor::ConnectorDescriptor;
use crate::error::ConnectError;
use crate::state::ConnectState;
use crate::store::ConnectStore;
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

/// Catalog entry (summary).
#[derive(Debug, Serialize, Deserialize)]
pub struct CatalogEntry {
    /// Connector type ID.
    pub type_id: String,
    /// Display name.
    pub display_name: String,
    /// Runtime kind.
    pub runtime: String,
    /// Version.
    pub version: String,
    /// Capabilities.
    pub capabilities: crate::descriptor::ConnectorCapabilities,
    /// Documentation URL.
    pub doc_url: Option<String>,
}

/// GET /connect/v1/catalog
pub async fn list<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
) -> Result<Json<Vec<CatalogEntry>>, ConnectError> {
    let type_ids = state.runtime.list_types().await?;
    
    let mut entries = Vec::new();
    for type_id in type_ids {
        if let Ok(desc) = state.runtime.descriptor(&type_id).await {
            entries.push(CatalogEntry {
                type_id: desc.type_id,
                display_name: desc.display_name,
                runtime: desc.runtime.to_string(),
                version: desc.version.to_string(),
                capabilities: desc.capabilities,
                doc_url: desc.doc_url,
            });
        }
    }
    
    Ok(Json(entries))
}

/// GET /connect/v1/catalog/:type_id
pub async fn show<S: ConnectStore + Clone + Send + Sync + 'static>(
    State(state): State<ConnectState<S>>,
    Path(type_id): Path<String>,
) -> Result<Json<ConnectorDescriptor>, ConnectError> {
    let descriptor = state.runtime.descriptor(&type_id).await?;
    Ok(Json(descriptor))
}
