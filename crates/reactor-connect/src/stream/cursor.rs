//! Cursor/state management for incremental sync.

use crate::protocol::{AirbyteStateMessage, AirbyteStateType, StateBundle};

/// Parse Airbyte state messages into a state bundle.
pub fn parse_state_message(msg: &AirbyteStateMessage, bundle: &mut StateBundle) {
    match msg.state_type {
        AirbyteStateType::Stream => {
            if let Some(stream_state) = &msg.stream {
                bundle.stream_states.insert(
                    stream_state.stream_descriptor.name.clone(),
                    stream_state.stream_state.clone(),
                );
            }
        }
        AirbyteStateType::Global => {
            if let Some(global) = &msg.global {
                bundle.global_state = Some(global.shared_state.clone());
                for stream_state in &global.stream_states {
                    bundle.stream_states.insert(
                        stream_state.stream_descriptor.name.clone(),
                        stream_state.stream_state.clone(),
                    );
                }
            }
        }
        AirbyteStateType::Legacy => {
            if let Some(data) = &msg.data {
                bundle.global_state = Some(data.clone());
            }
        }
    }
}
