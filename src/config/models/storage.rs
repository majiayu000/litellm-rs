//! Storage configuration

use super::file_storage::{FileStorageConfig, VectorDbConfig};
use super::*;
use super::{default_connection_timeout, default_redis_max_connections};
use serde::{Deserialize, Serialize};

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    /// Database configuration
    pub database: DatabaseConfig,
    /// Redis configuration
    pub redis: RedisConfig,
    /// File storage configuration
    #[serde(default, alias = "file_storage")]
    pub files: FileStorageConfig,
    /// Vector database configuration (optional)
    #[serde(default)]
    pub vector_db: Option<VectorDbConfig>,
}

impl StorageConfig {
    /// Merge storage configurations
    pub fn merge(mut self, other: Self) -> Self {
        self.database = self.database.merge(other.database);
        self.redis = self.redis.merge(other.redis);
        self.files = self.files.merge(other.files);
        if other.vector_db.is_some() {
            self.vector_db = other.vector_db;
        }
        self
    }
}

/// Database configuration
#[derive(Debug, Clone, Serialize)]
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
    /// Run database migrations automatically during storage startup.
    ///
    /// Disabled by default for configured databases so production runtime
    /// users do not need DDL privileges. The storage layer still migrates the
    /// non-persistent in-memory SQLite fallback used when `enabled=false`.
    #[serde(default)]
    pub auto_migrate: bool,
    /// Tracks whether `auto_migrate` was present in deserialized config.
    ///
    /// This preserves merge semantics for layered config: an omitted field
    /// should not be treated as an explicit `false` override.
    #[doc(hidden)]
    #[serde(skip)]
    pub auto_migrate_configured: bool,
    /// Allow PostgreSQL connection failures to fall back to local SQLite.
    ///
    /// Disabled by default to avoid silently writing production data to a
    /// local database when the configured PostgreSQL backend is unavailable.
    #[serde(default)]
    pub fallback_to_sqlite: bool,
    /// When `enabled` is true and budget-persistence snapshot loading fails,
    /// allow the gateway to keep running with in-memory budget tracking
    /// instead of failing startup.
    ///
    /// Defaults to `false` so a configured-but-broken budget persistence path
    /// is surfaced at startup. A `true` value documents that operating with
    /// in-memory budgets only is intentional.
    #[serde(default)]
    pub allow_degraded: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DatabaseConfigFields {
    pub url: String,
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    #[serde(default)]
    pub ssl: bool,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub auto_migrate: Option<bool>,
    #[serde(default)]
    pub fallback_to_sqlite: bool,
    #[serde(default)]
    pub allow_degraded: bool,
}

impl<'de> Deserialize<'de> for DatabaseConfig {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let fields = DatabaseConfigFields::deserialize(deserializer)?;
        let auto_migrate_configured = fields.auto_migrate.is_some();

        Ok(Self {
            url: fields.url,
            max_connections: fields.max_connections,
            connection_timeout: fields.connection_timeout,
            ssl: fields.ssl,
            enabled: fields.enabled,
            auto_migrate: fields.auto_migrate.unwrap_or(false),
            auto_migrate_configured,
            fallback_to_sqlite: fields.fallback_to_sqlite,
            allow_degraded: fields.allow_degraded,
        })
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "postgresql://localhost/litellm".to_string(),
            max_connections: default_max_connections(),
            connection_timeout: default_connection_timeout(),
            ssl: false,
            enabled: false,
            auto_migrate: false,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
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
        if other.auto_migrate_configured || other.auto_migrate {
            self.auto_migrate = other.auto_migrate;
            self.auto_migrate_configured = other.auto_migrate_configured;
        }
        if other.fallback_to_sqlite {
            self.fallback_to_sqlite = true;
        }
        if other.allow_degraded {
            self.allow_degraded = true;
        }
        self
    }
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// When `enabled` is true and Redis init fails, allow the gateway to keep
    /// running with an in-process/no-op fallback instead of failing startup.
    ///
    /// Defaults to `false` so an explicitly enabled-but-unreachable Redis is
    /// surfaced at startup. Set to `true` only when running in environments
    /// where caching is best-effort and silent degradation is acceptable.
    #[serde(default)]
    pub allow_degraded: bool,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            enabled: default_redis_enabled(),
            max_connections: default_redis_max_connections(),
            connection_timeout: default_connection_timeout(),
            cluster: false,
            allow_degraded: false,
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
        if other.allow_degraded {
            self.allow_degraded = true;
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
        assert!(!config.auto_migrate);
        assert!(!config.fallback_to_sqlite);
    }

    #[test]
    fn test_database_config_structure() {
        let config = DatabaseConfig {
            url: "postgresql://user:pass@host/db".to_string(),
            max_connections: 20,
            connection_timeout: 60,
            ssl: true,
            enabled: true,
            auto_migrate: true,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        };
        assert!(config.ssl);
        assert!(config.enabled);
        assert!(config.auto_migrate);
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
            auto_migrate: true,
            auto_migrate_configured: false,
            fallback_to_sqlite: false,
            allow_degraded: false,
        };
        let json = serde_json::to_value(&config).unwrap();
        assert_eq!(json["url"], "postgresql://test");
        assert_eq!(json["max_connections"], 15);
        assert_eq!(json["ssl"], true);
        assert_eq!(json["auto_migrate"], true);
    }

    #[test]
    fn test_database_config_deserialization() {
        let json = r#"{"url": "postgresql://prod/app", "max_connections": 50, "connection_timeout": 120, "ssl": true, "enabled": true}"#;
        let config: DatabaseConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.url, "postgresql://prod/app");
        assert!(config.ssl);
        assert!(!config.auto_migrate);
        assert!(!config.auto_migrate_configured);
    }

    #[test]
    fn test_database_config_merge_url() {
        let base = DatabaseConfig::default();
        let other = DatabaseConfig {
            url: "postgresql://new-host/new-db".to_string(),
            connection_timeout: 30,
            ..DatabaseConfig::default()
        };
        let merged = base.merge(other);
        assert_eq!(merged.url, "postgresql://new-host/new-db");
    }

    #[test]
    fn test_database_config_merge_ssl() {
        let base = DatabaseConfig::default();
        let other = DatabaseConfig {
            connection_timeout: 30,
            ssl: true,
            ..DatabaseConfig::default()
        };
        let merged = base.merge(other);
        assert!(merged.ssl);
    }

    #[test]
    fn test_database_config_merge_enabled_true() {
        let base = DatabaseConfig::default();
        let other = DatabaseConfig {
            enabled: true,
            ..DatabaseConfig::default()
        };
        let merged = base.merge(other);
        assert!(merged.enabled);
    }

    #[test]
    fn test_database_config_merge_auto_migrate_true() {
        let base = DatabaseConfig::default();
        let other = DatabaseConfig {
            auto_migrate: true,
            auto_migrate_configured: false,
            ..DatabaseConfig::default()
        };
        let merged = base.merge(other);
        assert!(merged.auto_migrate);
    }

    #[test]
    fn test_database_config_merge_auto_migrate_false_overrides_base() {
        let base = DatabaseConfig {
            auto_migrate: true,
            auto_migrate_configured: false,
            ..DatabaseConfig::default()
        };
        let other: DatabaseConfig = match serde_yml::from_str(
            r#"url: "postgresql://localhost/litellm"
auto_migrate: false
"#,
        ) {
            Ok(config) => config,
            Err(error) => panic!("explicit auto_migrate=false config should parse: {}", error),
        };
        assert!(other.auto_migrate_configured);

        let merged = base.merge(other);
        assert!(!merged.auto_migrate);
    }

    #[test]
    fn test_database_config_merge_preserves_auto_migrate_when_overlay_omits_field() {
        let base = DatabaseConfig {
            auto_migrate: true,
            auto_migrate_configured: false,
            ..DatabaseConfig::default()
        };
        let other: DatabaseConfig =
            match serde_yml::from_str(r#"url: "postgresql://localhost/litellm""#) {
                Ok(config) => config,
                Err(error) => panic!("omitted auto_migrate config should parse: {}", error),
            };
        assert!(!other.auto_migrate_configured);

        let merged = base.merge(other);
        assert!(merged.auto_migrate);
    }

    #[test]
    fn test_database_config_merge_fallback_to_sqlite_true() {
        let base = DatabaseConfig::default();
        let other = DatabaseConfig {
            fallback_to_sqlite: true,
            ..DatabaseConfig::default()
        };
        let merged = base.merge(other);
        assert!(merged.fallback_to_sqlite);
    }

    #[test]
    fn test_database_config_merge_preserves_fallback_to_sqlite_on_default_overlay() {
        let base = DatabaseConfig {
            fallback_to_sqlite: true,
            ..DatabaseConfig::default()
        };
        let merged = base.merge(DatabaseConfig::default());
        assert!(merged.fallback_to_sqlite);
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
            allow_degraded: false,
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
            allow_degraded: false,
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
            allow_degraded: false,
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
            allow_degraded: false,
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
            allow_degraded: false,
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
        assert_eq!(config.files.storage_type, "local");
        assert!(config.vector_db.is_none());
    }

    #[test]
    fn test_storage_config_structure() {
        let config = StorageConfig {
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            files: FileStorageConfig::default(),
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
        assert!(json["files"].is_object());
    }

    #[test]
    fn test_storage_config_deserializes_file_storage_alias() {
        let json = serde_json::json!({
            "database": DatabaseConfig::default(),
            "redis": RedisConfig::default(),
            "file_storage": {
                "local_path": "/configured/files"
            }
        });

        let config: StorageConfig = serde_json::from_value(json).unwrap();

        assert_eq!(
            config.files.local_path,
            Some("/configured/files".to_string())
        );
        assert_eq!(config.files.storage_type, "local");
    }

    #[test]
    fn test_storage_config_merge() {
        let base = StorageConfig::default();
        let other = StorageConfig {
            database: DatabaseConfig {
                url: "postgresql://new/db".to_string(),
                connection_timeout: 30,
                ..DatabaseConfig::default()
            },
            redis: RedisConfig::default(),
            files: FileStorageConfig::default(),
            vector_db: None,
        };
        let merged = base.merge(other);
        assert_eq!(merged.database.url, "postgresql://new/db");
    }

    #[test]
    fn test_redis_config_allow_degraded_default_false() {
        let config = RedisConfig::default();
        assert!(
            !config.allow_degraded,
            "allow_degraded must default to false so explicit failures are surfaced"
        );
    }

    #[test]
    fn test_redis_config_merge_allow_degraded() {
        let base = RedisConfig::default();
        let other = RedisConfig {
            allow_degraded: true,
            ..RedisConfig::default()
        };
        let merged = base.merge(other);
        assert!(merged.allow_degraded);
    }

    #[test]
    fn test_database_config_allow_degraded_default_false() {
        let config = DatabaseConfig::default();
        assert!(
            !config.allow_degraded,
            "allow_degraded must default to false so explicit failures are surfaced"
        );
    }

    #[test]
    fn test_database_config_auto_migrate_default_false() {
        let config = DatabaseConfig::default();
        assert!(
            !config.auto_migrate,
            "auto_migrate must default to false for configured databases"
        );
    }

    #[test]
    fn test_database_config_merge_allow_degraded() {
        let base = DatabaseConfig::default();
        let other = DatabaseConfig {
            allow_degraded: true,
            ..DatabaseConfig::default()
        };
        let merged = base.merge(other);
        assert!(merged.allow_degraded);
    }

    #[test]
    fn test_storage_config_merge_files() {
        let base = StorageConfig::default();
        let other = StorageConfig {
            database: DatabaseConfig::default(),
            redis: RedisConfig::default(),
            files: FileStorageConfig {
                storage_type: "local".to_string(),
                local_path: Some("/var/lib/litellm/files".to_string()),
                s3: None,
            },
            vector_db: None,
        };

        let merged = base.merge(other);
        assert_eq!(
            merged.files.local_path,
            Some("/var/lib/litellm/files".to_string())
        );
    }

    #[test]
    fn test_storage_config_clone() {
        let config = StorageConfig::default();
        let cloned = config.clone();
        assert_eq!(config.database.url, cloned.database.url);
        assert_eq!(config.redis.url, cloned.redis.url);
    }
}
