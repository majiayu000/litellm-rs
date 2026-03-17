//! Redis connection pool and core connection management
//!
//! This module provides Redis connectivity, connection pooling, and health checks.

use crate::config::models::storage::RedisConfig;
use crate::utils::error::gateway_error::{GatewayError, Result};
use redis::{AsyncConnectionConfig, Client, aio::MultiplexedConnection};
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tracing::{debug, info};

/// Redis connection pool (supports no-op mode when Redis is unavailable)
#[derive(Debug, Clone)]
pub struct RedisPool {
    /// Redis client (None in no-op mode)
    pub(crate) client: Option<Client>,
    /// Connection manager (None in no-op mode)
    pub(crate) connection_manager: Option<MultiplexedConnection>,
    /// Configuration
    pub(crate) config: RedisConfig,
    /// Whether this is a no-op pool (Redis unavailable)
    pub(crate) noop_mode: bool,
    /// Semaphore to enforce max_connections concurrency limit
    pub(crate) semaphore: Arc<Semaphore>,
}

/// Redis connection wrapper
pub struct RedisConnection {
    pub(crate) conn: Option<MultiplexedConnection>,
    /// Held permit that is released when the connection is dropped
    pub(crate) _permit: Option<OwnedSemaphorePermit>,
}

impl RedisPool {
    /// Create a new Redis pool
    pub async fn new(config: &RedisConfig) -> Result<Self> {
        if !config.enabled {
            info!("Redis disabled in config; using no-op Redis pool");
            return Ok(Self {
                client: None,
                connection_manager: None,
                config: config.clone(),
                noop_mode: true,
                semaphore: Arc::new(Semaphore::new(1)),
            });
        }

        info!("Creating Redis connection pool");
        debug!("Redis URL: {}", Self::sanitize_url(&config.url));
        debug!(
            "Redis max_connections: {}, connection_timeout: {}s",
            config.max_connections, config.connection_timeout
        );

        let client = Client::open(config.url.as_str()).map_err(GatewayError::Redis)?;

        let async_config = AsyncConnectionConfig::new()
            .set_connection_timeout(std::time::Duration::from_secs(config.connection_timeout));

        let connection_manager = client
            .get_multiplexed_async_connection_with_config(&async_config)
            .await
            .map_err(GatewayError::Redis)?;

        let max_connections = config.max_connections.max(1) as usize;

        info!(
            "Redis connection pool created successfully (max_connections={})",
            max_connections
        );
        Ok(Self {
            client: Some(client),
            connection_manager: Some(connection_manager),
            config: config.clone(),
            noop_mode: false,
            semaphore: Arc::new(Semaphore::new(max_connections)),
        })
    }

    /// Create a no-op Redis pool (for when Redis is unavailable)
    pub fn create_noop() -> Self {
        info!("Creating no-op Redis pool (Redis unavailable)");
        Self {
            client: None,
            connection_manager: None,
            config: RedisConfig {
                url: String::new(),
                enabled: false,
                max_connections: 0,
                connection_timeout: 0,
                cluster: false,
            },
            noop_mode: true,
            semaphore: Arc::new(Semaphore::new(1)),
        }
    }

    /// Check if this is a no-op pool
    pub fn is_noop(&self) -> bool {
        self.noop_mode
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
            conn: self.connection_manager.clone(),
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
                .map_err(GatewayError::Redis)?;
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
