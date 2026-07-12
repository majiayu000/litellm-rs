//! Default value functions for configuration
//!
//! This module provides shared default functions used across multiple config models.
//! Each function here MUST be referenced by at least one model struct's `#[serde(default)]`
//! annotation. Model-specific defaults live in their respective model files.

pub fn default_true() -> bool {
    true
}

pub fn default_max_retries() -> u32 {
    3
}

pub fn default_health_check_interval() -> u64 {
    30
}

pub fn default_failure_threshold() -> u32 {
    5
}

pub fn default_recovery_timeout() -> u64 {
    60
}

pub fn default_health_expected_codes() -> Vec<u16> {
    vec![200]
}

pub fn default_initial_delay_ms() -> u64 {
    100
}

pub fn default_max_delay_ms() -> u64 {
    30000
}

pub fn default_jwt_expiration() -> u64 {
    86400 // 24 hours
}

pub fn default_api_key_header() -> String {
    "Authorization".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_true() {
        assert!(default_true());
    }

    #[test]
    fn test_default_max_retries() {
        assert_eq!(default_max_retries(), 3);
    }

    #[test]
    fn test_default_health_check_interval() {
        assert_eq!(default_health_check_interval(), 30);
    }

    #[test]
    fn test_default_failure_threshold() {
        assert_eq!(default_failure_threshold(), 5);
    }

    #[test]
    fn test_default_recovery_timeout() {
        assert_eq!(default_recovery_timeout(), 60);
    }

    #[test]
    fn test_default_health_expected_codes() {
        assert_eq!(default_health_expected_codes(), vec![200]);
    }

    #[test]
    fn test_default_initial_delay_ms() {
        assert_eq!(default_initial_delay_ms(), 100);
    }

    #[test]
    fn test_default_max_delay_ms() {
        assert_eq!(default_max_delay_ms(), 30000);
    }

    #[test]
    fn test_default_jwt_expiration() {
        assert_eq!(default_jwt_expiration(), 86400);
    }

    #[test]
    fn test_default_api_key_header() {
        assert_eq!(default_api_key_header(), "Authorization");
    }

    #[test]
    fn test_retries_is_reasonable() {
        let retries = default_max_retries();
        assert!(retries > 0);
        assert!(retries <= 10);
    }

    #[test]
    fn test_jwt_algorithm_header_is_valid() {
        let header = default_api_key_header();
        assert!(!header.is_empty());
    }
}
