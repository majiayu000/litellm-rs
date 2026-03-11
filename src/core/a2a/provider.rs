//! A2A Provider Adapters
//!
//! Platform-specific adapters for different agent providers.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

use super::config::{AgentConfig, AgentProvider};
use super::error::{A2AError, A2AResult};
use super::message::{A2AMessage, A2AResponse, TaskResult};
use crate::utils::net::http::get_client_with_timeout;

/// Trait for A2A provider implementations
#[async_trait]
pub trait A2AProviderAdapter: Send + Sync {
    /// Get provider type
    fn provider_type(&self) -> AgentProvider;

    /// Send a message to the agent
    async fn send_message(
        &self,
        config: &AgentConfig,
        message: A2AMessage,
    ) -> A2AResult<A2AResponse>;

    /// Get task status
    async fn get_task(&self, config: &AgentConfig, task_id: &str) -> A2AResult<TaskResult>;

    /// Cancel a task
    async fn cancel_task(&self, config: &AgentConfig, task_id: &str) -> A2AResult<()>;

    /// Check if provider supports streaming
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Check if provider supports async tasks
    fn supports_async_tasks(&self) -> bool {
        true
    }
}

/// Generic A2A provider (standard JSON-RPC 2.0)
///
/// Uses a shared HTTP client pool for optimal connection reuse.
pub struct GenericA2AProvider {
    /// Cached client for the default timeout (60s)
    default_client: Arc<reqwest::Client>,
}

impl GenericA2AProvider {
    /// Create a new generic provider with shared HTTP client
    pub fn new() -> Self {
        // Use the shared client pool with default A2A timeout (60s)
        Self {
            default_client: get_client_with_timeout(Duration::from_secs(60)),
        }
    }

    /// Create with custom HTTP client (for testing)
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            default_client: Arc::new(client),
        }
    }

    /// Get the appropriate client for the given timeout
    fn get_client(&self, timeout_ms: u64) -> Arc<reqwest::Client> {
        let timeout_secs = timeout_ms / 1000;
        if timeout_secs == 60 {
            // Use the default cached client
            self.default_client.clone()
        } else {
            // Get from global cache for other timeouts
            get_client_with_timeout(Duration::from_secs(timeout_secs))
        }
    }

    /// Build request with authentication
    fn build_request(&self, config: &AgentConfig, message: &A2AMessage) -> reqwest::RequestBuilder {
        let client = self.get_client(config.timeout_ms);
        let mut request = client.post(&config.url).json(message);

        // Add API key if present
        if let Some(ref api_key) = config.api_key {
            request = request.bearer_auth(api_key);
        }

        // Add custom headers
        for (key, value) in &config.headers {
            request = request.header(key, value);
        }

        request
    }
}

impl Default for GenericA2AProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl A2AProviderAdapter for GenericA2AProvider {
    fn provider_type(&self) -> AgentProvider {
        AgentProvider::A2A
    }

    async fn send_message(
        &self,
        config: &AgentConfig,
        message: A2AMessage,
    ) -> A2AResult<A2AResponse> {
        let response = self
            .build_request(config, &message)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    A2AError::Timeout {
                        agent_name: config.name.clone(),
                        timeout_ms: config.timeout_ms,
                    }
                } else if e.is_connect() {
                    A2AError::ConnectionError {
                        agent_name: config.name.clone(),
                        message: e.to_string(),
                    }
                } else {
                    A2AError::ProtocolError {
                        message: e.to_string(),
                    }
                }
            })?;

        let status = response.status();
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(A2AError::AuthenticationError {
                agent_name: config.name.clone(),
                message: "Unauthorized".to_string(),
            });
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .map(|s| s * 1000);

            return Err(A2AError::RateLimitExceeded {
                agent_name: config.name.clone(),
                retry_after_ms: retry_after,
            });
        }

        let a2a_response: A2AResponse =
            response.json().await.map_err(|e| A2AError::ProtocolError {
                message: format!("Failed to parse response: {}", e),
            })?;

        Ok(a2a_response)
    }

    async fn get_task(&self, config: &AgentConfig, task_id: &str) -> A2AResult<TaskResult> {
        let message = A2AMessage::get_task(task_id);
        let response = self.send_message(config, message).await?;

        response.result.ok_or_else(|| {
            if let Some(error) = response.error {
                if error.code == -32001 {
                    A2AError::TaskNotFound {
                        agent_name: config.name.clone(),
                        task_id: task_id.to_string(),
                    }
                } else {
                    A2AError::ProtocolError {
                        message: error.message,
                    }
                }
            } else {
                A2AError::ProtocolError {
                    message: "Empty response".to_string(),
                }
            }
        })
    }

    async fn cancel_task(&self, config: &AgentConfig, task_id: &str) -> A2AResult<()> {
        let message = A2AMessage::cancel_task(task_id);
        let response = self.send_message(config, message).await?;

        if response.is_error()
            && let Some(error) = response.error
        {
            return Err(A2AError::TaskFailed {
                agent_name: config.name.clone(),
                task_id: task_id.to_string(),
                message: error.message,
            });
        }

        Ok(())
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_async_tasks(&self) -> bool {
        true
    }
}

/// LangGraph provider adapter
pub struct LangGraphProvider {
    inner: GenericA2AProvider,
}

impl LangGraphProvider {
    pub fn new() -> Self {
        Self {
            inner: GenericA2AProvider::new(),
        }
    }
}

impl Default for LangGraphProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl A2AProviderAdapter for LangGraphProvider {
    fn provider_type(&self) -> AgentProvider {
        AgentProvider::LangGraph
    }

    async fn send_message(
        &self,
        config: &AgentConfig,
        message: A2AMessage,
    ) -> A2AResult<A2AResponse> {
        // LangGraph uses standard A2A protocol
        self.inner.send_message(config, message).await
    }

    async fn get_task(&self, config: &AgentConfig, task_id: &str) -> A2AResult<TaskResult> {
        self.inner.get_task(config, task_id).await
    }

    async fn cancel_task(&self, config: &AgentConfig, task_id: &str) -> A2AResult<()> {
        self.inner.cancel_task(config, task_id).await
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn supports_async_tasks(&self) -> bool {
        true
    }
}

/// Get provider adapter for a given agent type
pub fn get_provider_adapter(provider: AgentProvider) -> Arc<dyn A2AProviderAdapter> {
    match provider {
        AgentProvider::LangGraph => Arc::new(LangGraphProvider::new()),
        AgentProvider::VertexAI => Arc::new(GenericA2AProvider::new()), // TODO: Add specific adapter
        AgentProvider::AzureAIFoundry => Arc::new(GenericA2AProvider::new()),
        AgentProvider::BedrockAgentCore => Arc::new(GenericA2AProvider::new()),
        AgentProvider::PydanticAI => Arc::new(GenericA2AProvider::new()),
        AgentProvider::A2A | AgentProvider::Custom => Arc::new(GenericA2AProvider::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== GenericA2AProvider Tests ====================

    #[test]
    fn test_generic_provider_creation() {
        let provider = GenericA2AProvider::new();
        assert_eq!(provider.provider_type(), AgentProvider::A2A);
        assert!(provider.supports_streaming());
        assert!(provider.supports_async_tasks());
    }

    #[test]
    fn test_generic_provider_default() {
        let provider = GenericA2AProvider::default();
        assert_eq!(provider.provider_type(), AgentProvider::A2A);
    }

    #[test]
    fn test_generic_provider_with_custom_client() {
        let client = reqwest::Client::new();
        let provider = GenericA2AProvider::with_client(client);
        assert_eq!(provider.provider_type(), AgentProvider::A2A);
    }

    #[test]
    fn test_generic_provider_get_client_default_timeout() {
        let provider = GenericA2AProvider::new();
        // 60 seconds is the default, should return cached client
        let client = provider.get_client(60000);
        assert!(Arc::ptr_eq(&client, &provider.default_client));
    }

    #[test]
    fn test_generic_provider_get_client_custom_timeout() {
        let provider = GenericA2AProvider::new();
        // Non-60s timeout should get different client from cache
        let client = provider.get_client(30000);
        // Should not be the same as default
        assert!(!Arc::ptr_eq(&client, &provider.default_client));
    }

    // ==================== LangGraphProvider Tests ====================

    #[test]
    fn test_langgraph_provider_creation() {
        let provider = LangGraphProvider::new();
        assert_eq!(provider.provider_type(), AgentProvider::LangGraph);
    }

    #[test]
    fn test_langgraph_provider_default() {
        let provider = LangGraphProvider::default();
        assert_eq!(provider.provider_type(), AgentProvider::LangGraph);
    }

    #[test]
    fn test_langgraph_provider_streaming_support() {
        let provider = LangGraphProvider::new();
        assert!(provider.supports_streaming());
    }

    #[test]
    fn test_langgraph_provider_async_tasks_support() {
        let provider = LangGraphProvider::new();
        assert!(provider.supports_async_tasks());
    }

    // ==================== get_provider_adapter Tests ====================

    #[test]
    fn test_get_provider_adapter() {
        let adapter = get_provider_adapter(AgentProvider::LangGraph);
        assert_eq!(adapter.provider_type(), AgentProvider::LangGraph);

        let adapter = get_provider_adapter(AgentProvider::A2A);
        assert_eq!(adapter.provider_type(), AgentProvider::A2A);
    }

    #[test]
    fn test_get_provider_adapter_vertex_ai() {
        let adapter = get_provider_adapter(AgentProvider::VertexAI);
        // Currently uses GenericA2AProvider which returns A2A
        assert_eq!(adapter.provider_type(), AgentProvider::A2A);
    }

    #[test]
    fn test_get_provider_adapter_azure_ai_foundry() {
        let adapter = get_provider_adapter(AgentProvider::AzureAIFoundry);
        assert_eq!(adapter.provider_type(), AgentProvider::A2A);
    }

    #[test]
    fn test_get_provider_adapter_bedrock_agent_core() {
        let adapter = get_provider_adapter(AgentProvider::BedrockAgentCore);
        assert_eq!(adapter.provider_type(), AgentProvider::A2A);
    }

    #[test]
    fn test_get_provider_adapter_pydantic_ai() {
        let adapter = get_provider_adapter(AgentProvider::PydanticAI);
        assert_eq!(adapter.provider_type(), AgentProvider::A2A);
    }

    #[test]
    fn test_get_provider_adapter_custom() {
        let adapter = get_provider_adapter(AgentProvider::Custom);
        assert_eq!(adapter.provider_type(), AgentProvider::A2A);
    }

    // ==================== Provider Adapter Capabilities Tests ====================

    #[test]
    fn test_all_adapters_support_async_tasks() {
        let providers = [
            AgentProvider::LangGraph,
            AgentProvider::VertexAI,
            AgentProvider::AzureAIFoundry,
            AgentProvider::BedrockAgentCore,
            AgentProvider::PydanticAI,
            AgentProvider::A2A,
            AgentProvider::Custom,
        ];

        for provider in providers {
            let adapter = get_provider_adapter(provider);
            assert!(
                adapter.supports_async_tasks(),
                "Provider {:?} should support async tasks",
                provider
            );
        }
    }

    #[test]
    fn test_all_adapters_support_streaming() {
        let providers = [AgentProvider::LangGraph, AgentProvider::A2A];

        for provider in providers {
            let adapter = get_provider_adapter(provider);
            assert!(
                adapter.supports_streaming(),
                "Provider {:?} should support streaming",
                provider
            );
        }
    }

    // ==================== Build Request Tests ====================

    #[test]
    fn test_build_request_basic() {
        let provider = GenericA2AProvider::new();
        let config = AgentConfig::new("test-agent", "https://example.com/api");
        let message = A2AMessage::send("Hello, agent!");

        let request = provider.build_request(&config, &message);
        // Request should be built without error
        let _ = request;
    }

    #[test]
    fn test_build_request_with_api_key() {
        let provider = GenericA2AProvider::new();
        let mut config = AgentConfig::new("test-agent", "https://example.com/api");
        config.api_key = Some("test-api-key".to_string());
        let message = A2AMessage::send("Hello!");

        let request = provider.build_request(&config, &message);
        let _ = request;
    }

    #[test]
    fn test_build_request_with_headers() {
        let provider = GenericA2AProvider::new();
        let mut config = AgentConfig::new("test-agent", "https://example.com/api");
        config
            .headers
            .insert("X-Custom-Header".to_string(), "custom-value".to_string());
        let message = A2AMessage::send("Hello!");

        let request = provider.build_request(&config, &message);
        let _ = request;
    }

    #[test]
    fn test_build_request_with_multiple_headers() {
        let provider = GenericA2AProvider::new();
        let mut config = AgentConfig::new("test-agent", "https://example.com/api");
        config
            .headers
            .insert("X-Header-1".to_string(), "value1".to_string());
        config
            .headers
            .insert("X-Header-2".to_string(), "value2".to_string());
        config
            .headers
            .insert("X-Header-3".to_string(), "value3".to_string());
        let message = A2AMessage::send("Hello!");

        let request = provider.build_request(&config, &message);
        let _ = request;
    }

    // ==================== A2AMessage Factory Tests ====================

    #[test]
    fn test_a2a_message_send() {
        let message = A2AMessage::send("Test content");
        assert_eq!(message.method, "message/send");
        assert!(message.params.is_some());
    }

    #[test]
    fn test_a2a_message_get_task() {
        let message = A2AMessage::get_task("task-123");
        assert_eq!(message.method, "tasks/get");
        assert!(message.params.is_some());
    }

    #[test]
    fn test_a2a_message_cancel_task() {
        let message = A2AMessage::cancel_task("task-456");
        assert_eq!(message.method, "tasks/cancel");
        assert!(message.params.is_some());
    }

    // ==================== Provider Type Coverage ====================

    #[test]
    fn test_provider_type_generic() {
        let provider = GenericA2AProvider::new();
        assert_eq!(provider.provider_type(), AgentProvider::A2A);
    }

    #[test]
    fn test_provider_type_langgraph() {
        let provider = LangGraphProvider::new();
        assert_eq!(provider.provider_type(), AgentProvider::LangGraph);
    }

    // ==================== Default Trait Implementations ====================

    #[test]
    fn test_trait_default_streaming() {
        // Test the default implementation returns false
        struct TestProvider;
        impl A2AProviderAdapter for TestProvider {
            fn provider_type(&self) -> AgentProvider {
                AgentProvider::Custom
            }
            fn send_message<'life0, 'life1, 'async_trait>(
                &'life0 self,
                _config: &'life1 AgentConfig,
                _message: A2AMessage,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = A2AResult<A2AResponse>> + Send + 'async_trait>,
            >
            where
                'life0: 'async_trait,
                'life1: 'async_trait,
                Self: 'async_trait,
            {
                Box::pin(async {
                    Err(A2AError::ProtocolError {
                        message: "not implemented".to_string(),
                    })
                })
            }
            fn get_task<'life0, 'life1, 'life2, 'async_trait>(
                &'life0 self,
                _config: &'life1 AgentConfig,
                _task_id: &'life2 str,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = A2AResult<TaskResult>> + Send + 'async_trait>,
            >
            where
                'life0: 'async_trait,
                'life1: 'async_trait,
                'life2: 'async_trait,
                Self: 'async_trait,
            {
                Box::pin(async {
                    Err(A2AError::ProtocolError {
                        message: "not implemented".to_string(),
                    })
                })
            }
            fn cancel_task<'life0, 'life1, 'life2, 'async_trait>(
                &'life0 self,
                _config: &'life1 AgentConfig,
                _task_id: &'life2 str,
            ) -> std::pin::Pin<
                Box<dyn std::future::Future<Output = A2AResult<()>> + Send + 'async_trait>,
            >
            where
                'life0: 'async_trait,
                'life1: 'async_trait,
                'life2: 'async_trait,
                Self: 'async_trait,
            {
                Box::pin(async {
                    Err(A2AError::ProtocolError {
                        message: "not implemented".to_string(),
                    })
                })
            }
        }

        let provider = TestProvider;
        // Default implementations
        assert!(!provider.supports_streaming());
        assert!(provider.supports_async_tasks());
    }
}
