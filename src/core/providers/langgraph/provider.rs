//! LangGraph Provider Implementation
//!
//! Main provider implementation for LangGraph Cloud integration

use async_trait::async_trait;
use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use crate::core::providers::base::{
    GlobalPoolManager, HeaderPair, HttpMethod, get_pricing_db, header, header_owned,
    streaming_client,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::traits::provider::ProviderConfig;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::{
    chat::{ChatMessage, ChatRequest},
    context::RequestContext,
    health::HealthStatus,
    message::{MessageContent, MessageRole},
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChoice, ChatChunk, ChatDelta, ChatResponse, ChatStreamChoice, Usage},
};

use super::config::LangGraphConfig;
use super::error::{LangGraphErrorMapper, PROVIDER_NAME};
use super::models::{
    CreateThreadRequest, RunGraphRequest, RunResponse, RunStatus, ThreadState, get_model_registry,
};

/// LangGraph Cloud provider for graph-based agent execution
#[derive(Debug, Clone)]
pub struct LangGraphProvider {
    config: LangGraphConfig,
    pool_manager: Arc<GlobalPoolManager>,
    supported_models: Vec<ModelInfo>,
}

impl LangGraphProvider {
    /// Create a new LangGraph provider with the given configuration
    pub fn new(config: LangGraphConfig) -> Result<Self, ProviderError> {
        config
            .validate()
            .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e))?;

        let pool_manager = Arc::new(
            GlobalPoolManager::new()
                .map_err(|e| ProviderError::configuration(PROVIDER_NAME, e.to_string()))?,
        );

        let supported_models = get_model_registry().to_vec();

        Ok(Self {
            config,
            pool_manager,
            supported_models,
        })
    }

    /// Create a provider from environment variables
    pub fn from_env() -> Result<Self, ProviderError> {
        Self::new(LangGraphConfig::from_env())
    }

    /// Create a provider with just an API key
    pub fn with_api_key(api_key: impl Into<String>) -> Result<Self, ProviderError> {
        Self::new(LangGraphConfig::with_api_key(api_key))
    }

    /// Generate headers for LangGraph API requests
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(3);

        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("x-api-key", api_key.clone()));
        }

        headers.push(header("Content-Type", "application/json".to_string()));

        // Add custom headers
        for (key, value) in &self.config.base.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    }

    /// Create a new thread for stateful conversations
    pub async fn create_thread(
        &self,
        metadata: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<ThreadState, ProviderError> {
        let url = format!("{}/threads", self.config.get_api_base());

        let request = CreateThreadRequest { metadata };
        let body = serde_json::to_value(&request)
            .map_err(|e| ProviderError::serialization(PROVIDER_NAME, e.to_string()))?;

        let headers = self.get_request_headers();
        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&response_bytes);
            return Err(LangGraphErrorMapper.map_http_error(status.as_u16(), &body_str));
        }

        serde_json::from_slice(&response_bytes)
            .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))
    }

    /// Get thread state
    pub async fn get_thread(&self, thread_id: &str) -> Result<ThreadState, ProviderError> {
        let url = format!("{}/threads/{}", self.config.get_api_base(), thread_id);

        let headers = self.get_request_headers();
        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, headers, None)
            .await?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&response_bytes);
            return Err(LangGraphErrorMapper.map_http_error(status.as_u16(), &body_str));
        }

        serde_json::from_slice(&response_bytes)
            .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))
    }

    /// Run a graph execution
    pub async fn run_graph(
        &self,
        thread_id: &str,
        assistant_id: &str,
        input: serde_json::Value,
        config: Option<serde_json::Value>,
    ) -> Result<RunResponse, ProviderError> {
        let url = format!("{}/threads/{}/runs", self.config.get_api_base(), thread_id);

        let request = RunGraphRequest {
            assistant_id: assistant_id.to_string(),
            input,
            config,
            metadata: None,
            stream_mode: None,
            interrupt_before: None,
            interrupt_after: None,
        };

        let body = serde_json::to_value(&request)
            .map_err(|e| ProviderError::serialization(PROVIDER_NAME, e.to_string()))?;

        let headers = self.get_request_headers();
        let response = self
            .pool_manager
            .execute_request(&url, HttpMethod::POST, headers, Some(body))
            .await?;

        let status = response.status();
        let response_bytes = response
            .bytes()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        if !status.is_success() {
            let body_str = String::from_utf8_lossy(&response_bytes);
            return Err(LangGraphErrorMapper.map_http_error(status.as_u16(), &body_str));
        }

        serde_json::from_slice(&response_bytes)
            .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))
    }

    /// Get run result by polling until completion
    pub async fn wait_for_run(
        &self,
        thread_id: &str,
        run_id: &str,
        timeout_secs: u64,
    ) -> Result<RunResponse, ProviderError> {
        let url = format!(
            "{}/threads/{}/runs/{}",
            self.config.get_api_base(),
            thread_id,
            run_id
        );

        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_secs);
        let poll_interval = std::time::Duration::from_millis(500);

        loop {
            if start.elapsed() > timeout {
                return Err(ProviderError::timeout(
                    PROVIDER_NAME,
                    format!("Run {} timed out after {} seconds", run_id, timeout_secs),
                ));
            }

            let headers = self.get_request_headers();
            let response = self
                .pool_manager
                .execute_request(&url, HttpMethod::GET, headers, None)
                .await?;

            let status = response.status();
            let response_bytes = response
                .bytes()
                .await
                .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

            if !status.is_success() {
                let body_str = String::from_utf8_lossy(&response_bytes);
                return Err(LangGraphErrorMapper.map_http_error(status.as_u16(), &body_str));
            }

            let run_response: RunResponse = serde_json::from_slice(&response_bytes)
                .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))?;

            match run_response.status {
                RunStatus::Success => return Ok(run_response),
                RunStatus::Error => {
                    return Err(ProviderError::api_error(
                        PROVIDER_NAME,
                        500,
                        run_response
                            .error
                            .unwrap_or_else(|| "Unknown error".to_string()),
                    ));
                }
                RunStatus::Timeout => {
                    return Err(ProviderError::timeout(
                        PROVIDER_NAME,
                        "Graph execution timed out",
                    ));
                }
                RunStatus::Interrupted => {
                    return Err(ProviderError::cancelled(
                        PROVIDER_NAME,
                        "graph_execution",
                        Some("Run was interrupted".to_string()),
                    ));
                }
                RunStatus::Pending | RunStatus::Running => {
                    tokio::time::sleep(poll_interval).await;
                }
            }
        }
    }

    /// Convert ChatRequest to LangGraph input format
    pub fn transform_chat_to_langgraph_input(&self, request: &ChatRequest) -> serde_json::Value {
        // LangGraph typically expects messages in a specific format
        let messages: Vec<serde_json::Value> = request
            .messages
            .iter()
            .map(|msg| {
                let mut m = serde_json::Map::new();
                m.insert(
                    "role".to_string(),
                    serde_json::Value::String(msg.role.to_string()),
                );
                // Extract text content from MessageContent
                if let Some(content) = &msg.content {
                    match content {
                        MessageContent::Text(text) => {
                            m.insert(
                                "content".to_string(),
                                serde_json::Value::String(text.clone()),
                            );
                        }
                        MessageContent::Parts(parts) => {
                            // For multimodal content, serialize the parts
                            if let Ok(val) = serde_json::to_value(parts) {
                                m.insert("content".to_string(), val);
                            }
                        }
                    }
                }
                if let Some(name) = &msg.name {
                    m.insert("name".to_string(), serde_json::Value::String(name.clone()));
                }
                serde_json::Value::Object(m)
            })
            .collect();

        serde_json::json!({
            "messages": messages
        })
    }

    /// Convert LangGraph output to ChatResponse
    fn transform_langgraph_output_to_chat(
        &self,
        run_response: &RunResponse,
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        // Extract the last message from the output
        let content = if let Some(output) = &run_response.output {
            if let Some(messages) = output.get("messages").and_then(|m| m.as_array()) {
                if let Some(last_msg) = messages.last() {
                    last_msg
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string()
                } else {
                    String::new()
                }
            } else if let Some(content) = output.get("content").and_then(|c| c.as_str()) {
                content.to_string()
            } else {
                // Fallback: serialize the entire output
                serde_json::to_string(output).unwrap_or_default()
            }
        } else {
            String::new()
        };

        Ok(ChatResponse {
            id: request_id.to_string(),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text(content)),
                    thinking: None,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                },
                finish_reason: Some(crate::core::types::responses::FinishReason::Stop),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens: 0,     // LangGraph doesn't provide token counts
                completion_tokens: 0, // Token counting would need the underlying LLM
                total_tokens: 0,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }),
            system_fingerprint: None,
        })
    }
}

#[async_trait]
impl LLMProvider for LangGraphProvider {
    type Config = LangGraphConfig;
    type Error = ProviderError;
    type ErrorMapper = LangGraphErrorMapper;

    fn name(&self) -> &'static str {
        PROVIDER_NAME
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::ToolCalling,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.supported_models
    }

    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[
            "messages",
            "temperature",
            "max_tokens",
            "stream",
            "tools",
            "tool_choice",
        ]
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, Self::Error> {
        // LangGraph can accept most OpenAI params and pass them to the underlying LLM
        Ok(params)
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, Self::Error> {
        // Transform to LangGraph input format
        Ok(self.transform_chat_to_langgraph_input(&request))
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, Self::Error> {
        let run_response: RunResponse = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))?;

        self.transform_langgraph_output_to_chat(&run_response, model, request_id)
    }

    fn get_error_mapper(&self) -> Self::ErrorMapper {
        LangGraphErrorMapper
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, Self::Error> {
        // Get or create a thread
        let thread_id = if let Some(thread_id) = &self.config.thread_id {
            thread_id.clone()
        } else {
            // Create a new thread for this request
            let thread = self.create_thread(None).await?;
            thread.thread_id
        };

        // Get the assistant/graph ID
        let assistant_id = self.config.assistant_id.clone().ok_or_else(|| {
            ProviderError::configuration(
                PROVIDER_NAME,
                "assistant_id is required. Set LANGGRAPH_ASSISTANT_ID or configure assistant_id",
            )
        })?;

        // Transform the request
        let input = self.transform_chat_to_langgraph_input(&request);

        // Run the graph
        let run_response = self
            .run_graph(&thread_id, &assistant_id, input, None)
            .await?;

        // Wait for completion
        let completed_run = self
            .wait_for_run(&thread_id, &run_response.run_id, self.config.base.timeout)
            .await?;

        // Transform to ChatResponse
        self.transform_langgraph_output_to_chat(&completed_run, &request.model, &context.request_id)
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, Self::Error>> + Send>>, Self::Error>
    {
        // Get or create a thread
        let thread_id = if let Some(thread_id) = &self.config.thread_id {
            thread_id.clone()
        } else {
            let thread = self.create_thread(None).await?;
            thread.thread_id
        };

        let assistant_id = self.config.assistant_id.clone().ok_or_else(|| {
            ProviderError::configuration(PROVIDER_NAME, "assistant_id is required for streaming")
        })?;

        let input = self.transform_chat_to_langgraph_input(&request);

        let url = format!(
            "{}/threads/{}/runs/stream",
            self.config.get_api_base(),
            thread_id
        );

        let stream_request = RunGraphRequest {
            assistant_id,
            input,
            config: None,
            metadata: None,
            stream_mode: Some(vec!["values".to_string(), "messages".to_string()]),
            interrupt_before: None,
            interrupt_after: None,
        };

        let api_key = self
            .config
            .get_api_key()
            .ok_or_else(|| ProviderError::authentication(PROVIDER_NAME, "API key is required"))?;

        let client = streaming_client();
        let response = client
            .post(&url)
            .header("x-api-key", api_key)
            .header("Content-Type", "application/json")
            .json(&stream_request)
            .send()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(LangGraphErrorMapper.map_http_error(status.as_u16(), &error_text));
        }

        // Create SSE stream
        let byte_stream = response.bytes_stream();
        let stream = create_langgraph_stream(byte_stream, request.model.clone());

        Ok(Box::pin(stream))
    }

    async fn health_check(&self) -> HealthStatus {
        // Try to list threads as a health check
        let url = format!("{}/threads?limit=1", self.config.get_api_base());

        let headers = self.get_request_headers();
        match self
            .pool_manager
            .execute_request(&url, HttpMethod::GET, headers, None)
            .await
        {
            Ok(response) => {
                if response.status().is_success() {
                    HealthStatus::Healthy
                } else if response.status().as_u16() == 401 {
                    HealthStatus::Unhealthy
                } else {
                    HealthStatus::Degraded
                }
            }
            Err(_) => HealthStatus::Unhealthy,
        }
    }

    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, Self::Error> {
        // LangGraph itself doesn't have fixed costs - costs depend on the underlying LLM
        // Use the pricing database for estimation
        let usage = crate::core::providers::base::pricing::Usage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
            reasoning_tokens: None,
        };

        Ok(get_pricing_db().calculate(model, &usage))
    }
}

/// Create a stream from LangGraph SSE events
fn create_langgraph_stream(
    byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<ChatChunk, ProviderError>> + Send {
    use futures::StreamExt;

    use std::sync::{Arc, Mutex};
    let buffer = Arc::new(Mutex::new(String::new()));

    byte_stream
        .map(move |chunk_result| {
            let model_clone = model.clone();
            match chunk_result {
                Ok(bytes) => {
                    let data = String::from_utf8_lossy(&bytes);
                    Ok((data.to_string(), model_clone))
                }
                Err(e) => Err(ProviderError::network(PROVIDER_NAME, e.to_string())),
            }
        })
        .filter_map(move |result| {
            let buffer_clone = Arc::clone(&buffer);
            async move {
                match result {
                    Ok((data, model)) => {
                        let mut buffer_guard = match buffer_clone.lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        buffer_guard.push_str(&data);

                        // Process complete SSE events
                        let mut chunks = Vec::new();
                        while let Some(event_end) = buffer_guard.find("\n\n") {
                            let event = buffer_guard[..event_end].to_string();
                            *buffer_guard = buffer_guard[event_end + 2..].to_string();

                            if let Some(chunk) = parse_langgraph_sse_event(&event, &model) {
                                chunks.push(Ok(chunk));
                            }
                        }

                        if chunks.is_empty() {
                            None
                        } else {
                            // Return the first chunk, remaining will be in buffer
                            Some(chunks.remove(0))
                        }
                    }
                    Err(e) => Some(Err(e)),
                }
            }
        })
}

/// Parse a single SSE event from LangGraph
fn parse_langgraph_sse_event(event: &str, model: &str) -> Option<ChatChunk> {
    // LangGraph SSE format: event: <type>\ndata: <json>
    let mut event_type = None;
    let mut data = None;

    for line in event.lines() {
        if let Some(et) = line.strip_prefix("event: ") {
            event_type = Some(et.trim());
        } else if let Some(d) = line.strip_prefix("data: ") {
            data = Some(d.trim());
        }
    }

    // Handle different event types
    match (event_type, data) {
        (Some("messages"), Some(json_str)) => {
            // Parse message delta
            if let Ok(msg) = serde_json::from_str::<serde_json::Value>(json_str) {
                let content = msg
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();

                if !content.is_empty() {
                    return Some(ChatChunk {
                        id: uuid::Uuid::new_v4().to_string(),
                        object: "chat.completion.chunk".to_string(),
                        created: chrono::Utc::now().timestamp(),
                        model: model.to_string(),
                        choices: vec![ChatStreamChoice {
                            index: 0,
                            delta: ChatDelta {
                                role: Some(MessageRole::Assistant),
                                content: Some(content),
                                thinking: None,
                                tool_calls: None,
                                function_call: None,
                            },
                            finish_reason: None,
                            logprobs: None,
                        }],
                        usage: None,
                        system_fingerprint: None,
                    });
                }
            }
        }
        (Some("end"), _) => {
            // End of stream
            return Some(ChatChunk {
                id: uuid::Uuid::new_v4().to_string(),
                object: "chat.completion.chunk".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: model.to_string(),
                choices: vec![ChatStreamChoice {
                    index: 0,
                    delta: ChatDelta {
                        role: None,
                        content: None,
                        thinking: None,
                        tool_calls: None,
                        function_call: None,
                    },
                    finish_reason: Some(crate::core::types::responses::FinishReason::Stop),
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            });
        }
        _ => {}
    }

    None
}
