//! # LiteLLM-RS
//!
//! A Rust implementation of Python LiteLLM - call 100+ LLM APIs using OpenAI format.
//! High-performance AI Gateway with unified interface for multiple providers.
//!
//! ## Features
//!
//! - **Python LiteLLM Compatible**: Drop-in replacement with same API design
//! - **OpenAI Compatible**: Full compatibility with OpenAI API format
//! - **Multi-Provider**: Support for 100+ AI providers (OpenAI, Anthropic, Azure, Google, etc.)
//! - **Unified Interface**: Call any LLM using the same function signature
//! - **High Performance**: Built with Rust and Tokio for maximum throughput
//! - **Intelligent Routing**: Smart load balancing and failover across providers
//! - **Cost Optimization**: Automatic cost tracking and provider selection
//! - **Streaming Support**: Real-time response streaming
//!
//! ## Quick Start - Python LiteLLM Style
//!
//! ```rust,no_run
//! use litellm_rs::{completion, user_message, system_message};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Call OpenAI (default provider for gpt-* models)
//!     let response = completion(
//!         "gpt-4",
//!         vec![
//!             system_message("You are a helpful assistant."),
//!             user_message("Hello, how are you?"),
//!         ],
//!         None,
//!     ).await?;
//!     
//!     if let Some(content) = &response.choices[0].message.content {
//!         println!("Response: {}", content);
//!     }
//!
//!     // Call Anthropic with explicit provider
//!     let response = completion(
//!         "anthropic/claude-3-sonnet-20240229",
//!         vec![user_message("What is the capital of France?")],
//!         None,
//!     ).await?;
//!     
//!     if let Some(content) = &response.choices[0].message.content {
//!         println!("Claude says: {}", content);
//!     }
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Gateway Mode
//!
//! Requires the `gateway` feature (enabled by default via `storage`):
//!
//! ```rust,ignore
//! use litellm_rs::{Gateway, Config};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::from_file("config/gateway.yaml").await?;
//!     let gateway = Gateway::new(config).await?;
//!     gateway.run().await?;
//!     Ok(())
//! }
//! ```

#![allow(missing_docs)]
#![warn(clippy::all)]

// Public module exports
#[cfg(feature = "gateway")]
mod auth;
// Core completion API moved to core::completion
pub mod config;
pub mod core;
#[cfg(feature = "gateway")]
mod monitoring;
pub mod sdk; // New SDK module
#[cfg(feature = "gateway")]
pub mod server;
pub mod services; // Add services module
#[cfg(feature = "gateway")]
pub mod storage;
pub mod utils;
pub mod version; // Build and version information

// Re-export main types
pub use config::Config;
pub use utils::error::gateway_error::{GatewayError, Result};
pub use version::{BuildInfo, GIT_HASH, VERSION, build_info, full_version};

// Export core completion functionality (Python LiteLLM compatible)
pub use core::completion::{
    Choice, CompletionOptions, CompletionResponse, ContentPart, LiteLLMError, Message, Router,
    Usage, acompletion, assistant_message, completion, completion_stream, system_message,
    user_message,
};

// Export core embedding functionality (Python LiteLLM compatible)
pub use core::embedding::{
    EmbeddingInput, EmbeddingOptions, EmbeddingResponse, aembedding, cosine_similarity,
    dot_product, embed_text, embed_texts, embed_texts_with_options, embedding, euclidean_distance,
    normalize,
};

// Export streaming types
pub use core::streaming::types::{
    ChatCompletionChunk, ChatCompletionChunkChoice, ChatCompletionDelta,
};

// Export unified type system
pub use core::types::message::{MessageContent, MessageRole};

// Export core functionality
pub use core::models::{RequestContext, openai::*};
pub use core::providers::{Provider, ProviderError, ProviderRegistry, ProviderType};

// Export unified router
pub use core::router::{
    CooldownReason, Deployment, DeploymentConfig, FallbackConfig, FallbackType, RouterConfig,
    RouterError, UnifiedRouter, UnifiedRoutingStrategy as RoutingStrategy,
};

#[cfg(feature = "gateway")]
use tracing::info;

/// A minimal LiteLLM Gateway implementation
#[cfg(feature = "gateway")]
pub struct Gateway {
    config: Config,
    server: server::HttpServer,
}

#[cfg(feature = "gateway")]
impl Gateway {
    /// Create a new gateway instance
    pub async fn new(config: Config) -> Result<Self> {
        info!("Creating new gateway instance");

        // Create HTTP server
        let server = server::HttpServer::new(&config).await?;

        Ok(Self { config, server })
    }

    /// Build a redacted startup summary suitable for logs.
    fn startup_summary(&self) -> String {
        let gateway = &self.config.gateway;
        format!(
            "server={{host:{}, port:{}, workers:{:?}}}, providers={}, features={{jwt_auth:{}, api_key_auth:{}, metrics:{}, tracing:{}, caching:{}, semantic_cache:{}, rate_limiting:{}, enterprise:{}}}",
            gateway.server.host,
            gateway.server.port,
            gateway.server.workers,
            gateway.providers.len(),
            gateway.auth.enable_jwt,
            gateway.auth.enable_api_key,
            gateway.monitoring.metrics.enabled,
            gateway.monitoring.tracing.enabled,
            gateway.cache.enabled,
            gateway.cache.semantic_cache,
            gateway.rate_limit.enabled,
            gateway.enterprise.enabled,
        )
    }

    /// Run the gateway server
    pub async fn run(self) -> Result<()> {
        info!("Starting LiteLLM Gateway");
        info!("Configuration: {}", self.startup_summary());

        // Start HTTP server
        self.server.start().await?;

        Ok(())
    }
}

// Version information - re-exported from version module
/// Name of the crate
pub const NAME: &str = env!("CARGO_PKG_NAME");
/// Description of the crate
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_info() {
        let info = build_info();
        assert!(!info.version.is_empty());
        assert_eq!(info.version, VERSION);
    }

    #[test]
    fn test_constants() {
        // Test that constants are defined and have expected values
        assert_eq!(VERSION, env!("CARGO_PKG_VERSION"));
        assert_eq!(NAME, env!("CARGO_PKG_NAME"));
        assert_eq!(DESCRIPTION, env!("CARGO_PKG_DESCRIPTION"));
    }
}
