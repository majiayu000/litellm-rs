//! Virtual Keys management system
//!
//! This module provides comprehensive virtual key management for the LiteLLM proxy.

mod manager;
mod requests;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public types
#[deprecated(
    since = "0.6.0",
    note = "use core::keys::KeyManager; the duplicate VirtualKeyManager is scheduled for removal in 0.7.0"
)]
pub use manager::VirtualKeyManager;
pub use requests::{CreateKeyRequest, UpdateKeyRequest};
pub use types::{KeyGenerationSettings, Permission, RateLimitState, RateLimits, VirtualKey};

/// Canonical virtual-key runtime used by the gateway.
pub type RuntimeVirtualKeyManager = crate::core::keys::KeyManager;
