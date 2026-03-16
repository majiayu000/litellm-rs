//! Authentication and authorization system
//!
//! This module provides comprehensive authentication and authorization functionality
//! including API key authentication, JWT tokens, RBAC, and OAuth 2.0 / SSO support.

#![allow(dead_code)]

// Public submodules
pub mod api_key;
pub mod jwt;
pub mod oauth;
pub mod rbac;

// Internal submodules
mod api_keys;
mod password;
mod system;
#[cfg(test)]
mod tests;
mod types;
mod user_management;

// Re-export commonly used types from core models
pub use crate::core::models::ApiKey;

// Re-export types from submodules
pub use system::AuthSystem;
pub use types::{AuthMethod, AuthResult};

// Re-export OAuth types for convenience
