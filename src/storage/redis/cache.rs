//! Basic Redis cache operations
//!
//! This module provides core key-value caching operations including get, set, delete, exists, expire, and ttl.

use super::pool::{RedisLiveConnection, RedisPool};
use crate::utils::error::gateway_error::{GatewayError, Result};
use redis::cluster_async::ClusterConnection;
use redis::cluster_routing::{RoutingInfo, SingleNodeRoutingInfo};
use redis::{AsyncCommands, Value};

impl RedisPool {
    /// Get a value from cache
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        if self.noop_mode {
            return Ok(None);
        }

        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let value: Option<String> = c.get(key).await.map_err(GatewayError::from)?;
            Ok(value)
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
        match conn.conn.as_mut() {
            Some(RedisLiveConnection::Standalone(c)) => scan_and_delete_standalone(c, prefix).await,
            Some(RedisLiveConnection::Cluster(c)) => scan_and_delete_cluster(c, prefix).await,
            None => Ok(0),
        }
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

async fn scan_and_delete_standalone(
    conn: &mut redis::aio::MultiplexedConnection,
    prefix: &str,
) -> Result<usize> {
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
            .query_async(&mut *conn)
            .await
            .map_err(GatewayError::from)?;

        if !keys.is_empty() {
            let count: usize = redis::cmd("DEL")
                .arg(&keys)
                .query_async(&mut *conn)
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

async fn scan_and_delete_cluster(conn: &mut ClusterConnection, prefix: &str) -> Result<usize> {
    let slots: Value = redis::cmd("CLUSTER")
        .arg("SLOTS")
        .query_async(&mut *conn)
        .await
        .map_err(GatewayError::from)?;
    let masters = cluster_master_addrs(&slots);

    if masters.is_empty() {
        return scan_node_and_delete(conn, None, prefix).await;
    }

    let mut deleted = 0_usize;
    for (host, port) in masters {
        deleted += scan_node_and_delete(conn, Some((host, port)), prefix).await?;
    }
    Ok(deleted)
}

async fn scan_node_and_delete(
    conn: &mut ClusterConnection,
    master: Option<(String, u16)>,
    prefix: &str,
) -> Result<usize> {
    let pattern = format!("{prefix}*");
    let mut cursor = 0_u64;
    let mut deleted = 0_usize;

    loop {
        let mut cmd = redis::cmd("SCAN");
        cmd.arg(cursor)
            .arg("MATCH")
            .arg(&pattern)
            .arg("COUNT")
            .arg(100_usize);

        let scanned = match &master {
            Some((host, port)) => conn
                .route_command(
                    cmd,
                    RoutingInfo::SingleNode(SingleNodeRoutingInfo::ByAddress {
                        host: host.clone(),
                        port: *port,
                    }),
                )
                .await
                .map_err(GatewayError::from)?,
            None => cmd.query_async(conn).await.map_err(GatewayError::from)?,
        };

        let (next_cursor, keys) = parse_scan_page(scanned)?;
        for key in keys {
            let count: usize = redis::cmd("DEL")
                .arg(&key)
                .query_async(&mut *conn)
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

fn cluster_master_addrs(slots: &Value) -> Vec<(String, u16)> {
    let Value::Array(ranges) = slots else {
        return Vec::new();
    };

    let mut masters = Vec::new();
    for range in ranges {
        let Value::Array(item) = range else {
            continue;
        };
        if item.len() < 3 {
            continue;
        }
        let Value::Array(node) = &item[2] else {
            continue;
        };
        if node.len() < 2 {
            continue;
        }
        let host = match &node[0] {
            Value::BulkString(bytes) => String::from_utf8_lossy(bytes).into_owned(),
            _ => continue,
        };
        let port = match node[1] {
            Value::Int(port) => port as u16,
            _ => continue,
        };
        if !masters
            .iter()
            .any(|(existing, p)| existing == &host && *p == port)
        {
            masters.push((host, port));
        }
    }
    masters
}

fn parse_scan_page(value: Value) -> Result<(u64, Vec<String>)> {
    redis::from_redis_value(value)
        .map_err(|error| GatewayError::Storage(format!("Invalid Redis SCAN response: {error}")))
}
