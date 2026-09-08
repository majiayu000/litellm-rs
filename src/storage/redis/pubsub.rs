//! Redis global Pub/Sub, including subscriptions through Redis Cluster seeds.
use super::pool::{RedisPool, cluster_seed_urls};
use crate::utils::error::gateway_error::{GatewayError, Result};
use futures::StreamExt;
use redis::AsyncCommands;
use std::time::Duration;

/// A dedicated async Pub/Sub connection.
pub struct Subscription {
    pubsub: redis::aio::PubSub,
}

impl RedisPool {
    /// Publish a message to the given channel.
    pub async fn publish(&self, channel: &str, message: &str) -> Result<()> {
        if self.noop_mode {
            return Err(GatewayError::Storage("Redis Pub/Sub is unavailable".into()));
        }
        let mut conn = self.get_connection().await?;
        let c = conn
            .conn
            .as_mut()
            .ok_or_else(|| GatewayError::Storage("Redis Pub/Sub is unavailable".into()))?;
        let _: () = c.publish(channel, message).await?;
        Ok(())
    }

    /// Subscribe through any reachable seed. Global Pub/Sub propagates across cluster nodes.
    pub async fn subscribe(&self, channels: &[String]) -> Result<Subscription> {
        if self.noop_mode {
            return Err(GatewayError::Storage("Redis Pub/Sub is unavailable".into()));
        }
        for seed in cluster_seed_urls(&self.config.url) {
            let attempt = async {
                let client = redis::Client::open(seed)?;
                let mut pubsub = client.get_async_pubsub().await?;
                pubsub.subscribe(channels).await?;
                Ok::<_, redis::RedisError>(Subscription { pubsub })
            };
            if let Ok(Ok(subscription)) =
                tokio::time::timeout(Duration::from_secs(self.config.connection_timeout), attempt)
                    .await
            {
                return Ok(subscription);
            }
        }
        Err(GatewayError::Storage(
            "No Redis seed accepted the Pub/Sub subscription".into(),
        ))
    }
}

impl Subscription {
    /// Wait for a message, failing explicitly when the connection closes.
    pub async fn next_message(&mut self) -> Result<redis::Msg> {
        self.pubsub
            .on_message()
            .next()
            .await
            .ok_or_else(|| GatewayError::Storage("Redis subscription disconnected".into()))
    }

    /// Remove all subscriptions from this connection.
    pub async fn unsubscribe_all(&mut self) -> Result<()> {
        self.pubsub.punsubscribe("*").await?;
        self.pubsub.unsubscribe(&[] as &[String]).await?;
        Ok(())
    }
}
