//! Webhook delivery processing
//!
//! This module handles webhook delivery queue processing and individual webhook delivery.

use super::manager::WebhookManager;
use super::types::{WebhookConfig, WebhookDelivery, WebhookDeliveryStatus, WebhookPayload};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::time::Duration;
use tracing::{debug, error};

impl WebhookManager {
    /// Process webhook delivery queue
    pub async fn process_delivery_queue(&self) -> Result<()> {
        // First, collect deliveries to process and their configs
        let deliveries_to_process: Vec<(usize, WebhookDelivery, WebhookConfig)> = {
            let data = self.data.read().await;
            data.delivery_queue
                .iter()
                .enumerate()
                .filter(|(_, delivery)| {
                    delivery.status == WebhookDeliveryStatus::Pending
                        || (delivery.status == WebhookDeliveryStatus::Retrying
                            && delivery
                                .next_retry_at
                                .is_some_and(|t| t <= chrono::Utc::now()))
                })
                .filter_map(|(idx, delivery)| {
                    data.webhooks
                        .get(&delivery.webhook_id)
                        .map(|config| (idx, delivery.clone(), config.clone()))
                })
                .collect()
        };

        // Process each delivery (without holding the lock)
        let mut results: Vec<(
            usize,
            WebhookDeliveryStatus,
            Option<String>,
            WebhookDelivery,
        )> = Vec::new();

        for (idx, mut delivery, config) in deliveries_to_process {
            let result = self.deliver_webhook_internal(&mut delivery, &config).await;

            match result {
                Ok(_) => {
                    results.push((idx, WebhookDeliveryStatus::Delivered, None, delivery));
                }
                Err(e) => {
                    delivery.attempts += 1;
                    if delivery.attempts >= config.max_retries {
                        results.push((
                            idx,
                            WebhookDeliveryStatus::Failed,
                            Some(e.to_string()),
                            delivery,
                        ));
                    } else {
                        let next_retry = chrono::Utc::now()
                            + chrono::Duration::seconds(config.retry_delay_seconds as i64);
                        results.push((
                            idx,
                            WebhookDeliveryStatus::Retrying,
                            Some(next_retry.to_rfc3339()),
                            delivery,
                        ));
                    }
                }
            }
        }

        // Apply results with a single lock acquisition
        {
            let mut data = self.data.write().await;
            for (idx, status, info, attempted) in results {
                if let Some(delivery) = data.delivery_queue.get_mut(idx) {
                    delivery.status = status.clone();
                    delivery.last_attempt_at = Some(chrono::Utc::now());
                    delivery.attempts = attempted.attempts;
                    delivery.response_status = attempted.response_status;
                    delivery.response_body = attempted.response_body;

                    match status {
                        WebhookDeliveryStatus::Delivered => {
                            data.stats.successful_deliveries += 1;
                        }
                        WebhookDeliveryStatus::Failed => {
                            data.stats.failed_deliveries += 1;
                            if let Some(err) = info {
                                error!("Webhook delivery failed permanently: {}", err);
                            }
                        }
                        WebhookDeliveryStatus::Retrying => {
                            if let Some(next_retry_str) = info {
                                delivery.next_retry_at =
                                    chrono::DateTime::parse_from_rfc3339(&next_retry_str)
                                        .ok()
                                        .map(|dt| dt.with_timezone(&chrono::Utc));
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Remove completed deliveries (keep failed ones for debugging)
            data.delivery_queue
                .retain(|d| d.status != WebhookDeliveryStatus::Delivered);
        }

        Ok(())
    }

    /// Deliver a single webhook (internal version with config)
    pub(super) async fn deliver_webhook_internal(
        &self,
        delivery: &mut WebhookDelivery,
        config: &WebhookConfig,
    ) -> Result<()> {
        let start_time = std::time::Instant::now();
        delivery.response_status = None;
        delivery.response_body = None;

        // Prepare request
        let mut request = self
            .client
            .post(&config.url)
            .map_err(|_| {
                GatewayError::Network(
                    "Webhook request rejected by outbound endpoint policy".to_string(),
                )
            })?
            .timeout(Duration::from_secs(config.timeout_seconds))
            .header("Content-Type", "application/json")
            .header("User-Agent", "LiteLLM-Gateway/1.0");

        // Add custom headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        // Add signature if secret is configured
        if let Some(secret) = &config.secret {
            let signature = self.generate_signature(&delivery.payload, secret)?;
            request = request.header("X-Webhook-Signature", signature);
        }

        // Send request
        let response = request
            .json(&delivery.payload)
            .send()
            .await
            .map_err(|error| {
                let message = if crate::utils::net::http::ProviderHttpClient::request_error_is_endpoint_policy(&error) {
                    "Webhook request rejected by outbound endpoint policy"
                } else {
                    "Webhook transport request failed"
                };
                GatewayError::Network(message.to_string())
            })?;

        let status_code = response.status().as_u16();
        let response_body = response.text().await.unwrap_or_default();

        delivery.response_status = Some(status_code);
        delivery.response_body = Some(response_body.clone());

        // Update delivery time statistics
        let delivery_time = start_time.elapsed().as_millis() as f64;
        {
            let mut data = self.data.write().await;
            data.stats.avg_delivery_time_ms = (data.stats.avg_delivery_time_ms
                * (data.stats.successful_deliveries as f64)
                + delivery_time)
                / (data.stats.successful_deliveries + 1) as f64;
        }

        if (200..300).contains(&status_code) {
            debug!("Webhook delivered successfully: {}", delivery.webhook_id);
            Ok(())
        } else {
            Err(GatewayError::Network(format!(
                "Webhook returned non-success status {}",
                status_code
            )))
        }
    }

    /// Generate webhook signature
    pub(super) fn generate_signature(
        &self,
        payload: &WebhookPayload,
        secret: &str,
    ) -> Result<String> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        type HmacSha256 = Hmac<Sha256>;

        let payload_json = serde_json::to_string(payload).map_err(GatewayError::from)?;

        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| GatewayError::Auth(e.to_string()))?;

        mac.update(payload_json.as_bytes());
        let result = mac.finalize();

        Ok(format!("sha256={}", hex::encode(result.into_bytes())))
    }
}
