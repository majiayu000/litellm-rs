//! Caching and refresh functionality for the pricing service

use super::service::PricingService;
use super::types::{PricingEventType, PricingUpdateEvent};
use crate::utils::error::gateway_error::Result;
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

        let data = if self.pricing_url.starts_with("http") {
            // Load from URL
            self.load_from_url().await?
        } else {
            // Load from local file
            self.load_from_file().await?
        };

        // Update in-memory data and timestamp in single lock
        {
            let mut pricing_data = self.pricing_data.write();
            pricing_data.models.clear();
            pricing_data.models.extend(data);
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
