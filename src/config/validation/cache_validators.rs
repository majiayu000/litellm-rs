//! Cache and rate limit configuration validators
//!
//! This module provides validation implementations for cache and rate limiting
//! configuration structures including CacheConfig and RateLimitConfig.

use super::trait_def::Validate;
use crate::config::models::cache::CacheConfig;
use crate::config::models::rate_limit::RateLimitConfig;

impl Validate for CacheConfig {
    fn validate(&self) -> Result<(), String> {
        if self.semantic_cache {
            return Err(
                "Semantic cache is not wired into runtime request handling; leave cache.semantic_cache=false until support lands"
                    .to_string(),
            );
        }

        Ok(())
    }
}

impl Validate for RateLimitConfig {
    fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }

        if self.effective_rpm() == 0 {
            return Err("Effective RPM must be greater than 0".to_string());
        }

        if self.default_tpm == 0 {
            return Err("Default TPM must be greater than 0".to_string());
        }

        let unimplemented = self.unimplemented_runtime_field_names();
        if !unimplemented.is_empty() {
            return Err(format!(
                "rate_limit fields [{}] are parsed but not enforced yet; leave them unset until support lands",
                unimplemented.join(", ")
            ));
        }

        Ok(())
    }
}
