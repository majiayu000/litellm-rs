//! Caching and refresh functionality for the pricing service

use super::service::PricingService;
use super::types::{PricingEventType, PricingUpdateEvent};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, info, warn};

impl PricingService {
    /// Start automatic pricing data refresh task
    pub fn start_auto_refresh_task(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let service = Arc::clone(&self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(service.cache_ttl);

            loop {
                interval.tick().await;

                if let Err(e) = service.refresh_pricing_data().await {
                    warn!("Auto-refresh pricing data failed: {}", e);
                } else {
                    debug!("Auto-refresh pricing data completed successfully");
                }
            }
        })
    }

    /// Force refresh pricing data immediately
    pub async fn force_refresh(&self) -> Result<()> {
        info!("Force refreshing pricing data");
        self.refresh_pricing_data().await
    }

    /// Refresh pricing data from source
    pub async fn refresh_pricing_data(&self) -> Result<()> {
        info!("Refreshing pricing data from: {}", self.pricing_url);

        let data = if self.pricing_url.is_empty() {
            return Err(GatewayError::Config(
                "Pricing source is disabled".to_string(),
            ));
        } else if self.pricing_url == super::DEFAULT_PRICING_SOURCE {
            self.load_from_embedded_default()?
        } else if self.pricing_url.starts_with("http") {
            match self.load_from_url().await {
                Ok(data) => data,
                Err(remote_error) if self.should_fallback_to_embedded_on_remote_error() => {
                    warn!(
                        pricing_url = %self.pricing_url,
                        error = %remote_error,
                        "Default remote pricing data initial load failed; falling back to embedded pricing"
                    );
                    self.load_from_embedded_default().map_err(|embedded_error| {
                        GatewayError::Config(format!(
                            "Failed to load pricing data from remote source {}: {}; embedded pricing fallback also failed: {}",
                            self.pricing_url, remote_error, embedded_error
                        ))
                    })?
                }
                Err(remote_error) => return Err(remote_error),
            }
        } else {
            // Load from local file
            self.load_from_file().await?
        };

        // Update in-memory data and timestamp in single lock
        {
            let mut pricing_data = self.pricing_data.write();
            pricing_data.replace_models(data);
            pricing_data.last_updated = SystemTime::now();
        }

        // Send update event
        let _ = self.event_sender.send(PricingUpdateEvent {
            event_type: PricingEventType::DataRefreshed,
            model: "*".to_string(),
            provider: "*".to_string(),
            timestamp: SystemTime::now(),
        });

        info!("Pricing data refreshed successfully");
        Ok(())
    }

    /// Check if pricing data needs refresh
    pub fn needs_refresh(&self) -> bool {
        let data = self.pricing_data.read();
        SystemTime::now()
            .duration_since(data.last_updated)
            .map(|duration| duration > self.cache_ttl)
            .unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_source_does_not_enable_embedded_fallback() {
        let service = PricingService::new(None);
        assert!(!service.should_fallback_to_embedded_on_remote_error());
    }

    #[test]
    fn explicit_default_remote_url_disables_embedded_fallback() {
        let service = PricingService::new(Some(
            super::super::REMOTE_LITELLM_PRICING_SOURCE.to_string(),
        ));

        assert_eq!(
            service.pricing_url,
            super::super::REMOTE_LITELLM_PRICING_SOURCE
        );
        assert!(!service.should_fallback_to_embedded_on_remote_error());
    }

    #[tokio::test]
    async fn failed_refresh_preserves_models_and_canonical_index() {
        let temp_dir = tempfile::tempdir().expect("temporary directory should be created");
        let missing_source = temp_dir.path().join("missing-pricing.json");
        let service = PricingService::new(Some(missing_source.to_string_lossy().into_owned()));
        let model_info = serde_json::from_str(r#"{"litellm_provider":"synthetic","mode":"chat"}"#)
            .expect("test model should deserialize");
        service.add_custom_model("synthetic/Mixed-Model".to_string(), model_info);

        let error = service
            .force_refresh()
            .await
            .expect_err("missing pricing source must return an error");

        assert!(matches!(error, GatewayError::Io(_)));
        assert_eq!(
            service
                .get_model_info_for_provider("synthetic", "MIXED-MODEL")
                .map(|(model, _)| model),
            Some("synthetic/Mixed-Model".to_string())
        );
    }
}
