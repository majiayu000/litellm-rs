//! Arize AI Integration
//!
//! Provides integration with Arize AI for ML observability, monitoring, and evaluation.

use crate::config::models::defaults::default_true;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::core::traits::integration::{
    CacheHitEvent, EmbeddingEndEvent, EmbeddingStartEvent, Integration, IntegrationError,
    IntegrationResult, LlmEndEvent, LlmErrorEvent, LlmStartEvent, LlmStreamEvent,
};
use crate::utils::net::http::create_custom_client;

/// Arize configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArizeConfig {
    /// Arize API key
    pub api_key: String,

    /// Arize space key
    pub space_key: String,

    /// Model ID for tracking
    #[serde(default = "default_model_id")]
    pub model_id: String,

    /// Model version
    #[serde(default)]
    pub model_version: Option<String>,

    /// Environment (production, staging, etc.)
    #[serde(default = "default_environment")]
    pub environment: String,

    /// Base URL for Arize API
    #[serde(default = "default_base_url")]
    pub base_url: String,

    /// Batch size for sending records
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,

    /// Flush interval in milliseconds
    #[serde(default = "default_flush_interval")]
    pub flush_interval_ms: u64,

    /// Enable embedding logging
    #[serde(default = "default_true")]
    pub log_embeddings: bool,

    /// Custom tags
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

fn default_model_id() -> String {
    "litellm-gateway".to_string()
}

fn default_environment() -> String {
    "production".to_string()
}

fn default_base_url() -> String {
    "https://api.arize.com".to_string()
}

fn default_batch_size() -> usize {
    100
}

fn default_flush_interval() -> u64 {
    10000
}

impl Default for ArizeConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            space_key: String::new(),
            model_id: default_model_id(),
            model_version: None,
            environment: default_environment(),
            base_url: default_base_url(),
            batch_size: default_batch_size(),
            flush_interval_ms: default_flush_interval(),
            log_embeddings: true,
            tags: HashMap::new(),
        }
    }
}

impl ArizeConfig {
    /// Create a new Arize configuration
    pub fn new(api_key: impl Into<String>, space_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            space_key: space_key.into(),
            ..Default::default()
        }
    }

    /// Set the model ID
    pub fn model_id(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = model_id.into();
        self
    }

    /// Set the model version
    pub fn model_version(mut self, version: impl Into<String>) -> Self {
        self.model_version = Some(version.into());
        self
    }

    /// Set the environment
    pub fn environment(mut self, env: impl Into<String>) -> Self {
        self.environment = env.into();
        self
    }

    /// Add a tag
    pub fn tag(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Create from environment variables
    pub fn from_env() -> Option<Self> {
        let api_key = std::env::var("ARIZE_API_KEY").ok()?;
        let space_key = std::env::var("ARIZE_SPACE_KEY").ok()?;

        Some(Self {
            api_key,
            space_key,
            model_id: std::env::var("ARIZE_MODEL_ID").unwrap_or_else(|_| default_model_id()),
            model_version: std::env::var("ARIZE_MODEL_VERSION").ok(),
            environment: std::env::var("ARIZE_ENVIRONMENT")
                .unwrap_or_else(|_| default_environment()),
            ..Default::default()
        })
    }
}

/// Arize record for LLM inference
#[derive(Debug, Clone, Serialize)]
struct ArizeRecord {
    /// Unique prediction ID
    prediction_id: String,

    /// Model ID
    model_id: String,

    /// Model version
    #[serde(skip_serializing_if = "Option::is_none")]
    model_version: Option<String>,

    /// Environment
    environment: String,

    /// Timestamp in milliseconds
    timestamp_ms: i64,

    /// Prediction label (model name)
    prediction_label: String,

    /// Features (input metadata)
    features: HashMap<String, ArizeValue>,

    /// Tags
    tags: HashMap<String, String>,

    /// Actual label (for feedback)
    #[serde(skip_serializing_if = "Option::is_none")]
    actual_label: Option<String>,

    /// Latency in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    latency_ms: Option<u64>,
}

/// Arize value type
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
enum ArizeValue {
    String(String),
    Number(f64),
    Integer(i64),
}

/// Arize Integration
pub struct ArizeIntegration {
    config: ArizeConfig,
    http_client: Client,
    buffer: Arc<RwLock<Vec<ArizeRecord>>>,
    pending_requests: Arc<RwLock<HashMap<String, PendingRequest>>>,
    enabled: bool,
}

/// Pending request tracking
#[derive(Debug, Clone)]
struct PendingRequest {
    start_time: u64,
    features: HashMap<String, ArizeValue>,
}

impl std::fmt::Debug for ArizeIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArizeIntegration")
            .field("model_id", &self.config.model_id)
            .field("environment", &self.config.environment)
            .field("enabled", &self.enabled)
            .finish()
    }
}

impl ArizeIntegration {
    /// Create a new Arize integration
    pub fn new(config: ArizeConfig) -> IntegrationResult<Self> {
        if config.api_key.is_empty() {
            return Err(IntegrationError::config(
                "Arize API key is required".to_string(),
            ));
        }

        if config.space_key.is_empty() {
            return Err(IntegrationError::config(
                "Arize space key is required".to_string(),
            ));
        }

        let http_client = create_custom_client(Duration::from_secs(30)).map_err(|e| {
            IntegrationError::connection(format!("Failed to create HTTP client: {}", e))
        })?;

        info!(
            "Arize integration initialized for model: {}",
            config.model_id
        );

        Ok(Self {
            config,
            http_client,
            buffer: Arc::new(RwLock::new(Vec::new())),
            pending_requests: Arc::new(RwLock::new(HashMap::new())),
            enabled: true,
        })
    }

    /// Create from environment variables
    pub fn from_env() -> IntegrationResult<Self> {
        let config = ArizeConfig::from_env().ok_or_else(|| {
            IntegrationError::config("ARIZE_API_KEY and ARIZE_SPACE_KEY not set".to_string())
        })?;
        Self::new(config)
    }

    /// Get current timestamp in milliseconds
    fn current_timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }

    /// Build base features
    fn build_features(&self, model: &str, provider: &str) -> HashMap<String, ArizeValue> {
        let mut features = HashMap::new();
        features.insert("model".to_string(), ArizeValue::String(model.to_string()));
        features.insert(
            "provider".to_string(),
            ArizeValue::String(provider.to_string()),
        );
        features
    }

    /// Build tags
    fn build_tags(&self, extra: &[(&str, &str)]) -> HashMap<String, String> {
        let mut tags = self.config.tags.clone();
        for (key, value) in extra {
            tags.insert(key.to_string(), value.to_string());
        }
        tags
    }

    /// Send records to Arize
    async fn send_records(&self, records: Vec<ArizeRecord>) -> IntegrationResult<()> {
        if records.is_empty() {
            return Ok(());
        }

        let url = format!("{}/v1/log", self.config.base_url);

        let payload = serde_json::json!({
            "space_key": self.config.space_key,
            "records": records,
        });

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| IntegrationError::connection(format!("Failed to send records: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let response_bytes = response.bytes().await.map(|bytes| bytes.len()).unwrap_or(0);
            warn!(
                %status,
                response_bytes,
                "Arize API returned non-success status"
            );
        }

        Ok(())
    }
}

#[async_trait]
impl Integration for ArizeIntegration {
    fn name(&self) -> &'static str {
        "arize"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn on_llm_start(&self, event: &LlmStartEvent) -> IntegrationResult<()> {
        debug!("Arize: LLM request started - {}", event.request_id);

        let provider = event
            .provider
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let features = self.build_features(&event.model, &provider);

        let pending = PendingRequest {
            start_time: Self::current_timestamp_ms(),
            features,
        };

        let mut pending_requests = self.pending_requests.write().await;
        pending_requests.insert(event.request_id.clone(), pending);

        Ok(())
    }

    async fn on_llm_end(&self, event: &LlmEndEvent) -> IntegrationResult<()> {
        debug!("Arize: LLM request completed - {}", event.request_id);

        let pending = {
            let mut pending_requests = self.pending_requests.write().await;
            pending_requests.remove(&event.request_id)
        };

        let provider = event
            .provider
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let (start_time, mut features) = match pending {
            Some(p) => (p.start_time, p.features),
            None => (
                Self::current_timestamp_ms() - event.latency_ms,
                self.build_features(&event.model, &provider),
            ),
        };

        // Add usage metrics to features
        if let Some(input_tokens) = event.input_tokens {
            features.insert(
                "prompt_tokens".to_string(),
                ArizeValue::Integer(input_tokens as i64),
            );
        }
        if let Some(output_tokens) = event.output_tokens {
            features.insert(
                "completion_tokens".to_string(),
                ArizeValue::Integer(output_tokens as i64),
            );
        }
        if let (Some(input), Some(output)) = (event.input_tokens, event.output_tokens) {
            features.insert(
                "total_tokens".to_string(),
                ArizeValue::Integer((input + output) as i64),
            );
        }

        if let Some(cost) = event.cost_usd {
            features.insert("cost".to_string(), ArizeValue::Number(cost));
        }

        let record = ArizeRecord {
            prediction_id: event.request_id.clone(),
            model_id: self.config.model_id.clone(),
            model_version: self.config.model_version.clone(),
            environment: self.config.environment.clone(),
            timestamp_ms: start_time as i64,
            prediction_label: event.model.clone(),
            features,
            tags: self.build_tags(&[("status", "success")]),
            actual_label: None,
            latency_ms: Some(event.latency_ms),
        };

        let mut buffer = self.buffer.write().await;
        buffer.push(record);

        if buffer.len() >= self.config.batch_size {
            drop(buffer);
            let _ = self.flush().await;
        }

        Ok(())
    }

    async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()> {
        debug!("Arize: LLM request error - {}", event.request_id);

        let pending = {
            let mut pending_requests = self.pending_requests.write().await;
            pending_requests.remove(&event.request_id)
        };

        let provider = event
            .provider
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let (start_time, mut features) = match pending {
            Some(p) => (p.start_time, p.features),
            None => (
                Self::current_timestamp_ms(),
                self.build_features(&event.model, &provider),
            ),
        };

        features.insert(
            "error_message".to_string(),
            ArizeValue::String(event.error_message.clone()),
        );

        if let Some(error_type) = &event.error_type {
            features.insert(
                "error_type".to_string(),
                ArizeValue::String(error_type.clone()),
            );
        }

        let record = ArizeRecord {
            prediction_id: event.request_id.clone(),
            model_id: self.config.model_id.clone(),
            model_version: self.config.model_version.clone(),
            environment: self.config.environment.clone(),
            timestamp_ms: start_time as i64,
            prediction_label: event.model.clone(),
            features,
            tags: self.build_tags(&[("status", "error")]),
            actual_label: None,
            latency_ms: None,
        };

        let mut buffer = self.buffer.write().await;
        buffer.push(record);

        if buffer.len() >= self.config.batch_size {
            drop(buffer);
            let _ = self.flush().await;
        }

        Ok(())
    }

    async fn on_llm_stream(&self, _event: &LlmStreamEvent) -> IntegrationResult<()> {
        // Arize tracks complete predictions, not stream chunks
        Ok(())
    }

    async fn on_embedding_start(&self, event: &EmbeddingStartEvent) -> IntegrationResult<()> {
        if !self.config.log_embeddings {
            return Ok(());
        }

        let provider = event
            .provider
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let mut features = self.build_features(&event.model, &provider);
        features.insert(
            "type".to_string(),
            ArizeValue::String("embedding".to_string()),
        );

        let pending = PendingRequest {
            start_time: Self::current_timestamp_ms(),
            features,
        };

        let mut pending_requests = self.pending_requests.write().await;
        pending_requests.insert(event.request_id.clone(), pending);

        Ok(())
    }

    async fn on_embedding_end(&self, event: &EmbeddingEndEvent) -> IntegrationResult<()> {
        if !self.config.log_embeddings {
            return Ok(());
        }

        let pending = {
            let mut pending_requests = self.pending_requests.write().await;
            pending_requests.remove(&event.request_id)
        };

        let provider = event
            .provider
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let (start_time, mut features) = match pending {
            Some(p) => (p.start_time, p.features),
            None => {
                let mut f = self.build_features(&event.model, &provider);
                f.insert(
                    "type".to_string(),
                    ArizeValue::String("embedding".to_string()),
                );
                (Self::current_timestamp_ms() - event.latency_ms, f)
            }
        };

        if let Some(tokens) = event.total_tokens {
            features.insert(
                "total_tokens".to_string(),
                ArizeValue::Integer(tokens as i64),
            );
        }

        if let Some(cost) = event.cost_usd {
            features.insert("cost".to_string(), ArizeValue::Number(cost));
        }

        let record = ArizeRecord {
            prediction_id: event.request_id.clone(),
            model_id: self.config.model_id.clone(),
            model_version: self.config.model_version.clone(),
            environment: self.config.environment.clone(),
            timestamp_ms: start_time as i64,
            prediction_label: event.model.clone(),
            features,
            tags: self.build_tags(&[("status", "success"), ("type", "embedding")]),
            actual_label: None,
            latency_ms: Some(event.latency_ms),
        };

        let mut buffer = self.buffer.write().await;
        buffer.push(record);

        Ok(())
    }

    async fn on_cache_hit(&self, _event: &CacheHitEvent) -> IntegrationResult<()> {
        // Cache hits can be tracked as a separate metric if needed
        Ok(())
    }

    async fn flush(&self) -> IntegrationResult<()> {
        let records = {
            let mut buffer = self.buffer.write().await;
            std::mem::take(&mut *buffer)
        };

        if records.is_empty() {
            return Ok(());
        }

        debug!("Arize: Flushing {} records", records.len());
        self.send_records(records).await
    }

    async fn shutdown(&self) -> IntegrationResult<()> {
        info!("Arize integration shutting down");
        self.flush().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arize_config_builder() {
        let config = ArizeConfig::new("test-api-key", "test-space-key")
            .model_id("my-model")
            .model_version("1.0.0")
            .environment("staging")
            .tag("team", "ml");

        assert_eq!(config.api_key, "test-api-key");
        assert_eq!(config.space_key, "test-space-key");
        assert_eq!(config.model_id, "my-model");
        assert_eq!(config.model_version, Some("1.0.0".to_string()));
        assert_eq!(config.environment, "staging");
        assert_eq!(config.tags.get("team"), Some(&"ml".to_string()));
    }

    #[test]
    fn test_arize_config_default() {
        let config = ArizeConfig::default();

        assert_eq!(config.model_id, "litellm-gateway");
        assert_eq!(config.environment, "production");
        assert!(config.log_embeddings);
    }

    #[test]
    fn test_arize_integration_requires_api_key() {
        let config = ArizeConfig::default();
        let result = ArizeIntegration::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_arize_integration_requires_space_key() {
        let config = ArizeConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let result = ArizeIntegration::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_arize_integration_creation() {
        let config = ArizeConfig::new("test-api-key", "test-space-key");
        let result = ArizeIntegration::new(config);
        assert!(result.is_ok());

        let integration = result.unwrap();
        assert_eq!(integration.name(), "arize");
        assert!(integration.is_enabled());
    }

    #[test]
    fn test_build_features() {
        let config = ArizeConfig::new("test-key", "test-space");
        let integration = ArizeIntegration::new(config).unwrap();

        let features = integration.build_features("gpt-4", "openai");

        assert!(matches!(
            features.get("model"),
            Some(ArizeValue::String(s)) if s == "gpt-4"
        ));
        assert!(matches!(
            features.get("provider"),
            Some(ArizeValue::String(s)) if s == "openai"
        ));
    }

    #[test]
    fn test_build_tags() {
        let config = ArizeConfig::new("test-key", "test-space").tag("env", "test");
        let integration = ArizeIntegration::new(config).unwrap();

        let tags = integration.build_tags(&[("extra", "value")]);

        assert_eq!(tags.get("env"), Some(&"test".to_string()));
        assert_eq!(tags.get("extra"), Some(&"value".to_string()));
    }
}
