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
pub mod state;
mod tls;
pub mod types;
mod utils;

pub use http::HttpServer;

#[cfg(test)]
mod tests;
