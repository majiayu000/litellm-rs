//! Basic Redis cache operations
//!
//! This module provides core key-value caching operations including get, set, delete, exists, expire, and ttl.

use super::pool::RedisPool;
use crate::utils::error::gateway_error::{GatewayError, Result};
use redis::{AsyncCommands, RedisResult};

impl RedisPool {
    /// Get a value from cache
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        if self.noop_mode {
            return Ok(None);
        }

        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let result: RedisResult<String> = c.get(key).await;
            match result {
                Ok(value) => Ok(Some(value)),
                Err(e) if e.kind() == redis::ErrorKind::UnexpectedReturnType => Ok(None),
                Err(e) => Err(GatewayError::from(e)),
            }
        } else {
            Ok(None)
        }
    }

    /// Set a key-value pair with optional TTL
    pub async fn set(&self, key: &str, value: &str, ttl: Option<u64>) -> Result<()> {
        if self.noop_mode {
            return Ok(());
        }

        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            if let Some(ttl_seconds) = ttl {
                let _: () = c
                    .set_ex(key, value, ttl_seconds)
                    .await
                    .map_err(GatewayError::from)?;
            } else {
                let _: () = c.set(key, value).await.map_err(GatewayError::from)?;
            }
        }
        Ok(())
    }

    /// Delete a key
    pub async fn delete(&self, key: &str) -> Result<()> {
        if self.noop_mode {
            return Ok(());
        }

        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let _: () = c.del(key).await.map_err(GatewayError::from)?;
        }
        Ok(())
    }

    /// Delete all keys whose Redis key starts with the provided prefix.
    pub async fn delete_by_prefix(&self, prefix: &str) -> Result<usize> {
        if self.noop_mode {
            return Ok(0);
        }

        let mut conn = self.get_connection().await?;
        let Some(ref mut c) = conn.conn else {
            return Ok(0);
        };

        let pattern = format!("{prefix}*");
        let mut cursor = 0_u64;
        let mut deleted = 0_usize;

        loop {
            let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(100_usize)
                .query_async(&mut *c)
                .await
                .map_err(GatewayError::from)?;

            if !keys.is_empty() {
                let count: usize = redis::cmd("DEL")
                    .arg(&keys)
                    .query_async(&mut *c)
                    .await
                    .map_err(GatewayError::from)?;
                deleted += count;
            }

            if next_cursor == 0 {
                break;
            }
            cursor = next_cursor;
        }

        Ok(deleted)
    }

    /// Check if a key exists
    pub async fn exists(&self, key: &str) -> Result<bool> {
        if self.noop_mode {
            return Ok(false);
        }

        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let exists: bool = c.exists(key).await.map_err(GatewayError::from)?;
            Ok(exists)
        } else {
            Ok(false)
        }
    }

    /// Set expiration time for a key
    pub async fn expire(&self, key: &str, ttl: u64) -> Result<()> {
        if self.noop_mode {
            return Ok(());
        }

        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let _: () = c
                .expire(key, ttl as i64)
                .await
                .map_err(GatewayError::from)?;
        }
        Ok(())
    }

    /// Get time to live for a key
    pub async fn ttl(&self, key: &str) -> Result<i64> {
        if self.noop_mode {
            return Ok(-2); // Key does not exist
        }

        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let ttl: i64 = c.ttl(key).await.map_err(GatewayError::from)?;
            Ok(ttl)
        } else {
            Ok(-2)
        }
    }
}
