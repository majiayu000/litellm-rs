//! Unified LLM Provider SDK
//!
//! This module provides a simplified, unified interface for interacting with multiple LLM providers.
//! It's built on top of the existing litellm-rs infrastructure but provides a more user-friendly API.

pub mod client;
pub mod config;
pub mod errors;
pub mod types;

// Re-exports for convenience
pub use client::{LLMClient, LoadBalancer, LoadBalancingStrategy, ProviderStats};
pub use config::{ClientConfig, ConfigBuilder, SdkConfigBuilder};
pub use errors::{Result, SDKError};

/// SDK version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Initialize the SDK with default logging
pub fn init() {
    #[cfg(feature = "tracing")]
    {
        tracing_subscriber::fmt::init();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        // VERSION is always non-empty as it's from env!("CARGO_PKG_VERSION")
        // Clippy warns about const expressions, but we want to test this
        #[allow(clippy::const_is_empty)]
        {
            assert!(!VERSION.is_empty());
        }
        assert!(VERSION.contains('.'));
    }
}
