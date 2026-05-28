use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::types::{ConversationTrace, TraceStep, TraceSummary};

#[derive(Clone)]
pub struct TraceStore {
    traces: Arc<RwLock<HashMap<String, ConversationTrace>>>,
}

impl Default for TraceStore {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceStore {
    pub fn new() -> Self {
        Self {
            traces: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get(&self, conversation_id: &str) -> Option<ConversationTrace> {
        self.traces.read().await.get(conversation_id).cloned()
    }

    pub async fn get_or_create(&self, conversation_id: &str, agent_id: &str) -> ConversationTrace {
        let mut traces = self.traces.write().await;
        traces
            .entry(conversation_id.to_string())
            .or_insert_with(|| ConversationTrace::new(conversation_id, agent_id))
            .clone()
    }

    pub async fn add_step(&self, conversation_id: &str, agent_id: &str, step: TraceStep) {
        let mut traces = self.traces.write().await;
        let trace = traces
            .entry(conversation_id.to_string())
            .or_insert_with(|| ConversationTrace::new(conversation_id, agent_id));
        trace.add_step(step);
    }

    pub async fn update_step<F>(&self, conversation_id: &str, step_id: &str, updater: F)
    where
        F: FnOnce(&mut TraceStep),
    {
        let mut traces = self.traces.write().await;
        if let Some(trace) = traces.get_mut(conversation_id) {
            if let Some(step) = trace.steps.iter_mut().find(|s| s.id == step_id) {
                updater(step);
                trace.updated_at = chrono::Utc::now().timestamp_millis();
                trace.recompute_metrics();
            }
        }
    }

    pub async fn list(&self) -> Vec<TraceSummary> {
        self.traces
            .read()
            .await
            .values()
            .map(TraceSummary::from)
            .collect()
    }

    pub async fn remove(&self, conversation_id: &str) -> Option<ConversationTrace> {
        self.traces.write().await.remove(conversation_id)
    }

    pub async fn clear(&self) {
        self.traces.write().await.clear();
    }
}
