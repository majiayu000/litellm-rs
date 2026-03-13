//! Data loading functionality for the pricing service

use super::service::PricingService;
use super::types::LiteLLMModelInfo;
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;
use std::time::Duration;
use tracing::debug;

impl PricingService {
    /// Initialize pricing data (load from URL or local file)
    pub async fn initialize(&self) -> Result<()> {
        self.refresh_pricing_data().await
    }

    /// Load pricing data from URL
    pub(super) async fn load_from_url(&self) -> Result<HashMap<String, LiteLLMModelInfo>> {
        let response = self
            .http_client
            .get(&self.pricing_url)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| GatewayError::network(format!("Failed to fetch pricing data: {}", e)))?;

        if !response.status().is_success() {
            return Err(GatewayError::network(format!(
                "HTTP {}: Failed to fetch pricing data",
                response.status()
            )));
        }

        let text = response
            .text()
            .await
            .map_err(|e| GatewayError::network(format!("Failed to read response: {}", e)))?;

        let data: HashMap<String, LiteLLMModelInfo> = serde_json::from_str(&text)
            .map_err(|e| GatewayError::parsing(format!("Failed to parse pricing JSON: {}", e)))?;

        debug!("Loaded {} models from URL", data.len());
        Ok(data)
    }

    /// Load pricing data from local file
    pub(super) async fn load_from_file(&self) -> Result<HashMap<String, LiteLLMModelInfo>> {
        let content = tokio::fs::read_to_string(&self.pricing_url)
            .await
            .map_err(GatewayError::Io)?;

        let data: HashMap<String, LiteLLMModelInfo> = serde_json::from_str(&content)
            .map_err(|e| GatewayError::parsing(format!("Failed to parse pricing JSON: {}", e)))?;

        debug!("Loaded {} models from file", data.len());
        Ok(data)
    }
}
