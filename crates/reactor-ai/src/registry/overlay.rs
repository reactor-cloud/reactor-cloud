//! Registry overlay for per-project customization.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

use super::{Alias, Model, Provider};

/// Registry overlay that can be merged into the base registry.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct RegistryOverlay {
    /// Additional or overriding providers.
    #[serde(default)]
    pub providers: HashMap<String, Provider>,
    /// Additional or overriding models.
    #[serde(default)]
    pub models: Vec<Model>,
    /// Additional or overriding aliases.
    #[serde(default)]
    pub aliases: Vec<Alias>,
}

impl RegistryOverlay {
    /// Load an overlay from a TOML file.
    pub fn from_file(path: &Path) -> Result<Self, crate::error::AiError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| crate::error::AiError::Registry(format!("Failed to read overlay: {}", e)))?;
        Self::from_toml(&content)
    }

    /// Parse an overlay from a TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self, crate::error::AiError> {
        toml::from_str(toml_str)
            .map_err(|e| crate::error::AiError::Registry(format!("Failed to parse overlay: {}", e)))
    }

    /// Load an overlay from a URL.
    pub async fn from_url(url: &str) -> Result<Self, crate::error::AiError> {
        let response = reqwest::get(url)
            .await
            .map_err(|e| crate::error::AiError::Registry(format!("Failed to fetch overlay: {}", e)))?;

        let content = response
            .text()
            .await
            .map_err(|e| crate::error::AiError::Registry(format!("Failed to read overlay response: {}", e)))?;

        Self::from_toml(&content)
    }
}
