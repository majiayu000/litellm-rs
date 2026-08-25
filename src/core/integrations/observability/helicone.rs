//! Helicone Integration
//!
//! Provides integration with Helicone for LLM observability and logging.

use crate::config::models::defaults::default_true;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info};

use super::durable_batch::DurableBatch;
use crate::core::traits::integration::{
    CacheHitEvent, EmbeddingEndEvent, EmbeddingErrorEvent, EmbeddingStartEvent, Integration,
    IntegrationError, IntegrationResult, LlmEndEvent, LlmErrorEvent, LlmStartEvent, LlmStreamEvent,
};
use crate::utils::net::http::create_custom_client;

/// Helicone configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeliconeConfig {
    /// Helicone API key
    pub api_key: String,

    /// Base URL for Helicone API
    #[serde(default = "default_base_url")]
    pub base_url: String,

    /// Enable request logging
    #[serde(default = "default_true")]
    pub enable_logging: bool,

    /// Enable caching through Helicone
    #[serde(default)]
    pub enable_cache: bool,

    /// Cache TTL in seconds (if caching enabled)
    #[serde(default = "default_cache_ttl")]
    pub cache_ttl_seconds: u64,

    /// Enable rate limiting through Helicone
    #[serde(default)]
    pub enable_rate_limit: bool,

    /// Rate limit policy
    #[serde(default)]
    pub rate_limit_policy: Option<String>,

    /// Custom properties to include with all requests
    #[serde(default)]
    pub custom_properties: HashMap<String, String>,

    /// User ID for tracking
    #[serde(default)]
    pub user_id: Option<String>,

    /// Batch size for sending events
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Flush interval in milliseconds
    #[serde(default = "default_flush_interval")]
    pub flush_interval_ms: u64,
}

fn default_base_url() -> String {
    "https://api.helicone.ai".to_string()
}

fn default_cache_ttl() -> u64 {
    3600
}

fn default_batch_size() -> usize {
    50
}

fn default_flush_interval() -> u64 {
    5000
}

impl Default for HeliconeConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: default_base_url(),
            enable_logging: true,
            enable_cache: false,
            cache_ttl_seconds: default_cache_ttl(),
            enable_rate_limit: false,
            rate_limit_policy: None,
            custom_properties: HashMap::new(),
            user_id: None,
            batch_size: default_batch_size(),
            flush_interval_ms: default_flush_interval(),
        }
    }
}

impl HeliconeConfig {
    /// Create a new Helicone configuration
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            ..Default::default()
        }
    }

    /// Set the base URL
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Enable caching
    pub fn with_cache(mut self, ttl_seconds: u64) -> Self {
        self.enable_cache = true;
        self.cache_ttl_seconds = ttl_seconds;
        self
    }

    /// Enable rate limiting
    pub fn with_rate_limit(mut self, policy: impl Into<String>) -> Self {
        self.enable_rate_limit = true;
        self.rate_limit_policy = Some(policy.into());
        self
    }

    /// Add a custom property
    pub fn property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom_properties.insert(key.into(), value.into());
        self
    }

    /// Set the user ID
    pub fn user_id(mut self, user_id: impl Into<String>) -> Self {
        self.user_id = Some(user_id.into());
        self
    }

    /// Create from environment variables
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("HELICONE_API_KEY").ok()?;

        Some(Self {
            api_key,
            base_url: std::env::var("HELICONE_BASE_URL").unwrap_or_else(|_| default_base_url()),
            ..Default::default()
        })
    }
}

/// Helicone log entry
#[derive(Debug, Clone, Serialize)]
struct HeliconeLogEntry {
    request_id: String,
    model: String,
    provider: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    prompt_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_tokens: Option<u32>,
    latency_ms: u64,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cost: Option<f64>,
    timestamp: i64,
    properties: HashMap<String, String>,
}

/// Helicone Integration
pub struct HeliconeIntegration {
    config: HeliconeConfig,
    http_client: Client,
    buffer: DurableBatch<HeliconeLogEntry>,
    pending_requests: Arc<RwLock<HashMap<String, PendingRequest>>>,
    enabled: bool,
}

/// Pending request tracking
#[derive(Debug, Clone)]
struct PendingRequest {
    start_time: u64,
    properties: HashMap<String, String>,
}

impl std::fmt::Debug for HeliconeIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeliconeIntegration")
            .field("base_url", &self.config.base_url)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl HeliconeIntegration {
    /// Create a new Helicone integration
    pub fn new(config: HeliconeConfig) -> IntegrationResult<Self> {
        if config.api_key.is_empty() {
            return Err(IntegrationError::config(
                "Helicone API key is required".to_string(),
            ));
        }

        let http_client = create_custom_client(Duration::from_secs(30)).map_err(|e| {
            IntegrationError::connection(format!("Failed to create HTTP client: {}", e))
        })?;

        info!("Helicone integration initialized");

        Ok(Self {
            config,
            http_client,
            buffer: DurableBatch::default(),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            enabled: true,
        })
    }

    /// Create from environment variables
    pub fn from_env() -> IntegrationResult<Self> {
        let config = HeliconeConfig::from_env()
            .ok_or_else(|| IntegrationError::config("HELICONE_API_KEY not set".to_string()))?;
        Self::new(config)
    }

    /// Get current timestamp in milliseconds
    fn current_timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Build properties map
    fn build_properties(&self, extra: &[(&str, &str)]) -> HashMap<String, String> {
        let mut props = self.config.custom_properties.clone();

        if let Some(user_id) = &self.config.user_id {
            props.insert("user_id".to_string(), user_id.clone());
        }

        for (key, value) in extra {
            props.insert(key.to_string(), value.to_string());
        }

        props
    }

    /// Send logs to Helicone
    async fn send_logs(&self, logs: &[HeliconeLogEntry]) -> IntegrationResult<()> {
        if logs.is_empty() {
            return Ok(());
        }

        let url = format!("{}/v1/log/batch", self.config.base_url);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&logs)
            .send()
            .await
            .map_err(|e| IntegrationError::connection(format!("Failed to send logs: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let response_bytes = response.bytes().await.map(|bytes| bytes.len()).unwrap_or(0);
            return Err(IntegrationError::connection(format!(
                "Helicone API returned {status} ({response_bytes} response bytes)"
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl Integration for HeliconeIntegration {
    fn name(&self) -> &'static str {
        "helicone"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn on_llm_start(&self, event: &LlmStartEvent) -> IntegrationResult<()> {
        debug!("Helicone: LLM request started - {}", event.request_id);

        let pending = PendingRequest {
            start_time: Self::current_timestamp_ms(),
            properties: self.build_properties(&[]),
        };

        let mut pending_requests = self.pending_requests.write().await;
        pending_requests.insert(event.request_id.clone(), pending);

        Ok(())
    }

    async fn on_llm_end(&self, event: &LlmEndEvent) -> IntegrationResult<()> {
        debug!("Helicone: LLM request completed - {}", event.request_id);

        let pending = {
            let mut pending_requests = self.pending_requests.write().await;
            pending_requests.remove(&event.request_id)
        };

        let (start_time, properties) = match pending {
            Some(p) => (p.start_time, p.properties),
            None => (
                Self::current_timestamp_ms() - event.latency_ms,
                self.build_properties(&[]),
            ),
        };

        let log_entry = HeliconeLogEntry {
            request_id: event.request_id.clone(),
            model: event.model.clone(),
            provider: event
                .provider
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            prompt_tokens: event.input_tokens,
            completion_tokens: event.output_tokens,
            total_tokens: match (event.input_tokens, event.output_tokens) {
                (Some(i), Some(o)) => Some(i + o),
                _ => None,
            },
            latency_ms: event.latency_ms,
            status: "success".to_string(),
            error: None,
            cost: event.cost_usd,
            timestamp: start_time as i64,
            properties,
        };

        if self.buffer.push(log_entry).await >= self.config.batch_size {
            let _ = self.flush().await;
        }

        Ok(())
    }

    async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()> {
        debug!("Helicone: LLM request error - {}", event.request_id);

        let pending = {
            let mut pending_requests = self.pending_requests.write().await;
            pending_requests.remove(&event.request_id)
        };

        let (start_time, properties) = match pending {
            Some(p) => (p.start_time, p.properties),
            None => (Self::current_timestamp_ms(), self.build_properties(&[])),
        };

        let log_entry = HeliconeLogEntry {
            request_id: event.request_id.clone(),
            model: event.model.clone(),
            provider: event
                .provider
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            latency_ms: 0,
            status: "error".to_string(),
            error: Some(event.error_message.clone()),
            cost: None,
            timestamp: start_time as i64,
            properties,
        };

        if self.buffer.push(log_entry).await >= self.config.batch_size {
            let _ = self.flush().await;
        }

        Ok(())
    }

    async fn on_llm_stream(&self, _event: &LlmStreamEvent) -> IntegrationResult<()> {
        // Helicone tracks complete requests, not individual stream chunks
        Ok(())
    }

    async fn on_embedding_start(&self, event: &EmbeddingStartEvent) -> IntegrationResult<()> {
        let pending = PendingRequest {
            start_time: Self::current_timestamp_ms(),
            properties: self.build_properties(&[("type", "embedding")]),
        };

        let mut pending_requests = self.pending_requests.write().await;
        pending_requests.insert(event.request_id.clone(), pending);

        Ok(())
    }

    async fn on_embedding_end(&self, event: &EmbeddingEndEvent) -> IntegrationResult<()> {
        let pending = {
            let mut pending_requests = self.pending_requests.write().await;
            pending_requests.remove(&event.request_id)
        };

        let (start_time, properties) = match pending {
            Some(p) => (p.start_time, p.properties),
            None => (
                Self::current_timestamp_ms() - event.latency_ms,
                self.build_properties(&[("type", "embedding")]),
            ),
        };

        let log_entry = HeliconeLogEntry {
            request_id: event.request_id.clone(),
            model: event.model.clone(),
            provider: event
                .provider
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            prompt_tokens: event.total_tokens,
            completion_tokens: None,
            total_tokens: event.total_tokens,
            latency_ms: event.latency_ms,
            status: "success".to_string(),
            error: None,
            cost: event.cost_usd,
            timestamp: start_time as i64,
            properties,
        };

        self.buffer.push(log_entry).await;

        Ok(())
    }

    async fn on_embedding_error(&self, event: &EmbeddingErrorEvent) -> IntegrationResult<()> {
        let pending = self
            .pending_requests
            .write()
            .await
            .remove(&event.request_id);
        let (start_time, properties) = match pending {
            Some(pending) => (pending.start_time, pending.properties),
            None => (
                Self::current_timestamp_ms().saturating_sub(event.latency_ms),
                self.build_properties(&[("type", "embedding")]),
            ),
        };

        let log_entry = HeliconeLogEntry {
            request_id: event.request_id.clone(),
            model: event.model.clone(),
            provider: event
                .provider
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
            prompt_tokens: None,
            completion_tokens: None,
            total_tokens: None,
            latency_ms: event.latency_ms,
            status: "error".to_string(),
            error: Some(event.error_message.clone()),
            cost: None,
            timestamp: start_time as i64,
            properties,
        };

        if self.buffer.push(log_entry).await >= self.config.batch_size {
            self.flush().await?;
        }

        Ok(())
    }

    async fn on_cache_hit(&self, _event: &CacheHitEvent) -> IntegrationResult<()> {
        // Cache hits are tracked separately in Helicone
        Ok(())
    }

    async fn flush(&self) -> IntegrationResult<()> {
        let _flush = self.buffer.serialize_flush().await;
        loop {
            let logs = self.buffer.batch_for_export().await;
            if logs.is_empty() {
                return Ok(());
            }
            debug!("Helicone: Flushing {} log entries", logs.len());
            self.send_logs(&logs).await?;
            self.buffer.acknowledge().await;
        }
    }

    async fn shutdown(&self) -> IntegrationResult<()> {
        info!("Helicone integration shutting down");
        self.flush().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_helicone_config_builder() {
        let config = HeliconeConfig::new("test-api-key")
            .base_url("https://custom.helicone.ai")
            .with_cache(7200)
            .with_rate_limit("10/minute")
            .property("env", "test")
            .user_id("user-123");

        assert_eq!(config.api_key, "test-api-key");
        assert_eq!(config.base_url, "https://custom.helicone.ai");
        assert!(config.enable_cache);
        assert_eq!(config.cache_ttl_seconds, 7200);
        assert!(config.enable_rate_limit);
        assert_eq!(config.rate_limit_policy, Some("10/minute".to_string()));
        assert_eq!(
            config.custom_properties.get("env"),
            Some(&"test".to_string())
        );
        assert_eq!(config.user_id, Some("user-123".to_string()));
    }

    #[test]
    fn test_helicone_config_default() {
        let config = HeliconeConfig::default();

        assert_eq!(config.base_url, "https://api.helicone.ai");
        assert!(config.enable_logging);
        assert!(!config.enable_cache);
        assert!(!config.enable_rate_limit);
    }

    #[test]
    fn test_helicone_integration_requires_api_key() {
        let config = HeliconeConfig::default();
        let result = HeliconeIntegration::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_helicone_integration_creation() {
        let config = HeliconeConfig::new("test-api-key");
        let result = HeliconeIntegration::new(config);
        assert!(result.is_ok());

        let integration = result.unwrap();
        assert_eq!(integration.name(), "helicone");
        assert!(integration.is_enabled());
    }

    #[test]
    fn test_build_properties() {
        let config = HeliconeConfig::new("test-key")
            .property("env", "test")
            .user_id("user-123");
        let integration = HeliconeIntegration::new(config).unwrap();

        let props = integration.build_properties(&[("extra", "value")]);

        assert_eq!(props.get("env"), Some(&"test".to_string()));
        assert_eq!(props.get("user_id"), Some(&"user-123".to_string()));
        assert_eq!(props.get("extra"), Some(&"value".to_string()));
    }

    #[tokio::test]
    async fn embedding_error_clears_pending_request_and_buffers_error_log() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut config = HeliconeConfig::new("test-key");
        config.base_url = format!("http://{}", listener.local_addr().unwrap());
        config.batch_size = 1;
        drop(listener);
        let integration = HeliconeIntegration::new(config).expect("test integration");
        let start = EmbeddingStartEvent {
            request_id: "embedding-error".to_string(),
            model: "embedding-model".to_string(),
            provider: Some("provider".to_string()),
            input_count: 1,
            user_id: None,
            timestamp_ms: chrono::Utc::now().timestamp_millis(),
        };
        integration.on_embedding_start(&start).await.unwrap();
        assert_eq!(integration.pending_requests.read().await.len(), 1);

        integration
            .on_embedding_error(&EmbeddingErrorEvent {
                request_id: start.request_id,
                model: start.model,
                provider: start.provider,
                error_message: "embedding failed".to_string(),
                error_type: Some("provider_error".to_string()),
                latency_ms: 25,
                timestamp_ms: chrono::Utc::now().timestamp_millis(),
            })
            .await
            .expect_err("batch threshold must attempt export");

        assert!(integration.pending_requests.read().await.is_empty());
        let buffer = integration.buffer.snapshot().await;
        assert_eq!(buffer.len(), 1);
        assert_eq!(buffer[0].status, "error");
        assert_eq!(buffer[0].error.as_deref(), Some("embedding failed"));
        assert_eq!(
            buffer[0].properties.get("type"),
            Some(&"embedding".to_string())
        );
    }
}
