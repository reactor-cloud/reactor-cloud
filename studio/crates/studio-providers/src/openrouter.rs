// Ported from 1jehuang/jcode (MIT) - jcode-provider-openrouter/src/lib.rs
// Adapted for Reactor Studio.

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use studio_protocol::{Message, Role, StreamChunk, ToolCall};

use crate::{LlmProvider, ProviderConfig, ProviderError, RequestOptions, ToolDefinition};

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

pub struct OpenRouterProvider {
    client: Client,
    config: ProviderConfig,
}

impl OpenRouterProvider {
    pub fn new(config: ProviderConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }
}

#[derive(Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenRouterTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Serialize)]
struct OpenRouterMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenRouterToolCall>>,
}

#[derive(Serialize, Deserialize)]
struct OpenRouterToolCall {
    id: String,
    r#type: String,
    function: OpenRouterFunction,
}

#[derive(Serialize, Deserialize)]
struct OpenRouterFunction {
    name: String,
    arguments: String,
}

#[derive(Serialize)]
struct OpenRouterTool {
    r#type: String,
    function: OpenRouterToolDefinition,
}

#[derive(Serialize)]
struct OpenRouterToolDefinition {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct StreamResponse {
    choices: Vec<StreamChoice>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct StreamDelta {
    content: Option<String>,
    tool_calls: Option<Vec<StreamToolCall>>,
}

#[derive(Deserialize)]
struct StreamToolCall {
    index: usize,
    id: Option<String>,
    function: Option<StreamFunction>,
}

#[derive(Deserialize)]
struct StreamFunction {
    name: Option<String>,
    arguments: Option<String>,
}

fn convert_role(role: &Role) -> String {
    match role {
        Role::System => "system".to_string(),
        Role::User => "user".to_string(),
        Role::Assistant => "assistant".to_string(),
        Role::Tool => "tool".to_string(),
    }
}

fn convert_messages(messages: &[Message]) -> Vec<OpenRouterMessage> {
    messages
        .iter()
        .map(|m| {
            let tool_calls = m.tool_calls.as_ref().map(|calls| {
                calls
                    .iter()
                    .map(|tc| OpenRouterToolCall {
                        id: tc.id.clone(),
                        r#type: "function".to_string(),
                        function: OpenRouterFunction {
                            name: tc.name.clone(),
                            arguments: tc.arguments.to_string(),
                        },
                    })
                    .collect()
            });

            OpenRouterMessage {
                role: convert_role(&m.role),
                content: m.content.clone(),
                tool_call_id: m.tool_call_id.clone(),
                tool_calls,
            }
        })
        .collect()
}

fn convert_tools(tools: &[ToolDefinition]) -> Vec<OpenRouterTool> {
    tools
        .iter()
        .map(|t| OpenRouterTool {
            r#type: "function".to_string(),
            function: OpenRouterToolDefinition {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.parameters.clone(),
            },
        })
        .collect()
}

fn parse_sse_chunk(
    text: &str,
    tool_calls: &mut HashMap<usize, (String, String, String)>,
) -> Vec<Result<StreamChunk, ProviderError>> {
    let mut chunks = Vec::new();
    
    for line in text.lines() {
        if line.starts_with("data: ") {
            let data = &line[6..];
            if data == "[DONE]" {
                chunks.push(Ok(StreamChunk::done()));
                continue;
            }
            
            if let Ok(response) = serde_json::from_str::<StreamResponse>(data) {
                if let Some(choice) = response.choices.first() {
                    if let Some(content) = &choice.delta.content {
                        if !content.is_empty() {
                            chunks.push(Ok(StreamChunk::text(content)));
                        }
                    }
                    
                    if let Some(tc_list) = &choice.delta.tool_calls {
                        for tc in tc_list {
                            let entry = tool_calls.entry(tc.index).or_insert_with(|| {
                                (String::new(), String::new(), String::new())
                            });
                            
                            if let Some(id) = &tc.id {
                                entry.0 = id.clone();
                            }
                            if let Some(func) = &tc.function {
                                if let Some(name) = &func.name {
                                    entry.1 = name.clone();
                                }
                                if let Some(args) = &func.arguments {
                                    entry.2.push_str(args);
                                }
                            }
                        }
                    }
                    
                    if choice.finish_reason.as_deref() == Some("tool_calls") {
                        for (_, (id, name, args)) in tool_calls.drain() {
                            if let Ok(arguments) = serde_json::from_str(&args) {
                                chunks.push(Ok(StreamChunk::ToolCall(ToolCall {
                                    id,
                                    name,
                                    arguments,
                                })));
                            }
                        }
                    }
                }
            }
        }
    }
    
    chunks
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    async fn stream(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        options: RequestOptions,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamChunk, ProviderError>> + Send>>, ProviderError>
    {
        let request = OpenRouterRequest {
            model: self.config.model.clone(),
            messages: convert_messages(&messages),
            stream: true,
            tools: tools.as_ref().map(|t| convert_tools(t)),
            temperature: options.temperature,
            max_tokens: options.max_tokens,
        };

        let response = self
            .client
            .post(OPENROUTER_API_URL)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::ApiError(error_text));
        }

        let stream = response.bytes_stream();
        let tool_calls: Arc<Mutex<HashMap<usize, (String, String, String)>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let mapped_stream = stream
            .map(move |result| {
                let tool_calls = Arc::clone(&tool_calls);
                match result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        let mut tc = tool_calls.lock().unwrap();
                        let chunks = parse_sse_chunk(&text, &mut tc);
                        futures::stream::iter(chunks)
                    }
                    Err(e) => futures::stream::iter(vec![Err(ProviderError::RequestError(e))]),
                }
            })
            .flatten();

        Ok(Box::pin(mapped_stream))
    }

    async fn generate(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolDefinition>>,
        options: RequestOptions,
    ) -> Result<Message, ProviderError> {
        use futures::TryStreamExt;

        let stream = self.stream(messages, tools, options).await?;
        let chunks: Vec<StreamChunk> = stream.try_collect().await?;

        let mut content = String::new();
        let mut tool_calls = Vec::new();

        for chunk in chunks {
            match chunk {
                StreamChunk::Text { content: text } => content.push_str(&text),
                StreamChunk::ToolCall(tc) => tool_calls.push(tc),
                _ => {}
            }
        }

        let mut message = Message::assistant(content);
        if !tool_calls.is_empty() {
            message = message.with_tool_calls(tool_calls);
        }

        Ok(message)
    }
}
