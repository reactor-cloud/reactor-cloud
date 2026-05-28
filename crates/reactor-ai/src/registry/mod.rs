//! Model registry for resolving aliases and looking up model metadata.

mod overlay;

pub use overlay::RegistryOverlay;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    /// Base URL for the provider API.
    pub base_url: Option<String>,
}

/// A concrete model definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// Unique model identifier (e.g., "anthropic/claude-sonnet-4").
    pub id: String,
    /// Provider name (e.g., "openrouter", "bedrock").
    pub provider: String,
    /// Upstream model ID (what the provider calls it).
    pub upstream_id: String,
    /// Input price per 1M tokens (in dollars).
    pub input_price_per_1m: f64,
    /// Output price per 1M tokens (in dollars).
    pub output_price_per_1m: f64,
    /// Context window size in tokens.
    pub context_window: u32,
    /// Capabilities (e.g., "chat", "reasoning", "vision").
    pub capabilities: Vec<String>,
    /// Speed class ("fast", "medium", "slow").
    pub speed_class: String,
    /// Quality score (0-100).
    #[serde(default = "default_quality_score")]
    pub quality_score: u8,
    /// Fallback model ID for when primary provider fails.
    #[serde(default)]
    pub fallback: Option<String>,
}

fn default_quality_score() -> u8 {
    50
}

impl Model {
    /// Check if this model has a specific capability.
    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }

    /// Calculate blended price (average of input and output per 1M tokens).
    pub fn blended_price(&self) -> f64 {
        (self.input_price_per_1m + self.output_price_per_1m) / 2.0
    }

    /// Get speed score (lower is faster).
    pub fn speed_score(&self) -> u8 {
        match self.speed_class.as_str() {
            "fast" => 1,
            "medium" => 2,
            "slow" => 3,
            _ => 2,
        }
    }
}

/// Resolution strategy for aliases.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ResolveStrategy {
    /// Lowest blended price among models with capability.
    CheapestWithCapability,
    /// Lowest latency (speed_class) among models with capability.
    FastestWithCapability,
    /// Best value (quality/price ratio) among models with capability.
    BestValue,
}

/// An alias definition that maps to a model or resolution strategy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alias {
    /// Alias pattern (e.g., "reasoning/cheapest").
    pub pattern: String,
    /// Direct resolution to a model ID.
    #[serde(default)]
    pub resolves_to: Option<String>,
    /// Strategy-based resolution.
    #[serde(default)]
    pub strategy: Option<ResolveStrategy>,
    /// Required capability for strategy resolution.
    #[serde(default)]
    pub capability: Option<String>,
}

/// Raw registry structure as parsed from TOML.
#[derive(Debug, Clone, Deserialize)]
struct RegistryToml {
    #[serde(default)]
    providers: HashMap<String, Provider>,
    #[serde(default)]
    models: Vec<Model>,
    #[serde(default)]
    aliases: Vec<Alias>,
}

/// The model registry for looking up models and resolving aliases.
#[derive(Debug, Clone)]
pub struct Registry {
    providers: HashMap<String, Provider>,
    models: HashMap<String, Model>,
    aliases: HashMap<String, Alias>,
}

impl Default for Registry {
    fn default() -> Self {
        Self::load_defaults().unwrap_or_else(|_| Self {
            providers: HashMap::new(),
            models: HashMap::new(),
            aliases: HashMap::new(),
        })
    }
}

impl Registry {
    /// Load the registry from the embedded defaults.
    pub fn load_defaults() -> Result<Self, crate::error::AiError> {
        let toml_str = include_str!("defaults.toml");
        Self::from_toml(toml_str)
    }

    /// Parse registry from a TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self, crate::error::AiError> {
        let raw: RegistryToml = toml::from_str(toml_str)
            .map_err(|e| crate::error::AiError::Registry(e.to_string()))?;

        let models: HashMap<String, Model> =
            raw.models.into_iter().map(|m| (m.id.clone(), m)).collect();

        let aliases: HashMap<String, Alias> = raw
            .aliases
            .into_iter()
            .map(|a| (a.pattern.clone(), a))
            .collect();

        Ok(Self {
            providers: raw.providers,
            models,
            aliases,
        })
    }

    /// Apply an overlay to the registry.
    pub fn with_overlay(mut self, overlay: RegistryOverlay) -> Self {
        for model in overlay.models {
            self.models.insert(model.id.clone(), model);
        }
        for alias in overlay.aliases {
            self.aliases.insert(alias.pattern.clone(), alias);
        }
        for (name, provider) in overlay.providers {
            self.providers.insert(name, provider);
        }
        self
    }

    /// Get a model by its exact ID.
    pub fn get_model(&self, id: &str) -> Option<&Model> {
        self.models.get(id)
    }

    /// Get a provider by name.
    pub fn get_provider(&self, name: &str) -> Option<&Provider> {
        self.providers.get(name)
    }

    /// Iterate over all models.
    pub fn models(&self) -> impl Iterator<Item = &Model> {
        self.models.values()
    }

    /// Iterate over all aliases.
    pub fn aliases(&self) -> impl Iterator<Item = &Alias> {
        self.aliases.values()
    }

    /// Resolve a model ID or alias to a concrete model.
    ///
    /// Returns `(model, upstream_model_id)` tuple.
    pub fn resolve(&self, id_or_alias: &str) -> Option<(&Model, String)> {
        // First, try direct model lookup
        if let Some(model) = self.models.get(id_or_alias) {
            return Some((model, model.upstream_id.clone()));
        }

        // Try alias resolution
        if let Some(alias) = self.aliases.get(id_or_alias) {
            return self.resolve_alias(alias);
        }

        None
    }

    /// Resolve an alias to a model.
    fn resolve_alias(&self, alias: &Alias) -> Option<(&Model, String)> {
        // Direct resolution to a specific model
        if let Some(ref target) = alias.resolves_to {
            let model = self.models.get(target)?;
            return Some((model, model.upstream_id.clone()));
        }

        // Strategy-based resolution
        if let (Some(strategy), Some(capability)) = (&alias.strategy, &alias.capability) {
            return self.resolve_strategy(strategy, capability);
        }

        None
    }

    /// Resolve using a strategy.
    fn resolve_strategy(
        &self,
        strategy: &ResolveStrategy,
        capability: &str,
    ) -> Option<(&Model, String)> {
        let candidates: Vec<&Model> = self
            .models
            .values()
            .filter(|m| m.has_capability(capability))
            .collect();

        if candidates.is_empty() {
            return None;
        }

        let chosen = match strategy {
            ResolveStrategy::CheapestWithCapability => candidates
                .into_iter()
                .min_by(|a, b| a.blended_price().partial_cmp(&b.blended_price()).unwrap()),

            ResolveStrategy::FastestWithCapability => candidates
                .into_iter()
                .min_by_key(|m| m.speed_score()),

            ResolveStrategy::BestValue => {
                candidates.into_iter().max_by(|a, b| {
                    let a_value = (a.quality_score as f64) / a.blended_price();
                    let b_value = (b.quality_score as f64) / b.blended_price();
                    a_value.partial_cmp(&b_value).unwrap()
                })
            }
        };

        chosen.map(|m| (m, m.upstream_id.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_defaults() {
        let registry = Registry::load_defaults().expect("Failed to load defaults");
        assert!(!registry.models.is_empty());
    }

    #[test]
    fn test_blended_price() {
        let model = Model {
            id: "test".to_string(),
            provider: "test".to_string(),
            upstream_id: "test".to_string(),
            input_price_per_1m: 2.0,
            output_price_per_1m: 8.0,
            context_window: 100000,
            capabilities: vec![],
            speed_class: "fast".to_string(),
            quality_score: 80,
            fallback: None,
        };
        assert_eq!(model.blended_price(), 5.0);
    }
}
