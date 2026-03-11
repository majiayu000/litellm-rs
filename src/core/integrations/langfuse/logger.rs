//! Langfuse Logger
//!
//! LLM call logging with callback interface for the Langfuse platform.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use super::client::{BatchSender, LangfuseClient, LangfuseError};
use super::config::LangfuseConfig;
use super::types::{Generation, IngestionEvent, Level, Span, Trace, Usage};

/// LLM request information for logging
#[derive(Debug, Clone)]
pub struct LlmRequest {
    /// Request ID
    pub request_id: String,
    /// Model name
    pub model: String,
    /// Input messages/prompt
    pub input: serde_json::Value,
    /// Model parameters
    pub parameters: HashMap<String, serde_json::Value>,
    /// User ID
    pub user_id: Option<String>,
    /// Session ID
    pub session_id: Option<String>,
    /// Request metadata
    pub metadata: HashMap<String, serde_json::Value>,
    /// Tags
    pub tags: Vec<String>,
    /// Provider name
    pub provider: Option<String>,
    /// Request timestamp
    pub timestamp: DateTime<Utc>,
}

impl LlmRequest {
    /// Create a new LLM request
    pub fn new(request_id: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            model: model.into(),
            input: serde_json::Value::Null,
            parameters: HashMap::new(),
            user_id: None,
            session_id: None,
            metadata: HashMap::new(),
            tags: Vec::new(),
            provider: None,
            timestamp: Utc::now(),
        }
    }

    /// Set input
    pub fn input(mut self, input: serde_json::Value) -> Self {
        self.input = input;
        self
    }

    /// Set user ID
    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Set session ID
    pub fn session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Add parameter
    pub fn param(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.parameters.insert(key.into(), value);
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Add tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Set provider
    pub fn provider(mut self, provider: impl Into<String>) -> Self {
        self.provider = Some(provider.into());
        self
    }
}

/// LLM response information for logging
#[derive(Debug, Clone)]
pub struct LlmResponse {
    /// Request ID (to match with request)
    pub request_id: String,
    /// Output content
    pub output: serde_json::Value,
    /// Input tokens
    pub input_tokens: Option<u32>,
    /// Output tokens
    pub output_tokens: Option<u32>,
    /// Total cost
    pub cost: Option<f64>,
    /// Response timestamp
    pub timestamp: DateTime<Utc>,
    /// First token timestamp (for TTFT)
    pub first_token_time: Option<DateTime<Utc>>,
    /// Response metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl LlmResponse {
    /// Create a new LLM response
    pub fn new(request_id: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            output: serde_json::Value::Null,
            input_tokens: None,
            output_tokens: None,
            cost: None,
            timestamp: Utc::now(),
            first_token_time: None,
            metadata: HashMap::new(),
        }
    }

    /// Set output
    pub fn output(mut self, output: serde_json::Value) -> Self {
        self.output = output;
        self
    }

    /// Set token usage
    pub fn tokens(mut self, input: u32, output: u32) -> Self {
        self.input_tokens = Some(input);
        self.output_tokens = Some(output);
        self
    }

    /// Set cost
    pub fn cost(mut self, cost: f64) -> Self {
        self.cost = Some(cost);
        self
    }

    /// Set first token time
    pub fn first_token_time(mut self, time: DateTime<Utc>) -> Self {
        self.first_token_time = Some(time);
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// LLM error information for logging
#[derive(Debug, Clone)]
pub struct LlmError {
    /// Request ID (to match with request)
    pub request_id: String,
    /// Error message
    pub message: String,
    /// Error type/code
    pub error_type: Option<String>,
    /// Error timestamp
    pub timestamp: DateTime<Utc>,
    /// Error metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl LlmError {
    /// Create a new LLM error
    pub fn new(request_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            request_id: request_id.into(),
            message: message.into(),
            error_type: None,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        }
    }

    /// Set error type
    pub fn error_type(mut self, error_type: impl Into<String>) -> Self {
        self.error_type = Some(error_type.into());
        self
    }

    /// Add metadata
    pub fn metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

/// Active request tracking
struct ActiveRequest {
    trace_id: String,
    generation_id: String,
    request: LlmRequest,
}

/// Langfuse logger for LLM call tracing
pub struct LangfuseLogger {
    /// Batch sender
    sender: Arc<BatchSender>,
    /// Active requests
    active_requests: Arc<RwLock<HashMap<String, ActiveRequest>>>,
    /// Flush task handle
    flush_handle: Option<tokio::task::JoinHandle<()>>,
    /// Shutdown signal
    shutdown_tx: Option<mpsc::Sender<()>>,
    /// Release version
    release: Option<String>,
}

impl LangfuseLogger {
    /// Create a new Langfuse logger
    pub fn new(config: LangfuseConfig) -> Result<Self, LangfuseError> {
        let release = config.release.clone();
        let flush_interval = config.flush_interval_ms;

        let client = LangfuseClient::new(config)?;
        let sender = Arc::new(BatchSender::new(client));

        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);

        // Spawn background flush task
        let sender_clone = Arc::clone(&sender);
        let flush_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(flush_interval));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if sender_clone.pending_count() > 0
                            && let Err(e) = sender_clone.flush().await {
                                warn!("Langfuse flush error: {}", e);
                            }
                    }
                    _ = shutdown_rx.recv() => {
                        debug!("Langfuse logger shutting down");
                        // Final flush
                        if let Err(e) = sender_clone.flush().await {
                            warn!("Langfuse final flush error: {}", e);
                        }
                        break;
                    }
                }
            }
        });

        Ok(Self {
            sender,
            active_requests: Arc::new(RwLock::new(HashMap::new())),
            flush_handle: Some(flush_handle),
            shutdown_tx: Some(shutdown_tx),
            release,
        })
    }

    /// Create logger from environment variables
    pub fn from_env() -> Result<Self, LangfuseError> {
        Self::new(LangfuseConfig::from_env())
    }

    /// Callback for LLM request start
    pub fn on_llm_start(&self, request: LlmRequest) {
        let trace_id = request.request_id.clone();
        let generation_id = super::types::generate_id();

        // Create trace
        let mut trace = Trace::with_id(&trace_id)
            .name("llm-request")
            .input(request.input.clone());

        if let Some(ref user_id) = request.user_id {
            trace = trace.user_id(user_id);
        }
        if let Some(ref session_id) = request.session_id {
            trace = trace.session_id(session_id);
        }
        for tag in &request.tags {
            trace = trace.tag(tag);
        }
        for (key, value) in &request.metadata {
            trace = trace.metadata(key, value.clone());
        }
        if let Some(ref release) = self.release {
            trace.release = Some(release.clone());
        }

        // Create generation
        let mut generation = Generation::new(&trace_id)
            .name("chat-completion")
            .model(&request.model)
            .input(request.input.clone());

        generation.id = generation_id.clone();
        generation.start_time = Some(request.timestamp);

        for (key, value) in &request.parameters {
            generation = generation.model_param(key, value.clone());
        }
        if let Some(ref provider) = request.provider {
            generation = generation.metadata("provider", serde_json::json!(provider));
        }

        // Track active request
        {
            let mut active = self.active_requests.write();
            active.insert(
                request.request_id.clone(),
                ActiveRequest {
                    trace_id: trace_id.clone(),
                    generation_id,
                    request,
                },
            );
        }

        // Queue events
        let should_flush = self.sender.add(IngestionEvent::trace_create(trace));
        if should_flush {
            self.trigger_flush();
        }

        let should_flush = self
            .sender
            .add(IngestionEvent::generation_create(generation));
        if should_flush {
            self.trigger_flush();
        }

        debug!("Langfuse: Started tracking request {}", trace_id);
    }

    /// Callback for LLM request end
    pub fn on_llm_end(&self, response: LlmResponse) {
        let active_request = {
            let mut active = self.active_requests.write();
            active.remove(&response.request_id)
        };

        let Some(active) = active_request else {
            warn!(
                "Langfuse: No active request found for {}",
                response.request_id
            );
            return;
        };

        // Create generation update
        let mut generation = Generation::new(&active.trace_id)
            .output(response.output.clone())
            .end();

        generation.id = active.generation_id;
        generation.model = Some(active.request.model);
        generation.completion_start_time = response.first_token_time;

        // Set usage
        if let (Some(input), Some(output)) = (response.input_tokens, response.output_tokens) {
            let mut usage = Usage::from_tokens(input, output);
            if let Some(cost) = response.cost {
                usage.total_cost = Some(cost);
            }
            generation.usage = Some(usage);
        }

        for (key, value) in response.metadata {
            generation = generation.metadata(key, value);
        }

        // Update trace with output
        let mut trace = Trace::with_id(&active.trace_id).output(response.output);
        trace.timestamp = Some(response.timestamp);

        // Queue events
        let should_flush = self
            .sender
            .add(IngestionEvent::generation_update(generation));
        if should_flush {
            self.trigger_flush();
        }

        debug!("Langfuse: Completed request {}", active.trace_id);
    }

    /// Callback for LLM error
    pub fn on_llm_error(&self, error: LlmError) {
        let active_request = {
            let mut active = self.active_requests.write();
            active.remove(&error.request_id)
        };

        let Some(active) = active_request else {
            warn!("Langfuse: No active request found for {}", error.request_id);
            return;
        };

        // Create generation update with error
        let mut generation = Generation::new(&active.trace_id)
            .error(&error.message)
            .level(Level::Error);

        generation.id = active.generation_id;
        generation.model = Some(active.request.model);

        if let Some(ref error_type) = error.error_type {
            generation = generation.metadata("error_type", serde_json::json!(error_type));
        }
        for (key, value) in error.metadata {
            generation = generation.metadata(key, value);
        }

        // Queue events
        let should_flush = self
            .sender
            .add(IngestionEvent::generation_update(generation));
        if should_flush {
            self.trigger_flush();
        }

        error!(
            "Langfuse: Request {} failed: {}",
            active.trace_id, error.message
        );
    }

    /// Create a span for tracking sub-operations
    pub fn create_span(&self, trace_id: &str, name: &str) -> Span {
        Span::new(trace_id).name(name)
    }

    /// Log a span
    pub fn log_span(&self, span: Span) {
        let should_flush = self.sender.add(IngestionEvent::span_create(span));
        if should_flush {
            self.trigger_flush();
        }
    }

    /// Update a span
    pub fn update_span(&self, span: Span) {
        let should_flush = self.sender.add(IngestionEvent::span_update(span));
        if should_flush {
            self.trigger_flush();
        }
    }

    /// Manually flush pending events
    pub async fn flush(&self) -> Result<(), LangfuseError> {
        self.sender.flush().await?;
        Ok(())
    }

    /// Get number of pending events
    pub fn pending_count(&self) -> usize {
        self.sender.pending_count()
    }

    /// Get number of active requests
    pub fn active_count(&self) -> usize {
        self.active_requests.read().len()
    }

    /// Trigger async flush
    fn trigger_flush(&self) {
        let sender = Arc::clone(&self.sender);
        tokio::spawn(async move {
            if let Err(e) = sender.flush().await {
                warn!("Langfuse async flush error: {}", e);
            }
        });
    }

    /// Shutdown the logger gracefully
    pub async fn shutdown(mut self) {
        info!("Shutting down Langfuse logger");

        // Send shutdown signal
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(()).await;
        }

        // Wait for flush task to complete
        if let Some(handle) = self.flush_handle.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for LangfuseLogger {
    fn drop(&mut self) {
        // Note: Can't do async flush in drop, but shutdown signal helps
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.try_send(());
        }
    }
}

/// Trait for LLM callback handlers
pub trait LlmCallback: Send + Sync {
    /// Called when LLM request starts
    fn on_start(&self, request: &LlmRequest);

    /// Called when LLM request completes successfully
    fn on_end(&self, response: &LlmResponse);

    /// Called when LLM request fails
    fn on_error(&self, error: &LlmError);
}

impl LlmCallback for LangfuseLogger {
    fn on_start(&self, request: &LlmRequest) {
        self.on_llm_start(request.clone());
    }

    fn on_end(&self, response: &LlmResponse) {
        self.on_llm_end(response.clone());
    }

    fn on_error(&self, error: &LlmError) {
        self.on_llm_error(error.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> LangfuseConfig {
        LangfuseConfig {
            public_key: Some("pk-test".to_string()),
            secret_key: Some("sk-test".to_string()),
            host: "https://cloud.langfuse.com".to_string(),
            enabled: true,
            batch_size: 10,
            flush_interval_ms: 60000, // Long interval for tests
            debug: true,
            release: Some("v1.0.0".to_string()),
        }
    }

    #[test]
    fn test_llm_request_builder() {
        let request = LlmRequest::new("req-123", "gpt-4")
            .input(serde_json::json!({"messages": []}))
            .user_id("user-456")
            .session_id("session-789")
            .param("temperature", serde_json::json!(0.7))
            .metadata("custom", serde_json::json!("value"))
            .tag("production")
            .provider("openai");

        assert_eq!(request.request_id, "req-123");
        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.user_id, Some("user-456".to_string()));
        assert_eq!(request.session_id, Some("session-789".to_string()));
        assert!(request.parameters.contains_key("temperature"));
        assert!(request.metadata.contains_key("custom"));
        assert_eq!(request.tags, vec!["production"]);
        assert_eq!(request.provider, Some("openai".to_string()));
    }

    #[test]
    fn test_llm_response_builder() {
        let response = LlmResponse::new("req-123")
            .output(serde_json::json!({"content": "Hello!"}))
            .tokens(100, 50)
            .cost(0.05)
            .metadata("latency_ms", serde_json::json!(150));

        assert_eq!(response.request_id, "req-123");
        assert_eq!(response.input_tokens, Some(100));
        assert_eq!(response.output_tokens, Some(50));
        assert_eq!(response.cost, Some(0.05));
        assert!(response.metadata.contains_key("latency_ms"));
    }

    #[test]
    fn test_llm_error_builder() {
        let error = LlmError::new("req-123", "Rate limited")
            .error_type("RateLimitError")
            .metadata("retry_after", serde_json::json!(60));

        assert_eq!(error.request_id, "req-123");
        assert_eq!(error.message, "Rate limited");
        assert_eq!(error.error_type, Some("RateLimitError".to_string()));
        assert!(error.metadata.contains_key("retry_after"));
    }

    #[tokio::test]
    async fn test_logger_creation() {
        let config = test_config();
        let logger = LangfuseLogger::new(config);
        assert!(logger.is_ok());
    }

    #[tokio::test]
    async fn test_logger_on_llm_start() {
        let config = test_config();
        let logger = LangfuseLogger::new(config).unwrap();

        let request = LlmRequest::new("req-123", "gpt-4")
            .input(serde_json::json!({"messages": [{"role": "user", "content": "Hi"}]}))
            .user_id("user-456");

        logger.on_llm_start(request);

        assert_eq!(logger.active_count(), 1);
        assert!(logger.pending_count() > 0);
    }

    #[tokio::test]
    async fn test_logger_on_llm_end() {
        let config = test_config();
        let logger = LangfuseLogger::new(config).unwrap();

        let request =
            LlmRequest::new("req-123", "gpt-4").input(serde_json::json!({"messages": []}));
        logger.on_llm_start(request);

        let response = LlmResponse::new("req-123")
            .output(serde_json::json!({"content": "Hello!"}))
            .tokens(100, 50);
        logger.on_llm_end(response);

        assert_eq!(logger.active_count(), 0);
    }

    #[tokio::test]
    async fn test_logger_on_llm_error() {
        let config = test_config();
        let logger = LangfuseLogger::new(config).unwrap();

        let request =
            LlmRequest::new("req-123", "gpt-4").input(serde_json::json!({"messages": []}));
        logger.on_llm_start(request);

        let error = LlmError::new("req-123", "API error");
        logger.on_llm_error(error);

        assert_eq!(logger.active_count(), 0);
    }

    #[tokio::test]
    async fn test_logger_span() {
        let config = test_config();
        let logger = LangfuseLogger::new(config).unwrap();

        let span = logger
            .create_span("trace-123", "process-data")
            .input(serde_json::json!({"data": "test"}));

        logger.log_span(span.clone());
        assert!(logger.pending_count() > 0);

        let completed = span.end().output(serde_json::json!({"result": "done"}));
        logger.update_span(completed);
    }

    #[tokio::test]
    async fn test_logger_flush() {
        let config = test_config();
        let logger = LangfuseLogger::new(config).unwrap();

        let request =
            LlmRequest::new("req-123", "gpt-4").input(serde_json::json!({"messages": []}));
        logger.on_llm_start(request);

        let result = logger.flush().await;
        assert!(result.is_ok());
        assert_eq!(logger.pending_count(), 0);
    }

    #[tokio::test]
    async fn test_callback_trait() {
        let config = test_config();
        let logger = LangfuseLogger::new(config).unwrap();

        let callback: &dyn LlmCallback = &logger;

        let request = LlmRequest::new("req-123", "gpt-4");
        callback.on_start(&request);

        let response = LlmResponse::new("req-123");
        callback.on_end(&response);
    }
}
