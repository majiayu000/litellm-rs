use super::super::{default_connection_timeout, default_redis_max_connections};
use serde::{Deserialize, Serialize};

/// Redis configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// When `enabled` is true and Redis init fails, allow the gateway to keep
    /// running with an in-process/no-op fallback instead of failing startup.
    ///
    /// Defaults to `false` so an explicitly enabled-but-unreachable Redis is
    /// surfaced at startup. Set to `true` only when running in environments
    /// where caching is best-effort and silent degradation is acceptable.
    #[serde(default)]
    pub allow_degraded: bool,
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
            allow_degraded: false,
        }
    }
}

impl RedisConfig {
    /// Merge Redis configurations.
    ///
    /// For source compatibility, a programmatic `false` in `other` retains
    /// the historical meaning of "not specified". Call
    /// [`Self::merge_with_cluster_override`] when an explicit `false` must
    /// clear an inherited `true` value.
    pub fn merge(self, other: Self) -> Self {
        self.merge_with_cluster_override(other, None)
    }

    /// Merge Redis configurations with presence-aware cluster semantics.
    ///
    /// `cluster_override` distinguishes an omitted value (`None`) from an
    /// explicit `true` or `false`. Configuration overlay loaders use this
    /// method so default-valued fields can override inherited settings without
    /// adding state to the public [`RedisConfig`] struct.
    pub fn merge_with_cluster_override(
        mut self,
        other: Self,
        cluster_override: Option<bool>,
    ) -> Self {
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
        if let Some(cluster) = cluster_override.or(other.cluster.then_some(true)) {
            self.cluster = cluster;
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
    fn yaml_keeps_existing_cluster_wire_shape() {
        let omitted = from_yaml("");
        let explicit_false = from_yaml("cluster: false");
        let explicit_true = from_yaml("cluster: true");

        assert!(!omitted.cluster);
        assert!(!explicit_false.cluster);
        assert!(explicit_true.cluster);
    }

    #[test]
    fn explicit_false_cluster_override_overrides_true_base() {
        let base = from_yaml("cluster: true");
        let overlay = from_yaml("cluster: false");

        let merged = base.merge_with_cluster_override(overlay, Some(false));

        assert!(!merged.cluster);
    }

    #[test]
    fn omitted_cluster_overlay_preserves_true_base() {
        let base = from_yaml("cluster: true");
        let overlay = from_yaml("enabled: true");

        let merged = base.merge(overlay);

        assert!(merged.cluster);
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
    fn programmatic_explicit_false_overlay_uses_override_api() {
        let base = RedisConfig {
            cluster: true,
            ..RedisConfig::default()
        };
        let overlay = RedisConfig::default();

        assert!(
            !base
                .merge_with_cluster_override(overlay, Some(false))
                .cluster
        );
    }

    #[test]
    fn serialization_keeps_existing_cluster_shape() {
        let value = serde_json::to_value(RedisConfig::default())
            .unwrap_or_else(|error| panic!("Redis config should serialize: {error}"));

        assert_eq!(value["cluster"], false);
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
