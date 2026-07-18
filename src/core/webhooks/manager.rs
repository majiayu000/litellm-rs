//! Webhook manager implementation
//!
//! This module contains the WebhookManager struct and its core functionality.

use super::types::{
    WebhookConfig, WebhookData, WebhookDelivery, WebhookDeliveryStatus, WebhookEventType,
    WebhookPayload, WebhookStats,
};
use crate::core::net::ProviderEndpointPolicy;
use crate::core::types::context::RequestContext;
use crate::utils::error::gateway_error::{GatewayError, Result};
use crate::utils::net::http::ProviderHttpClient;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use uuid::Uuid;

/// Webhook manager
pub struct WebhookManager {
    /// Public-only HTTP client for webhook requests
    pub(super) client: ProviderHttpClient,
    /// Consolidated webhook data - single lock for all related state
    pub(super) data: Arc<RwLock<WebhookData>>,
}

impl WebhookManager {
    /// Create a new webhook manager
    pub fn new() -> Result<Self> {
        let client = ProviderHttpClient::no_redirect(
            ProviderEndpointPolicy::public_only(),
            Duration::from_secs(30),
        )
        .map_err(|_| {
            GatewayError::Network("Failed to create policy-bound webhook client".to_string())
        })?;

        Ok(Self {
            client,
            data: Arc::new(RwLock::new(WebhookData::default())),
        })
    }

    #[cfg(test)]
    pub(super) fn with_client_for_test(client: ProviderHttpClient) -> Self {
        Self {
            client,
            data: Arc::new(RwLock::new(WebhookData::default())),
        }
    }

    /// Register a webhook
    pub async fn register_webhook(&self, id: String, mut config: WebhookConfig) -> Result<()> {
        let url = reqwest::Url::parse(&config.url).map_err(|_| {
            GatewayError::Validation(
                "Webhook URL is invalid or disallowed by outbound policy".to_string(),
            )
        })?;
        if !matches!(url.scheme(), "http" | "https")
            || ProviderEndpointPolicy::public_only()
                .validate_url_without_resolution(&url)
                .is_err()
        {
            return Err(GatewayError::Validation(
                "Webhook URL is invalid or disallowed by outbound policy".to_string(),
            ));
        }
        config.url = url.to_string();
        info!("Registering webhook: {}", id);

        let mut data = self.data.write().await;
        data.webhooks.insert(id, config);

        Ok(())
    }

    /// Unregister a webhook
    pub async fn unregister_webhook(&self, id: &str) -> Result<()> {
        info!("Unregistering webhook: {}", id);

        let mut data = self.data.write().await;
        data.webhooks.remove(id);

        Ok(())
    }

    /// Send webhook event
    pub async fn send_event(
        &self,
        event_type: WebhookEventType,
        event_data: serde_json::Value,
        context: Option<RequestContext>,
    ) -> Result<()> {
        let payload = WebhookPayload {
            event_type: event_type.clone(),
            timestamp: chrono::Utc::now(),
            request_context: context,
            data: event_data,
            metadata: HashMap::new(),
        };

        let mut data = self.data.write().await;
        // Pre-allocate with estimated capacity (most events go to few webhooks)
        let mut deliveries = Vec::with_capacity(data.webhooks.len().min(8));

        // Find webhooks subscribed to this event type
        for (webhook_id, config) in data.webhooks.iter() {
            if config.enabled && config.events.contains(&event_type) {
                let delivery = WebhookDelivery {
                    id: Uuid::new_v4().to_string(),
                    webhook_id: webhook_id.clone(),
                    payload: payload.clone(),
                    status: WebhookDeliveryStatus::Pending,
                    response_status: None,
                    response_body: None,
                    attempts: 0,
                    created_at: chrono::Utc::now(),
                    last_attempt_at: None,
                    next_retry_at: Some(chrono::Utc::now()),
                };
                deliveries.push(delivery);
            }
        }

        // Add to delivery queue and update statistics
        let delivery_count = deliveries.len();
        if !deliveries.is_empty() {
            data.delivery_queue.extend(deliveries);
            data.stats.total_events += 1;
            *data
                .stats
                .events_by_type
                .entry(format!("{:?}", event_type))
                .or_insert(0) += 1;
        }

        debug!(
            "Queued {} webhook deliveries for event: {:?}",
            delivery_count, event_type
        );
        Ok(())
    }

    /// Get webhook statistics
    pub async fn get_stats(&self) -> WebhookStats {
        self.data.read().await.stats.clone()
    }

    /// List all registered webhooks
    pub async fn list_webhooks(&self) -> HashMap<String, WebhookConfig> {
        self.data.read().await.webhooks.clone()
    }

    /// Get delivery history
    pub async fn get_delivery_history(&self, limit: Option<usize>) -> Vec<WebhookDelivery> {
        let data = self.data.read().await;
        let limit = limit.unwrap_or(100).min(data.delivery_queue.len());
        // Pre-allocate with exact capacity and collect from reverse iterator
        let mut result = Vec::with_capacity(limit);
        result.extend(data.delivery_queue.iter().rev().take(limit).cloned());
        result
    }

    /// Start background delivery processor
    pub async fn start_delivery_processor(&self) -> Result<()> {
        let manager = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                interval.tick().await;

                if let Err(e) = manager.process_delivery_queue().await {
                    error!("Error processing webhook delivery queue: {}", e);
                }
            }
        });

        info!("Started webhook delivery processor");
        Ok(())
    }
}

impl Clone for WebhookManager {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            data: self.data.clone(),
        }
    }
}
