use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u64>,
}

impl Default for TokenUsage {
    fn default() -> Self {
        Self {
            input_tokens: 0,
            output_tokens: 0,
            cache_creation_tokens: None,
            cache_read_tokens: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelPricing {
    pub model_id: String,
    pub input_per_m_tok: f64,
    pub output_per_m_tok: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_per_m_tok: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_per_m_tok: Option<f64>,
    pub fetched_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepCost {
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub pricing: ModelPricing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    UserMessage,
    LlmRequest,
    LlmResponse,
    ToolCall,
    ToolResult,
    SubagentCall,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Running,
    Success,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceStep {
    pub id: String,
    #[serde(rename = "type")]
    pub step_type: StepType,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<StepCost>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_args: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<serde_json::Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_description: Option<String>,

    pub status: StepStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl TraceStep {
    pub fn new(step_type: StepType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            step_type,
            timestamp: chrono::Utc::now().timestamp_millis(),
            duration: None,
            input: None,
            output: None,
            model: None,
            token_usage: None,
            cost: None,
            tool_name: None,
            tool_args: None,
            tool_result: None,
            child_conversation_id: None,
            subagent_type: None,
            subagent_description: None,
            status: StepStatus::Pending,
            error: None,
        }
    }

    pub fn user_message(content: impl Into<String>) -> Self {
        let mut step = Self::new(StepType::UserMessage);
        step.input = Some(serde_json::json!(content.into()));
        step.status = StepStatus::Success;
        step
    }

    pub fn llm_request(model: impl Into<String>, messages: serde_json::Value) -> Self {
        let mut step = Self::new(StepType::LlmRequest);
        step.model = Some(model.into());
        step.input = Some(messages);
        step.status = StepStatus::Running;
        step
    }

    pub fn llm_response(
        model: impl Into<String>,
        content: serde_json::Value,
        token_usage: Option<TokenUsage>,
    ) -> Self {
        let mut step = Self::new(StepType::LlmResponse);
        step.model = Some(model.into());
        step.output = Some(content);
        step.token_usage = token_usage;
        step.status = StepStatus::Success;
        step
    }

    pub fn tool_call(name: impl Into<String>, args: serde_json::Value) -> Self {
        let mut step = Self::new(StepType::ToolCall);
        step.tool_name = Some(name.into());
        step.tool_args = Some(args);
        step.status = StepStatus::Running;
        step
    }

    pub fn tool_result(name: impl Into<String>, result: serde_json::Value, error: Option<String>) -> Self {
        let mut step = Self::new(StepType::ToolResult);
        step.tool_name = Some(name.into());
        step.tool_result = Some(result);
        step.status = if error.is_some() {
            StepStatus::Error
        } else {
            StepStatus::Success
        };
        step.error = error;
        step
    }

    pub fn error(message: impl Into<String>) -> Self {
        let mut step = Self::new(StepType::Error);
        step.error = Some(message.into());
        step.status = StepStatus::Error;
        step
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceMetrics {
    pub total_duration: u64,
    pub llm_calls: u64,
    pub tool_calls: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub estimated_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentMeta {
    pub subagent_type: String,
    pub description: String,
    pub depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationTrace {
    pub conversation_id: String,
    pub agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_conversation_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subagent_meta: Option<SubagentMeta>,
    pub steps: Vec<TraceStep>,
    pub metrics: TraceMetrics,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ConversationTrace {
    pub fn new(conversation_id: impl Into<String>, agent_id: impl Into<String>) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            conversation_id: conversation_id.into(),
            agent_id: agent_id.into(),
            parent_conversation_id: None,
            subagent_meta: None,
            steps: Vec::new(),
            metrics: TraceMetrics::default(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn add_step(&mut self, step: TraceStep) {
        self.steps.push(step);
        self.updated_at = chrono::Utc::now().timestamp_millis();
        self.recompute_metrics();
    }

    pub fn recompute_metrics(&mut self) {
        self.metrics = compute_metrics(&self.steps);
    }
}

pub fn compute_metrics(steps: &[TraceStep]) -> TraceMetrics {
    let mut total_duration = 0u64;
    let mut llm_calls = 0u64;
    let mut tool_calls = 0u64;
    let mut total_input_tokens = 0u64;
    let mut total_output_tokens = 0u64;
    let mut estimated_cost = 0.0f64;

    for step in steps {
        if let Some(duration) = step.duration {
            total_duration += duration;
        }

        match step.step_type {
            StepType::LlmResponse => {
                llm_calls += 1;
                if let Some(ref usage) = step.token_usage {
                    total_input_tokens += usage.input_tokens;
                    total_output_tokens += usage.output_tokens;
                }
                if let Some(ref cost) = step.cost {
                    estimated_cost += cost.total_cost;
                }
            }
            StepType::ToolCall => {
                tool_calls += 1;
            }
            StepType::SubagentCall | StepType::ToolResult => {
                if step.tool_name.as_deref() == Some("task") {
                    if let Some(ref usage) = step.token_usage {
                        total_input_tokens += usage.input_tokens;
                        total_output_tokens += usage.output_tokens;
                    }
                    if let Some(ref cost) = step.cost {
                        estimated_cost += cost.total_cost;
                    }
                }
            }
            _ => {}
        }
    }

    TraceMetrics {
        total_duration,
        llm_calls,
        tool_calls,
        total_input_tokens,
        total_output_tokens,
        estimated_cost,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceSummary {
    pub conversation_id: String,
    pub agent_id: String,
    pub step_count: usize,
    pub metrics: TraceMetrics,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&ConversationTrace> for TraceSummary {
    fn from(trace: &ConversationTrace) -> Self {
        Self {
            conversation_id: trace.conversation_id.clone(),
            agent_id: trace.agent_id.clone(),
            step_count: trace.steps.len(),
            metrics: trace.metrics.clone(),
            created_at: trace.created_at,
            updated_at: trace.updated_at,
        }
    }
}
