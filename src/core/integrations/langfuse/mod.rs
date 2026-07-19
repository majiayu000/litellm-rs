//! Langfuse Integration
//!
//! Integration with Langfuse - an open-source LLMOps platform for tracing,
//! evaluation, and analytics of LLM applications.
//!
//! # Features
//!
//! - **Tracing**: Automatic trace creation for LLM calls
//! - **Generations**: Track model inputs, outputs, and usage
//! - **Spans**: Measure sub-operations within traces
//! - **Batching**: Efficient batch ingestion with async flush
//! - **Middleware**: Actix-web middleware for HTTP request tracing
//!
//! # Quick Start
//!
//! ## Using the Logger
//!
//! ```rust,ignore
//! use litellm_rs::core::integrations::langfuse::{LangfuseLogger, LlmRequest, LlmResponse};
//!
//! // Create logger from environment variables
//! let logger = LangfuseLogger::from_env()?;
//!
//! // Log LLM request
//! let request = LlmRequest::new("request-id", "gpt-4")
//!     .input(serde_json::json!({"messages": []}))
//!     .user_id("user-123");
//! logger.on_llm_start(request);
//!
//! // Log response
//! let response = LlmResponse::new("request-id")
//!     .output(serde_json::json!({"content": "Hello!"}))
//!     .tokens(100, 50);
//! logger.on_llm_end(response);
//! ```
//!
//! ## Using the Middleware
//!
//! ```rust,ignore
//! use litellm_rs::core::integrations::langfuse::LangfuseTracing;
//!
//! App::new()
//!     .wrap(LangfuseTracing::from_env())
//!     .route("/chat", web::post().to(chat_handler))
//! ```
//!
//! # Environment Variables
//!
//! - `LANGFUSE_PUBLIC_KEY`: Your Langfuse public key
//! - `LANGFUSE_SECRET_KEY`: Your Langfuse secret key
//! - `LANGFUSE_HOST`: Langfuse host (default: <https://cloud.langfuse.com>)
//! - `LANGFUSE_ENABLED`: Enable/disable integration (default: true)
//! - `LANGFUSE_DEBUG`: Debug mode - log instead of send (default: false)
//! - `LANGFUSE_BATCH_SIZE`: Batch size for ingestion (default: 10)
//! - `LANGFUSE_FLUSH_INTERVAL_MS`: Flush interval in ms (default: 1000)

pub mod client;
pub mod config;
pub mod integration;
pub mod logger;
#[cfg(feature = "gateway")]
pub mod middleware;
pub mod types;

// Re-export main types
pub use client::{BatchSender, LangfuseClient, LangfuseError};
pub use config::LangfuseConfig;
pub use integration::LangfuseIntegration;
pub use logger::{LangfuseLogger, LlmCallback, LlmError, LlmRequest, LlmResponse};
#[cfg(feature = "gateway")]
pub use middleware::{
    LangfuseRequestExt, LangfuseTracing, PARENT_SPAN_ID_HEADER, SESSION_ID_HEADER, TRACE_ID_HEADER,
    USER_ID_HEADER,
};
pub use types::{Generation, IngestionBatch, IngestionEvent, Level, Span, Trace, Usage};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_exports() {
        // Verify that main types are accessible
        let _config = LangfuseConfig::default();
        let trace = Trace::new().name("test");
        let _event = IngestionEvent::trace_create(trace);
        let _usage = Usage::from_tokens(100, 50);
    }

    #[test]
    fn test_full_workflow() {
        // Create a complete trace workflow
        let trace = Trace::new()
            .name("chat-completion")
            .user_id("user-123")
            .session_id("session-456")
            .tag("production");

        let generation = Generation::new(&trace.id)
            .name("gpt-4-call")
            .model("gpt-4")
            .input(serde_json::json!({"messages": [{"role": "user", "content": "Hi"}]}))
            .model_param("temperature", serde_json::json!(0.7));

        let span = Span::new(&trace.id)
            .name("preprocessing")
            .input(serde_json::json!({"data": "raw"}));

        // Create events
        let mut batch = IngestionBatch::new();
        batch.add(IngestionEvent::trace_create(trace));
        batch.add(IngestionEvent::generation_create(generation));
        batch.add(IngestionEvent::span_create(span));

        assert_eq!(batch.len(), 3);
    }

    #[tokio::test]
    async fn test_logger_workflow() {
        let config = LangfuseConfig {
            public_key: Some("pk-test".to_string()),
            secret_key: Some("sk-test".to_string()),
            debug: true,
            ..Default::default()
        };

        let logger = LangfuseLogger::new(config).unwrap();

        // Start request
        let request = LlmRequest::new("req-1", "gpt-4")
            .input(serde_json::json!({"prompt": "Hello"}))
            .user_id("user-1");
        logger.on_llm_start(request);

        assert_eq!(logger.active_count(), 1);

        // Complete request
        let response = LlmResponse::new("req-1")
            .output(serde_json::json!({"text": "World"}))
            .tokens(10, 5);
        logger.on_llm_end(response);

        assert_eq!(logger.active_count(), 0);
    }

    #[test]
    fn test_config_from_env() {
        // Just verify it doesn't panic
        let config = LangfuseConfig::from_env();
        let _ = config.is_valid();
    }

    #[cfg(feature = "gateway")]
    #[test]
    fn test_middleware_creation() {
        let config = LangfuseConfig {
            public_key: Some("pk-test".to_string()),
            secret_key: Some("sk-test".to_string()),
            debug: true,
            ..Default::default()
        };

        let middleware = LangfuseTracing::new(config)
            .service_name("test-service")
            .exclude_path("/internal");

        let _ = middleware;
    }
}
