//! Redis Pub/Sub operations
//!
//! This module provides publish/subscribe messaging functionality.
//! Subscription is stubbed out pending redis crate API stabilisation;
//! only `publish` is fully operational.

use super::pool::RedisPool;
use crate::utils::error::gateway_error::{GatewayError, Result};
use redis::AsyncCommands;

/// Handle for an active Redis pub/sub subscription.
///
/// Subscription is currently disabled at the API level.  When the redis
/// crate exposes a stable async subscribe API this type will be backed by
/// a real `redis::aio::PubSub` handle.
pub struct Subscription {
    _placeholder: (),
}

impl RedisPool {
    /// Publish a message to the given channel.
    pub async fn publish(&self, channel: &str, message: &str) -> Result<()> {
        if self.noop_mode {
            return Ok(());
        }

        let mut conn = self.get_connection().await?;
        if let Some(ref mut c) = conn.conn {
            let _: () = c
                .publish(channel, message)
                .await
                .map_err(GatewayError::from)?;
        }
        Ok(())
    }

    /// Subscribe to one or more Redis channels.
    ///
    /// Currently returns an error stub; re-enable once the redis crate
    /// exposes a stable `async-std`/`tokio` subscribe interface.
    pub async fn subscribe(&self, _channels: &[String]) -> Result<Subscription> {
        Err(GatewayError::Storage(
            "PubSub subscribe is not yet implemented".to_string(),
        ))
    }
}

impl Subscription {
    /// Receive the next pub/sub message.
    ///
    /// Stub — always returns an error until subscribe is implemented.
    pub async fn next_message(&mut self) -> Result<redis::Msg> {
        Err(GatewayError::Storage(
            "PubSub subscribe is not yet implemented".to_string(),
        ))
    }

    /// Unsubscribe from all channels.
    pub async fn unsubscribe_all(&mut self) -> Result<()> {
        Ok(())
    }
}
