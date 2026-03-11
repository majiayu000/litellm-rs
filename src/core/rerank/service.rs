//! Rerank service and provider trait

use super::cache::RerankCache;
use super::types::{RerankRequest, RerankResponse};
use crate::utils::error::gateway_error::{GatewayError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{debug, info};

/// Trait for rerank providers
#[async_trait]
pub trait RerankProvider: Send + Sync {
    /// Rerank documents based on query relevance
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse>;

    /// Get the provider name
    fn provider_name(&self) -> &'static str;

    /// Check if a model is supported
    fn supports_model(&self, model: &str) -> bool;

    /// Get supported models
    fn supported_models(&self) -> Vec<&'static str>;
}

/// Rerank service that routes to appropriate providers
pub struct RerankService {
    /// Registered rerank providers
    providers: HashMap<String, Arc<dyn RerankProvider>>,

    /// Default provider name
    default_provider: Option<String>,

    /// Request timeout
    timeout: Duration,

    /// Enable caching
    enable_cache: bool,

    /// Cache for rerank results
    cache: Option<Arc<RerankCache>>,
}

impl RerankService {
    /// Create a new rerank service
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            default_provider: None,
            timeout: Duration::from_secs(30),
            enable_cache: false,
            cache: None,
        }
    }

    /// Register a rerank provider
    pub fn register_provider(
        &mut self,
        name: impl Into<String>,
        provider: Arc<dyn RerankProvider>,
    ) -> &mut Self {
        let name = name.into();
        info!("Registering rerank provider: {}", name);
        self.providers.insert(name, provider);
        self
    }

    /// Set the default provider
    pub fn set_default_provider(&mut self, name: impl Into<String>) -> &mut Self {
        self.default_provider = Some(name.into());
        self
    }

    /// Set request timeout
    pub fn set_timeout(&mut self, timeout: Duration) -> &mut Self {
        self.timeout = timeout;
        self
    }

    /// Enable caching
    pub fn enable_cache(&mut self, cache: Arc<RerankCache>) -> &mut Self {
        self.enable_cache = true;
        self.cache = Some(cache);
        self
    }

    /// Rerank documents using the appropriate provider
    pub async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse> {
        let start = Instant::now();

        // Validate request
        self.validate_request(&request)?;

        // Check cache if enabled
        if self.enable_cache
            && let Some(cache) = &self.cache
            && let Some(cached) = cache.get(&request).await
        {
            debug!("Rerank cache hit for query: {}", request.query);
            return Ok(cached);
        }

        // Determine provider from model name
        let provider_name = self.extract_provider_name(&request.model);
        let provider = self.get_provider(&provider_name)?;

        // Execute rerank with timeout
        let response = tokio::time::timeout(self.timeout, provider.rerank(request.clone()))
            .await
            .map_err(|_| {
                GatewayError::Timeout(format!("Rerank request timed out after {:?}", self.timeout))
            })??;

        // Cache result if enabled
        if self.enable_cache
            && let Some(cache) = &self.cache
        {
            cache.set(&request, &response).await;
        }

        let elapsed = start.elapsed();
        info!(
            "Rerank completed in {:?}: {} documents -> {} results",
            elapsed,
            request.documents.len(),
            response.results.len()
        );

        Ok(response)
    }

    /// Validate rerank request
    pub(crate) fn validate_request(&self, request: &RerankRequest) -> Result<()> {
        if request.query.is_empty() {
            return Err(GatewayError::BadRequest(
                "Query cannot be empty".to_string(),
            ));
        }

        if request.documents.is_empty() {
            return Err(GatewayError::BadRequest(
                "Documents list cannot be empty".to_string(),
            ));
        }

        if request.documents.len() > 10000 {
            return Err(GatewayError::BadRequest(
                "Too many documents (max 10000)".to_string(),
            ));
        }

        if let Some(top_n) = request.top_n
            && top_n == 0
        {
            return Err(GatewayError::BadRequest(
                "top_n must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    /// Extract provider name from model string (e.g., "cohere/rerank-v3" -> "cohere")
    pub(crate) fn extract_provider_name(&self, model: &str) -> String {
        if let Some(idx) = model.find('/') {
            model[..idx].to_string()
        } else {
            self.default_provider
                .clone()
                .unwrap_or_else(|| "cohere".to_string())
        }
    }

    /// Get provider by name
    fn get_provider(&self, name: &str) -> Result<&Arc<dyn RerankProvider>> {
        self.providers.get(name).ok_or_else(|| {
            GatewayError::ProviderNotFound(format!("Rerank provider not found: {}", name))
        })
    }

    /// Get all registered providers
    pub fn providers(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a model is supported by any provider
    pub fn supports_model(&self, model: &str) -> bool {
        let provider_name = self.extract_provider_name(model);
        if let Some(provider) = self.providers.get(&provider_name) {
            provider.supports_model(model)
        } else {
            false
        }
    }
}

impl Default for RerankService {
    fn default() -> Self {
        Self::new()
    }
}
