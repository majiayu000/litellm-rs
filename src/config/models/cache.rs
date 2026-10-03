//! Cache configuration

use super::*;
use serde::{Deserialize, Serialize};

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CacheConfig {
    /// Enable caching
    #[serde(default)]
    pub enabled: bool,
    /// Cache TTL in seconds
    #[serde(default = "default_cache_ttl")]
    pub ttl: u64,
    /// Maximum cache size
    #[serde(default = "default_cache_max_size")]
    pub max_size: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ttl: default_cache_ttl(),
            max_size: default_cache_max_size(),
        }
    }
}

impl CacheConfig {
    /// Merge cache configurations
    pub fn merge(mut self, other: Self) -> Self {
        self.enabled = other.enabled;
        if other.ttl != default_cache_ttl() {
            self.ttl = other.ttl;
        }
        if other.max_size != default_cache_max_size() {
            self.max_size = other.max_size;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_config_default() {
        let config = CacheConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.ttl, 3600);
        assert_eq!(config.max_size, 1000);
    }

    #[test]
    fn test_cache_config_structure() {
        let config = CacheConfig {
            enabled: true,
            ttl: 7200,
            max_size: 5000,
        };
        assert!(config.enabled);
        assert_eq!(config.ttl, 7200);
    }

    #[test]
    fn test_cache_config_serialization() {
        let config = CacheConfig {
            enabled: true,
            ttl: 1800,
            max_size: 2000,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["enabled"], true);
        assert_eq!(json["ttl"], 1800);
        assert_eq!(json["max_size"], 2000);
    }

    #[test]
    fn test_cache_config_deserialization() {
        let json = r#"{"enabled": true, "ttl": 900, "max_size": 500}"#;
        let config: CacheConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled);
        assert_eq!(config.ttl, 900);
    }

    #[test]
    fn test_cache_config_merge_enabled() {
        let base = CacheConfig::default();
        let other = CacheConfig {
            enabled: true,
            ttl: 3600,
            max_size: 1000,
        };
        let merged = base.merge(other);
        assert!(merged.enabled);
    }

    #[test]
    fn test_cache_config_merge_ttl() {
        let base = CacheConfig::default();
        let other = CacheConfig {
            enabled: false,
            ttl: 1800,
            max_size: 1000,
        };
        let merged = base.merge(other);
        assert_eq!(merged.ttl, 1800);
    }

    #[test]
    fn test_cache_config_merge_disable_enabled() {
        let base = CacheConfig {
            enabled: true,
            ..CacheConfig::default()
        };
        let other = CacheConfig {
            enabled: false,
            ..CacheConfig::default()
        };
        let merged = base.merge(other);
        assert!(!merged.enabled);
    }

    #[test]
    fn test_cache_config_clone() {
        let config = CacheConfig {
            enabled: true,
            ttl: 3600,
            max_size: 2000,
        };
        let cloned = config.clone();
        assert_eq!(config.enabled, cloned.enabled);
        assert_eq!(config.ttl, cloned.ttl);
    }
}
