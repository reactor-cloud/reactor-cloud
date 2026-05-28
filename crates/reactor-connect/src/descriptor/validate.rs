//! Descriptor validation.

use super::ConnectorDescriptor;
use crate::error::ConnectError;

impl ConnectorDescriptor {
    /// Validate the descriptor.
    pub fn validate(&self) -> Result<(), ConnectError> {
        // Check type_id is not empty
        if self.type_id.is_empty() {
            return Err(ConnectError::InvalidInput(
                "type_id cannot be empty".to_string(),
            ));
        }

        // Check display_name is not empty
        if self.display_name.is_empty() {
            return Err(ConnectError::InvalidInput(
                "display_name cannot be empty".to_string(),
            ));
        }

        // Validate auth fields
        if self.auth.fields.is_empty() && !matches!(self.auth.kind, super::AuthKind::Custom { .. })
        {
            return Err(ConnectError::InvalidInput(
                "auth.fields cannot be empty for non-custom auth".to_string(),
            ));
        }

        // Validate streams
        for stream in &self.streams {
            if stream.name.is_empty() {
                return Err(ConnectError::InvalidInput(
                    "stream name cannot be empty".to_string(),
                ));
            }
            if stream.supported_modes.is_empty() {
                return Err(ConnectError::InvalidInput(format!(
                    "stream '{}' must have at least one supported mode",
                    stream.name
                )));
            }
        }

        // Validate actions
        for action in &self.actions {
            if action.name.is_empty() {
                return Err(ConnectError::InvalidInput(
                    "action name cannot be empty".to_string(),
                ));
            }
        }

        // Validate webhooks
        for webhook in &self.webhooks {
            if webhook.name.is_empty() {
                return Err(ConnectError::InvalidInput(
                    "webhook name cannot be empty".to_string(),
                ));
            }
        }

        Ok(())
    }
}
