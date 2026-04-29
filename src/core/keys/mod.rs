//! API Key Management System
//!
//! This module provides comprehensive API key management functionality including:
//! - Key generation with secure hashing
//! - Key validation and verification
//! - Key rotation and revocation
//! - Permission and rate limit management
//! - Usage tracking and statistics

#[cfg(feature = "gateway")]
mod database_repository;
#[cfg(feature = "gateway")]
mod db_mapping;
#[cfg(feature = "gateway")]
mod db_update;
mod manager;
mod repository;
mod types;

#[cfg(all(test, feature = "gateway"))]
mod database_repository_tests;
#[cfg(test)]
mod tests;

// Re-export public types
#[cfg(feature = "gateway")]
pub use database_repository::DatabaseKeyRepository;
pub use manager::KeyManager;
pub use repository::{InMemoryKeyRepository, KeyRepository};
pub use types::{
    CreateKeyConfig, KeyInfo, KeyPermissions, KeyRateLimits, KeyStatus, KeyUsageStats,
    ManagedApiKey, UpdateKeyConfig, VerifyKeyResult,
};
