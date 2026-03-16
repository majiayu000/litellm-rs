//! Configuration validation
//!
//! This module provides validation logic for all configuration structures.
//!
//! The validation is organized into several submodules:
//! - `ssrf`: SSRF protection utilities for URL validation
//! - `trait_def`: Core Validate trait definition
//! - `config_validators`: Main configuration validators (GatewayConfig, ServerConfig, ProviderConfig)
//! - `router_validators`: Router-related validators
//! - `storage_validators`: Storage-related validators
//! - `auth_validators`: Authentication-related validators
//! - `monitoring_validators`: Monitoring-related validators
//! - `cache_validators`: Cache and rate limit validators
//! - `enterprise_validators`: Enterprise configuration validators
//! - `tests`: Test suite for all validators

mod auth_validators;
mod cache_validators;
mod config_validators;
mod enterprise_validators;
mod monitoring_validators;
mod router_validators;
mod ssrf;
mod storage_validators;
#[cfg(test)]
mod tests;
mod trait_def;

// Re-export the Validate trait for backward compatibility
pub use trait_def::Validate;

// Re-export SSRF validation function if needed externally
pub(crate) use ssrf::is_private_or_internal_ip;
pub use ssrf::validate_url_against_ssrf;
