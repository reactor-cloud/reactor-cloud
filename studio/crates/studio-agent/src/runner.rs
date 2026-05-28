// Ported from 1jehuang/jcode (MIT) - jcode-agent-runtime/src/runner.rs
// Adapted for Reactor Studio.

use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use studio_memory::MemoryManager;
use studio_protocol::{ConversationId, Message, StreamChunk, ToolCall};
use studio_providers::{LlmProvider, RequestOptions, ToolDefinition};
use studio_storage::{AgentDefinition, AgentLoader, ConversationStore, ReactorPaths};
use studio_tools::{ToolContext, ToolRegistry};
use studio_tracing::{TraceStep, TraceStore, TraceWriter};

use crate::{AgentError, ContextBuilder};

const MAX_TOOL_ITERATIONS: usize = 10;

/// Result of a single agent run
pub struct AgentRunResult {
    pub assistant_message: Message,
    pub tool_calls: Vec<ToolCall>,
}

/// Agent runner that manages the agent loop
pub struct AgentRunner {
    paths: ReactorPaths,
    agent_loader: AgentLoader,
    conversation_store: ConversationStore,
    memory_manager: MemoryManager,
    tool_registry: ToolRegistry,
    workspace_name: String,
    trace_store: TraceStore,
    trace_writer: TraceWriter,
}

impl AgentRunner {
    pub fn new(
        workspace_path: impl Into<String>,
        workspace_name: impl Into<String>,
    ) -> Self {
        let workspace_path = workspace_path.into();
        let workspace_name = workspace_name.into();
        let paths = ReactorPaths::new(&workspace_path);
        let trace_writer = TraceWriter::new(paths.project_root());

        Self {
            agent_loader: AgentLoader::new(paths.clone()),
            conversation_store: ConversationStore::new(paths.clone()),
            memory_manager: MemoryManager::new(paths.clone()),
            tool_registry: ToolRegistry::with_defaults(),
            trace_store: TraceStore::new(),
            trace_writer,
            paths,
            workspace_name,
        }
    }

    pub fn trace_store(&self) -> &TraceStore {
        &self.trace_store
    }

    /// Run the agent loop with streaming output
    pub async fn run(
        &self,
        agent_id: &str,
        conversation_id: &ConversationId,
        user_message: &str,
        provider: Arc<dyn LlmProvider>,
        cancellation_token: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamChunk>, AgentError> {
        let (tx, rx) = mpsc::channel(100);

        // Load agent definition
        let agent = self.agent_loader.load(agent_id)?;

        // Get conversation history
        let history = self.memory_manager.get_history(conversation_id, Some(50))?;

        // Save user message
        let user_msg = Message::user(user_message);
        self.memory_manager.save_message(conversation_id, &user_msg)?;

        // Emit user message trace step
        let user_step = TraceStep::user_message(user_message);
        self.trace_store.add_step(conversation_id.as_str(), agent_id, user_step.clone()).await;
        let _ = self.trace_writer.write_step(conversation_id.as_str(), &user_step).await;

        // Build context
        let context_builder = ContextBuilder::new(
            self.paths.project_root().to_string_lossy().to_string(),
            &self.workspace_name,
        );

        // Spawn the agent loop
        let agent_clone = agent.clone();
        let history_clone = history.clone();
        let user_message_clone = user_message.to_string();
        let tool_registry = self.tool_registry.clone();
        let workspace_path = self.paths.project_root().to_string_lossy().to_string();
        let conversation_id_clone = conversation_id.clone();
        let memory_manager_paths = self.paths.clone();
        let trace_store = self.trace_store.clone();
        let trace_writer_path = self.trace_writer.log_path().to_path_buf();
        let agent_id_clone = agent_id.to_string();

        tokio::spawn(async move {
            let trace_writer = TraceWriter::new(trace_writer_path.parent().unwrap().parent().unwrap());

            let result = run_agent_loop(
                agent_clone,
                history_clone,
                user_message_clone,
                provider,
                tool_registry,
                context_builder,
                workspace_path,
                conversation_id_clone.clone(),
                cancellation_token,
                tx.clone(),
                trace_store.clone(),
                trace_writer,
                agent_id_clone.clone(),
            )
            .await;

            match result {
                Ok(final_message) => {
                    // Save final assistant message
                    let memory = MemoryManager::new(memory_manager_paths);
                    let _ = memory.save_message(&conversation_id_clone, &final_message);
                    let _ = tx.send(StreamChunk::done()).await;
                }
                Err(e) => {
                    // Emit error trace step
                    let error_step = TraceStep::error(e.to_string());
                    trace_store.add_step(&conversation_id_clone.as_str(), &agent_id_clone, error_step).await;
                    let _ = tx.send(StreamChunk::error(e.to_string())).await;
                }
            }
        });

        Ok(rx)
    }

    pub fn load_agent(&self, agent_id: &str) -> Result<AgentDefinition, AgentError> {
        self.agent_loader.load(agent_id).map_err(AgentError::from)
    }

    pub fn list_agents(&self) -> Result<Vec<AgentDefinition>, AgentError> {
        self.agent_loader.list_all().map_err(AgentError::from)
    }
}

async fn run_agent_loop(
    agent: AgentDefinition,
    history: Vec<Message>,
    user_message: String,
    provider: Arc<dyn LlmProvider>,
    tool_registry: ToolRegistry,
    context_builder: ContextBuilder,
    workspace_path: String,
    conversation_id: ConversationId,
    cancellation_token: CancellationToken,
    tx: mpsc::Sender<StreamChunk>,
    trace_store: TraceStore,
    trace_writer: TraceWriter,
    agent_id: String,
) -> Result<Message, AgentError> {
    let mut iteration = 0;
    let mut accumulated_content = String::new();
    let mut all_tool_calls: Vec<ToolCall> = Vec::new();
    let mut current_history = history;

    let conv_id = conversation_id.as_str();
    let model_name = agent.model.clone();

    loop {
        if cancellation_token.is_cancelled() {
            return Err(AgentError::Cancelled);
        }

        iteration += 1;
        if iteration > MAX_TOOL_ITERATIONS {
            return Err(AgentError::MaxIterations);
        }

        // Build messages for this iteration
        let messages = if iteration == 1 {
            context_builder.build(&agent, &current_history, &user_message)
        } else {
            context_builder.build(&agent, &current_history, "Continue based on the tool results.")
        };

        // Get tool definitions
        let tool_defs: Vec<ToolDefinition> = tool_registry
            .definitions()
            .into_iter()
            .filter(|t| agent.allowed_tools.is_empty() || agent.allowed_tools.contains(&t.name))
            .map(|t| ToolDefinition {
                name: t.name,
                description: t.description,
                parameters: t.parameters,
            })
            .collect();

        // Emit LLM request trace step
        let request_step = TraceStep::llm_request(&model_name, serde_json::json!({"iteration": iteration}));
        let request_step_id = request_step.id.clone();
        trace_store.add_step(conv_id, &agent_id, request_step.clone()).await;
        let _ = trace_writer.write_step(conv_id, &request_step).await;
        let request_start = std::time::Instant::now();

        // Stream from provider
        let options = RequestOptions {
            temperature: Some(0.7),
            max_tokens: Some(4096),
            ..Default::default()
        };

        let mut stream = provider
            .stream(messages, Some(tool_defs), options)
            .await?;

        // Process stream
        let mut iteration_content = String::new();
        let mut iteration_tool_calls: Vec<ToolCall> = Vec::new();

        while let Some(chunk_result) = stream.next().await {
            if cancellation_token.is_cancelled() {
                return Err(AgentError::Cancelled);
            }

            match chunk_result {
                Ok(chunk) => {
                    match &chunk {
                        StreamChunk::Text { content } => {
                            iteration_content.push_str(content);
                            accumulated_content.push_str(content);
                        }
                        StreamChunk::ToolCall(tc) => {
                            iteration_tool_calls.push(tc.clone());
                            all_tool_calls.push(tc.clone());
                        }
                        _ => {}
                    }
                    // Forward chunk to output
                    let _ = tx.send(chunk).await;
                }
                Err(e) => {
                    return Err(AgentError::Provider(e));
                }
            }
        }

        // Emit LLM response trace step
        let response_duration = request_start.elapsed().as_millis() as u64;
        let response_step = {
            let mut step = TraceStep::llm_response(
                &model_name,
                serde_json::json!({
                    "content": iteration_content,
                    "tool_calls": iteration_tool_calls.len(),
                }),
                None, // TODO: Get actual token usage from provider
            );
            step.duration = Some(response_duration);
            step
        };
        trace_store.add_step(conv_id, &agent_id, response_step.clone()).await;
        let _ = trace_writer.write_step(conv_id, &response_step).await;

        // If no tool calls, we're done
        if iteration_tool_calls.is_empty() {
            return Ok(Message::assistant(accumulated_content));
        }

        // Execute tool calls
        let tool_ctx = ToolContext {
            workspace_path: workspace_path.clone(),
            conversation_id: conversation_id.as_str().to_string(),
        };

        // Add assistant message with tool calls to history
        let assistant_msg = Message::assistant(&iteration_content)
            .with_tool_calls(iteration_tool_calls.clone());
        current_history.push(assistant_msg);

        for tc in &iteration_tool_calls {
            // Emit tool call trace step
            let tool_call_step = TraceStep::tool_call(&tc.name, tc.arguments.clone());
            let tool_call_step_id = tool_call_step.id.clone();
            trace_store.add_step(conv_id, &agent_id, tool_call_step.clone()).await;
            let _ = trace_writer.write_step(conv_id, &tool_call_step).await;
            let tool_start = std::time::Instant::now();

            let result = tool_registry
                .execute(&tc.name, tc.arguments.clone(), &tool_ctx)
                .await;

            let (output, is_error) = match result {
                Ok(r) => (r.output, r.is_error),
                Err(e) => (e.to_string(), true),
            };

            // Emit tool result trace step
            let tool_duration = tool_start.elapsed().as_millis() as u64;
            let tool_result_step = {
                let mut step = TraceStep::tool_result(
                    &tc.name,
                    serde_json::json!(output),
                    if is_error { Some(output.clone()) } else { None },
                );
                step.duration = Some(tool_duration);
                step
            };
            trace_store.add_step(conv_id, &agent_id, tool_result_step.clone()).await;
            let _ = trace_writer.write_step(conv_id, &tool_result_step).await;

            // Send tool result chunk
            let _ = tx
                .send(StreamChunk::ToolResult {
                    tool_call_id: tc.id.clone(),
                    output: output.clone(),
                    is_error,
                })
                .await;

            // Add tool result to history
            current_history.push(Message::tool_result(&tc.id, output));
        }
    }
}

