//! Guardrails Framework
//!
//! A decoupled content safety and validation system for AI Gateway.
//!
//! # Features
//!
//! - **OpenAI Moderation**: Integration with OpenAI's content moderation API
//! - **PII Detection**: Detect and mask personally identifiable information
//! - **Prompt Injection**: Detect potential prompt injection attacks
//! - **Custom Rules**: Define custom guardrail rules
//! - **Middleware**: Actix-web middleware for request/response filtering
//!
//! # Architecture
//!
//! The guardrails system is designed with the following principles:
//! - **Decoupled**: Each guardrail is independent and can be enabled/disabled
//! - **Extensible**: Easy to add new guardrail types via the `Guardrail` trait
//! - **Async**: All operations are non-blocking
//! - **Configurable**: Fine-grained control over behavior
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use litellm_rs::core::guardrails::{GuardrailEngine, GuardrailConfig};
//!
//! let config = GuardrailConfig::default()
//!     .enable_openai_moderation(true)
//!     .enable_pii_detection(true);
//!
//! let engine = GuardrailEngine::new(config).await?;
//!
//! // Check content
//! let result = engine.check_input("Hello, world!").await?;
//! if result.is_blocked() {
//!     println!("Content blocked: {:?}", result.reasons());
//! }
//! ```

pub mod config;
pub mod custom_rules;
pub mod engine;
#[cfg(feature = "gateway")]
pub mod middleware;
pub mod openai_moderation;
pub mod pii;
pub mod prompt_injection;
pub mod traits;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export main types
pub use config::{GuardrailConfig, OpenAIModerationConfig, PIIConfig, PromptInjectionConfig};
pub use engine::GuardrailEngine;
#[cfg(feature = "gateway")]
pub use middleware::{GuardrailCheckContext, GuardrailMiddleware, GuardrailMiddlewareService};
pub use openai_moderation::OpenAIModerationGuardrail;
pub use pii::PIIGuardrail;
pub use prompt_injection::PromptInjectionGuardrail;
pub use traits::Guardrail;
pub use types::{
    CheckResult, GuardrailAction, GuardrailError, GuardrailResult, ModerationCategory,
    ModerationResult, PIIMatch, ViolationType,
};
