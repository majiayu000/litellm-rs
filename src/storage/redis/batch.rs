//! Batch Redis operations
//!
//! This module provides batch operations for efficient multi-key operations.

use super::pool::RedisPool;
use crate::utils::error::gateway_error::{GatewayError, Result};
use redis::AsyncCommands;

impl RedisPool {
    /// Get multiple keys at once.
    ///
    /// Standalone mode uses `MGET`. Cluster mode issues per-key `GET` so keys
    /// in different hash slots do not trigger CROSSSLOT errors. Result order
    /// matches `keys`.
    pub async fn mget(&self, keys: &[String]) -> Result<Vec<Option<String>>> {
        if self.noop_mode {
            return Ok(vec![None; keys.len()]);
        }

        if self.is_cluster() {
            let mut values = Vec::with_capacity(keys.len());
            for key in keys {
                values.push(self.get(key).await?);
            }
            return Ok(values);
        }

        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let values: Vec<Option<String>> = c.mget(keys).await.map_err(GatewayError::from)?;
            Ok(values)
        } else {
            Ok(vec![None; keys.len()])
        }
    }

    /// Set multiple key-value pairs with optional TTL.
    ///
    /// Standalone mode uses an atomic pipeline. Cluster mode issues per-key
    /// `SET`/`SETEX` to stay slot-safe.
    pub async fn mset(&self, pairs: &[(String, String)], ttl: Option<u64>) -> Result<()> {
        if self.noop_mode || pairs.is_empty() {
            return Ok(());
        }

        if self.is_cluster() {
            for (key, value) in pairs {
                self.set(key, value, ttl).await?;
            }
            return Ok(());
        }

        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            // Use atomic pipeline for better performance and consistency
            let mut pipe = redis::pipe();
            pipe.atomic();

            for (key, value) in pairs {
                if let Some(ttl_seconds) = ttl {
                    pipe.set_ex(key, value, ttl_seconds);
                } else {
                    pipe.set(key, value);
                }
            }

            let _: () = pipe.query_async(c).await.map_err(GatewayError::from)?;
        }
        Ok(())
    }
}
