//! Redis connection pool and core connection management
//!
//! This module provides Redis connectivity, connection pooling, and health checks.

use crate::config::models::storage::RedisConfig;
use crate::utils::error::gateway_error::{GatewayError, Result};
use redis::aio::{ConnectionLike, MultiplexedConnection};
use redis::cluster::ClusterClient;
use redis::cluster_async::ClusterConnection;
use redis::{AsyncConnectionConfig, Client, Cmd, Pipeline, RedisFuture, Value};
use std::fmt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{debug, info};

/// Format a Redis Cluster hash-tagged key so related keys map to one hash slot.
///
/// Cluster hashes only the substring between `{` and `}`. Use this when multiple
/// keys must participate in one atomic Lua script or MULTI/EXEC. Single-key
/// operations (including the rate-limit scripts, which use `KEYS[1]` only) do
/// not need a hash tag.
pub fn cluster_hash_tag(tag: &str, suffix: &str) -> String {
    format!("{{{tag}}}{suffix}")
}

/// Split `storage.redis.url` into cluster seed nodes. A single seed is enough;
/// comma-separated `redis://` / `rediss://` URLs are also accepted.
pub(crate) fn cluster_seed_urls(url: &str) -> Vec<&str> {
    url.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

/// Live Redis connection used by both standalone and cluster modes.
#[derive(Clone)]
pub(crate) enum RedisLiveConnection {
    Standalone(MultiplexedConnection),
    Cluster(ClusterConnection),
}

impl fmt::Debug for RedisLiveConnection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Standalone(_) => write!(f, "Standalone"),
            Self::Cluster(_) => write!(f, "Cluster"),
        }
    }
}

impl ConnectionLike for RedisLiveConnection {
    fn req_packed_command<'a>(&'a mut self, cmd: &'a Cmd) -> RedisFuture<'a, Value> {
        match self {
            Self::Standalone(conn) => conn.req_packed_command(cmd),
            Self::Cluster(conn) => conn.req_packed_command(cmd),
        }
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        pipeline: &'a Pipeline,
        offset: usize,
        count: usize,
    ) -> RedisFuture<'a, Vec<Value>> {
        match self {
            Self::Standalone(conn) => conn.req_packed_commands(pipeline, offset, count),
            Self::Cluster(conn) => conn.req_packed_commands(pipeline, offset, count),
        }
    }

    fn get_db(&self) -> i64 {
        match self {
            Self::Standalone(conn) => conn.get_db(),
            Self::Cluster(conn) => conn.get_db(),
        }
    }
}

/// Redis connection pool (supports no-op mode when Redis is unavailable)
#[derive(Debug, Clone)]
pub struct RedisPool {
    /// Live connection (None in no-op mode)
    pub(crate) connection: Option<RedisLiveConnection>,
    /// Configuration
    pub(crate) config: RedisConfig,
    /// Whether this is a no-op pool (Redis unavailable)
    pub(crate) noop_mode: bool,
    /// Semaphore to enforce max_connections concurrency limit
    pub(crate) semaphore: Arc<Semaphore>,
}

/// Redis connection wrapper
pub struct RedisConnection {
    pub(crate) conn: Option<RedisLiveConnection>,
    /// Held permit that is released when the connection is dropped
    pub(crate) _permit: Option<OwnedSemaphorePermit>,
}

impl RedisPool {
    /// Create a new Redis pool
    pub async fn new(config: &RedisConfig) -> Result<Self> {
        crate::config::Validate::validate(config).map_err(|error| {
            GatewayError::Config(format!("Invalid Redis configuration: {error}"))
        })?;

        if !config.enabled {
            info!("Redis disabled in config; using no-op Redis pool");
            return Ok(Self::noop_from_config(config));
        }

        debug!("Redis URL: {}", Self::sanitize_url(&config.url));
        debug!(
            "Redis max_connections: {}, connection_timeout: {}s, cluster: {}",
            config.max_connections, config.connection_timeout, config.cluster
        );

        let connection = if config.cluster {
            info!("Creating Redis Cluster connection pool");
            Some(Self::connect_cluster(config).await?)
        } else {
            info!("Creating Redis connection pool");
            Some(Self::connect_standalone(config).await?)
        };

        let max_connections = config.max_connections.max(1) as usize;

        info!(
            "Redis connection pool created successfully (max_connections={}, cluster={})",
            max_connections, config.cluster
        );
        Ok(Self {
            connection,
            config: config.clone(),
            noop_mode: false,
            semaphore: Arc::new(Semaphore::new(max_connections)),
        })
    }

    async fn connect_standalone(config: &RedisConfig) -> Result<RedisLiveConnection> {
        let client = Client::open(config.url.as_str()).map_err(GatewayError::from)?;
        let async_config = AsyncConnectionConfig::new()
            .set_connection_timeout(Some(Duration::from_secs(config.connection_timeout)));
        let connection = client
            .get_multiplexed_async_connection_with_config(&async_config)
            .await
            .map_err(GatewayError::from)?;
        Ok(RedisLiveConnection::Standalone(connection))
    }

    async fn connect_cluster(config: &RedisConfig) -> Result<RedisLiveConnection> {
        let seeds = cluster_seed_urls(&config.url);
        let timeout = Duration::from_secs(config.connection_timeout);
        let client = ClusterClient::builder(seeds)
            .connection_timeout(timeout)
            .build()
            .map_err(GatewayError::from)?;
        let connection = client
            .get_async_connection()
            .await
            .map_err(GatewayError::from)?;
        Ok(RedisLiveConnection::Cluster(connection))
    }

    fn noop_from_config(config: &RedisConfig) -> Self {
        Self {
            connection: None,
            config: config.clone(),
            noop_mode: true,
            semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    /// Create a no-op Redis pool (for when Redis is unavailable)
    pub fn create_noop() -> Self {
        info!("Creating no-op Redis pool (Redis unavailable)");
        Self::noop_from_config(&RedisConfig {
            url: String::new(),
            enabled: false,
            max_connections: 0,
            connection_timeout: 0,
            cluster: false,
            allow_degraded: false,
        })
    }

    /// Check if this is a no-op pool
    pub fn is_noop(&self) -> bool {
        self.noop_mode
    }

    /// Whether this pool uses Redis Cluster routing.
    pub(crate) fn is_cluster(&self) -> bool {
        self.config.cluster && !self.noop_mode
    }

    /// Open a live connection on the current Tokio runtime.
    ///
    /// Budget Lua runs on a dedicated runtime so Actix `current_thread` workers
    /// can block without parking the multiplexed-connection driver.
    pub(crate) async fn open_live_connection(&self) -> Result<RedisLiveConnection> {
        if self.noop_mode {
            return Err(GatewayError::Storage(
                "budget redis backend is unavailable".to_string(),
            ));
        }
        if self.config.cluster {
            Self::connect_cluster(&self.config).await
        } else {
            Self::connect_standalone(&self.config).await
        }
    }

    /// Get a connection from the pool.
    ///
    /// The returned [`RedisConnection`] holds a semaphore permit that limits
    /// the number of concurrent in-flight Redis operations to `max_connections`.
    /// The permit is released automatically when the connection is dropped.
    pub async fn get_connection(&self) -> Result<RedisConnection> {
        if self.noop_mode {
            return Ok(RedisConnection {
                conn: None,
                _permit: None,
            });
        }

        let permit = self
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| GatewayError::Internal("Redis semaphore closed".to_string()))?;

        Ok(RedisConnection {
            conn: self.connection.clone(),
            _permit: Some(permit),
        })
    }

    /// Health check
    pub async fn health_check(&self) -> Result<()> {
        if self.noop_mode {
            debug!("Redis health check skipped (no-op mode)");
            return Ok(());
        }

        debug!("Performing Redis health check");
        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let _: String = redis::cmd("PING")
                .query_async(c)
                .await
                .map_err(GatewayError::from)?;
        }

        debug!("Redis health check passed");
        Ok(())
    }

    /// Close the connection pool
    pub async fn close(&self) -> Result<()> {
        info!("Closing Redis connection pool");
        // Connection manager will be dropped automatically
        info!("Redis connection pool closed");
        Ok(())
    }

    /// Sanitize Redis URL for logging (hide password)
    pub(crate) fn sanitize_url(url: &str) -> String {
        if let Ok(parsed) = url::Url::parse(url) {
            let mut sanitized = parsed.clone();
            if sanitized.password().is_some() {
                let _ = sanitized.set_password(Some("***"));
            }
            sanitized.to_string()
        } else {
            "invalid_url".to_string()
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::{cluster_hash_tag, cluster_seed_urls};

    #[test]
    fn cluster_hash_tag_wraps_only_the_tag() {
        assert_eq!(cluster_hash_tag("cache", ":user:1"), "{cache}:user:1");
    }

    #[test]
    fn cluster_seed_urls_split_comma_separated_seeds() {
        assert_eq!(
            cluster_seed_urls("redis://127.0.0.1:7000, redis://127.0.0.1:7001"),
            vec!["redis://127.0.0.1:7000", "redis://127.0.0.1:7001"]
        );
        assert_eq!(
            cluster_seed_urls("redis://127.0.0.1:7000"),
            vec!["redis://127.0.0.1:7000"]
        );
    }
}
