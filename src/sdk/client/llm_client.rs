//! Core LLM client implementation

use super::types::{LoadBalancer, LoadBalancingStrategy, ProviderStats};
use crate::core::router::RuntimeBinding;
use crate::sdk::{config::ClientConfig, config::SdkProviderConfig, errors::*};
use crate::utils::net::ClientUtils;
use crate::utils::net::http::{create_custom_client, create_streaming_client};
use reqwest;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::info;

/// Full-featured LLM client
#[derive(Debug)]
pub struct LLMClient {
    pub(crate) config: ClientConfig,
    pub(crate) http_client: reqwest::Client,
    pub(crate) stream_http_client: reqwest::Client,
    pub(crate) provider_stats: Arc<RwLock<HashMap<String, ProviderStats>>>,
    pub(crate) load_balancer: Arc<LoadBalancer>,
    pub(crate) runtime_binding: Option<RuntimeBinding>,
    pub(crate) runtime_default_model: Option<String>,
}

impl LLMClient {
    /// Create new LLM client
    ///
    /// This constructor retains the 0.6 `ClientConfig` compatibility transport.
    /// New applications that already own a gateway runtime should use
    /// [`Self::from_runtime`] so HTTP, SDK, and completion calls share selection
    /// state and typed errors.
    pub fn new(config: ClientConfig) -> Result<Self> {
        if config.providers.is_empty() {
            return Err(SDKError::ConfigError("No providers configured".to_string()));
        }

        // Build HTTP client
        let http_client = create_custom_client(Duration::from_secs(config.settings.timeout))
            .map_err(|e| SDKError::ConfigError(format!("Failed to create HTTP client: {}", e)))?;

        let stream_http_client = create_streaming_client().map_err(|e| {
            SDKError::ConfigError(format!("Failed to create streaming HTTP client: {}", e))
        })?;

        let provider_stats = Arc::new(RwLock::new(HashMap::new()));
        let load_balancer = Arc::new(LoadBalancer::new(LoadBalancingStrategy::WeightedRandom));

        info!(
            "LLMClient created with {} providers",
            config.providers.len()
        );

        Ok(Self {
            config,
            http_client,
            stream_http_client,
            provider_stats,
            load_balancer,
            runtime_binding: None,
            runtime_default_model: None,
        })
    }

    /// Create a stateless SDK facade over an explicit canonical runtime.
    pub fn from_runtime(runtime: RuntimeBinding, default_model: impl Into<String>) -> Result<Self> {
        let http_client = create_custom_client(Duration::from_secs(30))
            .map_err(|e| SDKError::ConfigError(format!("Failed to create HTTP client: {e}")))?;
        let stream_http_client = create_streaming_client().map_err(|e| {
            SDKError::ConfigError(format!("Failed to create streaming HTTP client: {e}"))
        })?;

        Ok(Self {
            config: ClientConfig::default(),
            http_client,
            stream_http_client,
            provider_stats: Arc::new(RwLock::new(HashMap::new())),
            load_balancer: Arc::new(LoadBalancer::new(LoadBalancingStrategy::WeightedRandom)),
            runtime_binding: Some(runtime),
            runtime_default_model: Some(default_model.into()),
        })
    }

    /// Create new LLM client asynchronously with initialization
    pub async fn new_async(config: ClientConfig) -> Result<Self> {
        let client = Self::new(config)?;

        // Initialize providers
        client.initialize_providers().await?;

        Ok(client)
    }

    /// Initialize provider statistics
    pub(crate) async fn initialize_providers(&self) -> Result<()> {
        use tracing::debug;

        let mut stats = self.provider_stats.write().await;

        for provider in &self.config.providers {
            let provider_stats = ProviderStats {
                health_score: 1.0, // Initial health score
                ..Default::default()
            };
            stats.insert(provider.id.clone(), provider_stats);

            // Log initialization
            debug!("Initialized provider: {}", provider.id);
        }

        Ok(())
    }

    /// List available providers
    pub fn list_providers(&self) -> Vec<String> {
        self.config.providers.iter().map(|p| p.id.clone()).collect()
    }

    /// Get configuration
    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// Find provider configuration by ID.
    pub(crate) fn provider_config(&self, provider_id: &str) -> Result<&SdkProviderConfig> {
        self.config
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .ok_or_else(|| SDKError::ProviderNotFound(provider_id.to_string()))
    }

    /// Return the configured default provider when it exists and is enabled.
    pub(crate) fn default_enabled_provider(&self) -> Option<&SdkProviderConfig> {
        self.config
            .default_provider
            .as_ref()
            .and_then(|provider_id| self.provider_config(provider_id).ok())
            .filter(|provider| provider.enabled)
    }

    /// Find the first enabled provider that explicitly supports `model`.
    pub(crate) fn provider_for_model(&self, model: &str) -> Result<&SdkProviderConfig> {
        self.config
            .providers
            .iter()
            .find(|provider| {
                provider.enabled && provider.models.iter().any(|candidate| candidate == model)
            })
            .ok_or_else(|| {
                SDKError::ModelNotFound(format!("Model '{}' not supported by any provider", model))
            })
    }

    /// Resolve the provider's default model without allocating a fallback `String`.
    pub(crate) fn provider_default_model<'a>(
        &self,
        provider: &'a SdkProviderConfig,
        fallback: &'a str,
    ) -> &'a str {
        provider
            .models
            .first()
            .map(String::as_str)
            .unwrap_or(fallback)
    }

    /// Resolve the provider's base URL without allocating a fallback `String`.
    pub(crate) fn provider_base_url<'a>(
        &self,
        provider: &'a SdkProviderConfig,
        fallback: &'a str,
    ) -> &'a str {
        provider.base_url.as_deref().unwrap_or(fallback)
    }

    /// Build a provider endpoint URL from its configured base URL.
    pub(crate) fn provider_endpoint(
        &self,
        provider: &SdkProviderConfig,
        fallback_base: &str,
        endpoint: &str,
    ) -> String {
        ClientUtils::add_path_to_api_base(self.provider_base_url(provider, fallback_base), endpoint)
    }

    /// Build the Anthropic messages endpoint, avoiding duplicate `/v1` segments.
    pub(crate) fn anthropic_messages_endpoint(&self, provider: &SdkProviderConfig) -> String {
        let base_url = self.provider_base_url(provider, "https://api.anthropic.com");
        let endpoint = if base_url.contains("/v1") {
            "messages"
        } else {
            "v1/messages"
        };

        ClientUtils::add_path_to_api_base(base_url, endpoint)
    }

    /// Health check all providers
    pub async fn health_check(&self) -> Result<HashMap<String, bool>> {
        let mut health_status = HashMap::new();

        for provider in &self.config.providers {
            let is_healthy = self.check_provider_health(&provider.id).await.is_ok();
            health_status.insert(provider.id.clone(), is_healthy);
        }

        Ok(health_status)
    }

    /// Check individual provider health
    pub(crate) async fn check_provider_health(&self, provider_id: &str) -> Result<()> {
        use crate::sdk::types::{ChatOptions, Content, Message, Role, SdkChatRequest};

        let simple_request = SdkChatRequest {
            model: String::new(),
            messages: vec![Message {
                role: Role::User,
                content: Some(Content::Text("Hi".to_string())),
                name: None,
                tool_calls: None,
            }],
            options: ChatOptions {
                max_tokens: Some(1),
                ..Default::default()
            },
        };

        // Send test request
        self.execute_chat_request(provider_id, simple_request)
            .await?;
        Ok(())
    }
}
