//! External Integrations
//!
//! This module provides integrations with external LLMOps platforms and services.
//!
//! # Architecture
//!
//! All integrations implement the `Integration` trait from `crate::core::traits::integration`.
//! The `IntegrationManager` handles registration and dispatching of events to all integrations.
//!
//! # Available Integrations
//!
//! ## Langfuse
//!
//! Open-source LLMOps platform for tracing, evaluation, and analytics.
//!
//! ```rust,ignore
//! use litellm_rs::core::integrations::langfuse::{LangfuseLogger, LlmRequest};
//!
//! let logger = LangfuseLogger::from_env()?;
//! logger.on_llm_start(LlmRequest::new("req-id", "gpt-4"));
//! ```
//!
//! See the [`langfuse`] module for detailed documentation.
//!
//! ## Observability
//!
//! Prometheus and OpenTelemetry integrations for metrics and tracing.
//!
//! ```rust,ignore
//! use litellm_rs::core::integrations::observability::{PrometheusIntegration, OpenTelemetryIntegration};
//!
//! let prometheus = PrometheusIntegration::with_defaults();
//! let otel = OpenTelemetryIntegration::with_defaults();
//! ```
//!
//! # Usage with IntegrationManager
//!
//! ```rust,ignore
//! use litellm_rs::core::integrations::{IntegrationManager, IntegrationManagerConfig};
//! use litellm_rs::core::traits::integration::LlmStartEvent;
//!
//! let manager = IntegrationManager::with_defaults();
//! manager.register(my_integration).await;
//!
//! // Dispatch events to all integrations
//! let event = LlmStartEvent::new("req-1", "gpt-4");
//! manager.on_llm_start(&event).await?;
//! ```

pub mod callback_runtime;
pub mod langfuse;
pub mod manager;
pub mod observability;

// Re-export commonly used types
#[cfg(feature = "gateway")]
pub use langfuse::LangfuseTracing;
pub use langfuse::{
    LangfuseConfig, LangfuseIntegration, LangfuseLogger, LlmCallback, LlmError, LlmRequest,
    LlmResponse,
};
pub use manager::{IntegrationManager, IntegrationManagerConfig};
pub use observability::{
    ArizeConfig, ArizeIntegration, DataDogConfig, DataDogIntegration, HeliconeConfig,
    HeliconeIntegration, OpenTelemetryConfig, OpenTelemetryIntegration, PrometheusConfig,
    PrometheusIntegration,
};

// Re-export trait types for convenience
pub use crate::core::traits::integration::{
    BoxedIntegration, CacheHitEvent, EmbeddingEndEvent, EmbeddingErrorEvent, EmbeddingStartEvent,
    Integration, IntegrationError, IntegrationResult, LlmEndEvent, LlmErrorEvent, LlmStartEvent,
    LlmStreamEvent,
};
pub use callback_runtime::{
    CallbackDispatchError, CallbackDispatcher, CallbackRuntime, CallbackTerminalPermit,
};
