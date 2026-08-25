use super::super::{default_connection_timeout, default_redis_max_connections};
use serde::{Deserialize, Serialize};

/// Redis configuration.
#[derive(Debug, Clone, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RedisConfig {
    /// Redis URL.
    pub url: String,
    /// Enable Redis (if false, use in-memory cache).
    #[serde(default = "default_redis_enabled")]
    pub enabled: bool,
    /// Maximum connections.
    #[serde(default = "default_redis_max_connections")]
    pub max_connections: u32,
    /// Connection timeout in seconds.
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,
    /// Enable cluster mode. Not implemented: startup validation rejects
    /// `cluster=true` instead of silently using a standalone connection.
    #[serde(default)]
    pub cluster: bool,
    /// Tracks whether `cluster` was present in deserialized config.
    ///
    /// This preserves merge semantics for layered configuration: an omitted
    /// field must not be treated as an explicit `false` override.
    #[doc(hidden)]
    #[serde(skip)]
    pub cluster_configured: bool,
    /// When `enabled` is true and Redis init fails, allow the gateway to keep
    /// running with an in-process/no-op fallback instead of failing startup.
    ///
    /// Defaults to `false` so an explicitly enabled-but-unreachable Redis is
    /// surfaced at startup. Set to `true` only when running in environments
    /// where caching is best-effort and silent degradation is acceptable.
    #[serde(default)]
    pub allow_degraded: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RedisConfigFields {
    url: String,
    #[serde(default = "default_redis_enabled")]
    enabled: bool,
    #[serde(default = "default_redis_max_connections")]
    max_connections: u32,
    #[serde(default = "default_connection_timeout")]
    connection_timeout: u64,
    #[serde(default, deserialize_with = "deserialize_cluster")]
    cluster: Option<bool>,
    #[serde(default)]
    allow_degraded: bool,
}

impl<'de> Deserialize<'de> for RedisConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let fields = RedisConfigFields::deserialize(deserializer)?;
        let cluster_configured = fields.cluster.is_some();

        Ok(Self {
            url: fields.url,
            enabled: fields.enabled,
            max_connections: fields.max_connections,
            connection_timeout: fields.connection_timeout,
            cluster: fields.cluster.unwrap_or(false),
            cluster_configured,
            allow_degraded: fields.allow_degraded,
        })
    }
}

fn deserialize_cluster<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    bool::deserialize(deserializer).map(Some)
}

fn default_redis_url() -> String {
    "redis://localhost:6379".to_string()
}

fn default_redis_enabled() -> bool {
    false
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            enabled: default_redis_enabled(),
            max_connections: default_redis_max_connections(),
            connection_timeout: default_connection_timeout(),
            cluster: false,
            cluster_configured: false,
            allow_degraded: false,
        }
    }
}

impl RedisConfig {
    /// Set cluster mode while preserving explicit presence for later merges.
    pub fn with_cluster(mut self, cluster: bool) -> Self {
        self.cluster = cluster;
        self.cluster_configured = true;
        self
    }

    /// Merge Redis configurations.
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
        if other.cluster_configured || other.cluster {
            self.cluster = other.cluster;
            self.cluster_configured = other.cluster_configured;
        }
        // Redis defaults to enabled=false; propagate if other differs from default.
        if other.enabled != default_redis_enabled() {
            self.enabled = other.enabled;
        }
        if other.allow_degraded {
            self.allow_degraded = true;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::RedisConfig;

    fn from_yaml(fragment: &str) -> RedisConfig {
        serde_yml::from_str(&format!("url: redis://localhost:6379\n{fragment}"))
            .unwrap_or_else(|error| panic!("Redis YAML should parse: {error}"))
    }

    #[test]
    fn yaml_tracks_cluster_presence() {
        let omitted = from_yaml("");
        let explicit_false = from_yaml("cluster: false");
        let explicit_true = from_yaml("cluster: true");

        assert!(!omitted.cluster_configured);
        assert!(!explicit_false.cluster);
        assert!(explicit_false.cluster_configured);
        assert!(explicit_true.cluster);
        assert!(explicit_true.cluster_configured);
    }

    #[test]
    fn explicit_false_cluster_overlay_overrides_true_base() {
        let base = from_yaml("cluster: true");
        let overlay = from_yaml("cluster: false");

        let merged = base.merge(overlay);

        assert!(!merged.cluster);
        assert!(merged.cluster_configured);
    }

    #[test]
    fn omitted_cluster_overlay_preserves_true_base() {
        let base = from_yaml("cluster: true");
        let overlay = from_yaml("enabled: true");

        let merged = base.merge(overlay);

        assert!(merged.cluster);
        assert!(merged.cluster_configured);
    }

    #[test]
    fn programmatic_true_overlay_remains_compatible() {
        let base = RedisConfig::default();
        let overlay = RedisConfig {
            cluster: true,
            ..RedisConfig::default()
        };

        assert!(base.merge(overlay).cluster);
    }

    #[test]
    fn programmatic_explicit_false_overlay_uses_presence_flag() {
        let base = RedisConfig {
            cluster: true,
            ..RedisConfig::default()
        };
        let overlay = RedisConfig::default().with_cluster(false);

        assert!(overlay.cluster_configured);
        assert!(!base.merge(overlay).cluster);
    }

    #[test]
    fn serialization_keeps_existing_cluster_shape() {
        let value = serde_json::to_value(RedisConfig::default())
            .unwrap_or_else(|error| panic!("Redis config should serialize: {error}"));

        assert_eq!(value["cluster"], false);
        assert!(value.get("cluster_configured").is_none());
    }

    #[test]
    fn null_cluster_is_rejected() {
        let result = serde_yml::from_str::<RedisConfig>("url: redis://localhost:6379\ncluster:");

        assert!(result.is_err());
    }

    #[test]
    fn missing_url_remains_rejected() {
        let result = serde_yml::from_str::<RedisConfig>("cluster: false");

        assert!(result.is_err());
    }
}
