//! HTTP server implementation
//!
//! This module provides the HTTP server and routing functionality.

// Submodules
pub mod middleware;
pub mod routes;

// New modular server components
pub mod builder;
mod callbacks;
mod guardrails;
pub mod http;
mod http_listener;
pub mod state;
pub(crate) mod tls;
#[cfg(test)]
mod tls_listener_tests;
pub mod types;
mod utils;

pub use http::HttpServer;

#[cfg(test)]
pub(crate) fn valid_test_config() -> crate::config::Config {
    let mut config = crate::config::Config::default();
    config
        .gateway
        .providers
        .push(crate::config::models::provider::ProviderConfig {
            name: "test-openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test".to_string(),
            ..Default::default()
        });
    config
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod http_capacity_tests;
