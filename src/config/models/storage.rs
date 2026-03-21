//! Storage configuration

use super::file_storage::VectorDbConfig;
use super::*;
use super::{default_connection_timeout, default_redis_max_connections};
use serde::{Deserialize, Serialize};

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StorageConfig {
    /// Database configuration
    pub database: DatabaseConfig,
    /// Redis configuration
    pub redis: RedisConfig,
    /// Vector database configuration (optional)
    #[serde(default)]
    pub vector_db: Option<VectorDbConfig>,
}

impl StorageConfig {
    /// Merge storage configurations
    pub fn merge(mut self, other: Self) -> Self {
        self.database = self.database.merge(other.database);
        self.redis = self.redis.merge(other.redis);
        if other.vector_db.is_some() {
            self.vector_db = other.vector_db;
        }
        self
    }
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database URL
    pub url: String,
    /// Maximum connections
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    /// Enable SSL
    #[serde(default)]
    pub ssl: bool,
    /// Enable database (if false, use in-memory storage)
    #[serde(default)]
    pub enabled: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost/litellm".to_string(),
            max_connections: default_max_connections(),
            connection_timeout: default_connection_timeout(),
            ssl: false,
            enabled: false,
        }
    }
}

impl DatabaseConfig {
    /// Merge database configurations
    pub fn merge(mut self, other: Self) -> Self {
        let default = Self::default();
        if !other.url.is_empty() && other.url != default.url {
            self.url = other.url;
        }
        if other.max_connections != default_max_connections() {
            self.max_connections = other.max_connections;
        }
        if other.connection_timeout != default_connection_timeout() {
            self.connection_timeout = other.connection_timeout;
        }
        if other.ssl {
            self.ssl = true;
        }
        if other.enabled {
            self.enabled = true;
        }
        self
    }
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis URL
    pub url: String,
    /// Enable Redis (if false, use in-memory cache)
    #[serde(default = "default_redis_enabled")]
    pub enabled: bool,
    /// Maximum connections
    #[serde(default = "default_redis_max_connections")]
    pub max_connections: u32,
    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    /// Enable cluster mode
    #[serde(default)]
    pub cluster: bool,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            enabled: default_redis_enabled(),
            max_connections: default_redis_max_connections(),
            connection_timeout: default_connection_timeout(),
            cluster: false,
        }
    }
}

impl RedisConfig {
    /// Merge Redis configurations
    pub fn merge(mut self, other: Self) -> Self {
        let default = Self::default();
        if !other.url.is_empty() && other.url != default.url {
            self.url = other.url;
        }
        if other.max_connections != default_redis_max_connections() {
            self.max_connections = other.max_connections;
        }
        if other.connection_timeout != default_connection_timeout() {
            self.connection_timeout = other.connection_timeout;
        }
        if other.cluster {
            self.cluster = true;
        }
        // Redis defaults to enabled=false; propagate if other differs from default
        if other.enabled != default_redis_enabled() {
            self.enabled = other.enabled;
        }
        self
    }
}

fn default_redis_enabled() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== DatabaseConfig Tests ====================

    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();
        assert_eq!(config.url, "postgresql://localhost/litellm");
        assert_eq!(config.max_connections, 10);
        assert_eq!(config.connection_timeout, 5);
        assert!(!config.ssl);
        assert!(!config.enabled);
    }

    #[test]
    fn test_database_config_structure() {
        let config = DatabaseConfig {
            url: "postgresql://user:pass@host/db".to_string(),
            max_connections: 20,
            connection_timeout: 60,
            ssl: true,
            enabled: true,
        };
        assert!(config.ssl);
        assert!(config.enabled);
        assert_eq!(config.max_connections, 20);
    }

    #[test]
    fn test_database_config_serialization() {
        let config = DatabaseConfig {
            url: "postgresql://test".to_string(),
            max_connections: 15,
            connection_timeout: 45,
            ssl: true,
            enabled: true,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["url"], "postgresql://test");
        assert_eq!(json["max_connections"], 15);
        assert_eq!(json["ssl"], true);
    }

    #[test]
    fn test_database_config_deserialization() {
        let json = r#"{"url": "postgresql://prod/app", "max_connections": 50, "connection_timeout": 120, "ssl": true, "enabled": true}"#;
        let config: DatabaseConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.url, "postgresql://prod/app");
        assert!(config.ssl);
    }

    #[test]
    fn test_database_config_merge_url() {
        let base = DatabaseConfig::default();
        let other = DatabaseConfig {
            url: "postgresql://new-host/new-db".to_string(),
            max_connections: 10,
            connection_timeout: 30,
            ssl: false,
            enabled: false,
        };
        let merged = base.merge(other);
        assert_eq!(merged.url, "postgresql://new-host/new-db");
    }

    #[test]
    fn test_database_config_merge_ssl() {
        let base = DatabaseConfig::default();
        let other = DatabaseConfig {
            url: "postgresql://localhost/litellm".to_string(),
            max_connections: 10,
            connection_timeout: 30,
            ssl: true,
            enabled: false,
        };
        let merged = base.merge(other);
        assert!(merged.ssl);
    }

    #[test]
    fn test_database_config_merge_enabled_true() {
        let base = DatabaseConfig::default();
        let other = DatabaseConfig {
            url: "postgresql://localhost/litellm".to_string(),
            max_connections: default_max_connections(),
            connection_timeout: default_connection_timeout(),
            ssl: false,
            enabled: true,
        };
        let merged = base.merge(other);
        assert!(merged.enabled);
    }

    #[test]
    fn test_database_config_merge_preserves_base_when_other_is_default_url() {
        let base = DatabaseConfig {
            url: "postgresql://custom-host/mydb".to_string(),
            ..DatabaseConfig::default()
        };
        // other has the default URL — should not override base
        let other = DatabaseConfig::default();
        let merged = base.merge(other);
        assert_eq!(merged.url, "postgresql://custom-host/mydb");
    }

    #[test]
    fn test_database_config_merge_preserves_base_when_other_url_is_empty() {
        let base = DatabaseConfig {
            url: "postgresql://custom-host/mydb".to_string(),
            ..DatabaseConfig::default()
        };
        let other = DatabaseConfig {
            url: "".to_string(),
            ..DatabaseConfig::default()
        };
        let merged = base.merge(other);
        assert_eq!(merged.url, "postgresql://custom-host/mydb");
    }

    #[test]
    fn test_database_config_merge_both_custom_urls_takes_other() {
        let base = DatabaseConfig {
            url: "postgresql://base-host/basedb".to_string(),
            ..DatabaseConfig::default()
        };
        let other = DatabaseConfig {
            url: "postgresql://other-host/otherdb".to_string(),
            ..DatabaseConfig::default()
        };
        let merged = base.merge(other);
        assert_eq!(merged.url, "postgresql://other-host/otherdb");
    }

    #[test]
    fn test_database_config_clone() {
        let config = DatabaseConfig::default();
        let cloned = config.clone();
        assert_eq!(config.url, cloned.url);
        assert_eq!(config.max_connections, cloned.max_connections);
    }

    // ==================== RedisConfig Tests ====================

    #[test]
    fn test_redis_config_default() {
        let config = RedisConfig::default();
        assert_eq!(config.url, "redis://localhost:6379");
        assert!(!config.enabled);
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.connection_timeout, 5);
        assert!(!config.cluster);
    }

    #[test]
    fn test_redis_config_structure() {
        let config = RedisConfig {
            url: "redis://redis-cluster:6379".to_string(),
            enabled: true,
            max_connections: 200,
            connection_timeout: 60,
            cluster: true,
        };
        assert!(config.cluster);
        assert_eq!(config.max_connections, 200);
    }

    #[test]
    fn test_redis_config_serialization() {
        let config = RedisConfig {
            url: "redis://cache:6379".to_string(),
            enabled: true,
            max_connections: 50,
            connection_timeout: 15,
            cluster: false,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["url"], "redis://cache:6379");
        assert_eq!(json["max_connections"], 50);
    }

    #[test]
    fn test_redis_config_deserialization() {
        let json = r#"{"url": "redis://prod:6379", "enabled": true, "max_connections": 150, "connection_timeout": 20, "cluster": true}"#;
        let config: RedisConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.url, "redis://prod:6379");
        assert!(config.cluster);
    }

    #[test]
    fn test_redis_config_merge_url() {
        let base = RedisConfig::default();
        let other = RedisConfig {
            url: "redis://new-redis:6379".to_string(),
            enabled: true,
            max_connections: 100,
            connection_timeout: 30,
            cluster: false,
        };
        let merged = base.merge(other);
        assert_eq!(merged.url, "redis://new-redis:6379");
    }

    #[test]
    fn test_redis_config_merge_cluster() {
        let base = RedisConfig::default();
        let other = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            enabled: true,
            max_connections: 100,
            connection_timeout: 30,
            cluster: true,
        };
        let merged = base.merge(other);
        assert!(merged.cluster);
    }

    #[test]
    fn test_redis_config_merge_enabled_true() {
        let base = RedisConfig::default();
        let other = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            enabled: true,
            max_connections: default_redis_max_connections(),
            connection_timeout: default_connection_timeout(),
            cluster: false,
        };
        let merged = base.merge(other);
        assert!(merged.enabled);
    }

    #[test]
    fn test_redis_config_merge_preserves_base_when_other_is_default_url() {
        let base = RedisConfig {
            url: "redis://custom-host:6379".to_string(),
            ..RedisConfig::default()
        };
        // other has the default URL — should not override base
        let other = RedisConfig::default();
        let merged = base.merge(other);
        assert_eq!(merged.url, "redis://custom-host:6379");
    }

    #[test]
    fn test_redis_config_merge_preserves_base_when_other_url_is_empty() {
        let base = RedisConfig {
            url: "redis://custom-host:6379".to_string(),
            ..RedisConfig::default()
        };
        let other = RedisConfig {
            url: "".to_string(),
            ..RedisConfig::default()
        };
        let merged = base.merge(other);
        assert_eq!(merged.url, "redis://custom-host:6379");
    }

    #[test]
    fn test_redis_config_merge_both_custom_urls_takes_other() {
        let base = RedisConfig {
            url: "redis://base-host:6379".to_string(),
            ..RedisConfig::default()
        };
        let other = RedisConfig {
            url: "redis://other-host:6380".to_string(),
            ..RedisConfig::default()
        };
        let merged = base.merge(other);
        assert_eq!(merged.url, "redis://other-host:6380");
    }

    #[test]
    fn test_redis_config_clone() {
        let config = RedisConfig::default();
        let cloned = config.clone();
        assert_eq!(config.url, cloned.url);
        assert_eq!(config.enabled, cloned.enabled);
    }

    // ==================== StorageConfig Tests ====================

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();
        assert_eq!(config.database.url, "postgresql://localhost/litellm");
        assert_eq!(config.redis.url, "redis://localhost:6379");
        assert!(config.vector_db.is_none());
    }

    #[test]
    fn test_storage_config_structure() {
        let config = StorageConfig {
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            vector_db: None,
        };
        assert!(config.vector_db.is_none());
    }

    #[test]
    fn test_storage_config_serialization() {
        let config = StorageConfig::default();
        let json = serde_json::to_value(&config).unwrap();
        assert!(json["database"].is_object());
        assert!(json["redis"].is_object());
    }

    #[test]
    fn test_storage_config_merge() {
        let base = StorageConfig::default();
        let other = StorageConfig {
            database: DatabaseConfig {
                url: "postgresql://new/db".to_string(),
                max_connections: 10,
                connection_timeout: 30,
                ssl: false,
                enabled: false,
            },
            redis: RedisConfig::default(),
            vector_db: None,
        };
        let merged = base.merge(other);
        assert_eq!(merged.database.url, "postgresql://new/db");
    }

    #[test]
    fn test_storage_config_clone() {
        let config = StorageConfig::default();
        let cloned = config.clone();
        assert_eq!(config.database.url, cloned.database.url);
        assert_eq!(config.redis.url, cloned.redis.url);
    }
}
