//! LLM Provider trait definition
//!
//! Defines the unified interface for all AI providers

use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;
use std::pin::Pin;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::error_mapper::trait_def::ErrorMapper;
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    image::ImageGenerationRequest,
    model::ModelInfo,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse, EmbeddingResponse, ImageGenerationResponse},
};

/// Unified LLM Provider interface
///
/// This is the core abstraction of LiteLLM, all AI providers must implement this trait
///
/// # Design Principles
///
/// 1. **Request uniformity**: All providers use the same request/response format
/// 2. **Capability driven**: Declare supported features through capabilities()
/// 3. **Provider agnostic**: Users don't need to know provider-specific details
/// 4. **Type safety**: Use `ProviderError` as the unified error type
/// 5. **Async first**: All I/O operations are asynchronous
/// 6. **Observability**: Built-in cost calculation, latency statistics, and monitoring
///
/// # Example
///
/// The `LLMProvider` trait is the core abstraction for AI providers. All providers
/// use `ProviderError` as the unified error type. See existing provider
/// implementations in `src/core/providers/` for reference.
#[allow(async_fn_in_trait)]
pub trait LLMProvider: Send + Sync + Debug + 'static {
    // ==================== Basic Metadata ====================

    /// Get provider name
    ///
    /// # Returns
    /// String identifier for the provider, such as "openai", "anthropic", "v0", etc.
    ///
    /// # Note
    /// This name is used for routing and logging, must be unique across the entire system.
    /// Implementations may return either a static literal or a value borrowed from `self`.
    fn name(&self) -> &str;

    /// Static provider name used by legacy `ProviderError` constructors.
    ///
    /// Providers with dynamic names should return a stable family name here and
    /// keep the specific provider identity in `name()`.
    fn error_provider_name(&self) -> &'static str {
        "provider"
    }

    /// Get provider capabilities
    ///
    /// # Returns
    /// Static capability list for quickly querying which features this provider supports
    ///
    /// # Use Cases
    /// - Request routing: Route requests only to compatible providers
    /// - Feature detection: Check if specific functionality is available
    /// - UI display: Show provider feature characteristics
    fn capabilities(&self) -> &'static [ProviderCapability];

    /// Get supported model list
    ///
    /// # Returns
    /// List of models supported by this provider
    ///
    /// # Implementation Suggestions
    /// - Load from configuration file or remote API
    /// - Include model metadata like context length, cost, etc.
    /// - Recommend caching for performance
    fn models(&self) -> &[ModelInfo];

    // ==================== Capability Query Methods ====================

    /// Check if model is supported
    ///
    /// # Parameters
    /// * `model` - Model name to check
    ///
    /// # Returns
    /// True if the model is supported by this provider
    ///
    /// # Default Implementation
    /// Searches in the list returned by models()
    fn supports_model(&self, model: &str) -> bool {
        self.models().iter().any(|m| m.id == model)
    }

    /// Check if tools are supported
    ///
    /// # Returns
    /// True if tool calling is supported
    ///
    /// # Default Implementation
    /// Checks if capabilities contain ToolCalling
    fn supports_tools(&self) -> bool {
        self.capabilities()
            .contains(&ProviderCapability::ToolCalling)
    }

    /// Check if streaming is supported
    ///
    /// # Returns
    /// True if Server-Sent Events streaming output is supported
    ///
    /// # Default Implementation
    /// Checks if capabilities contain ChatCompletionStream
    fn supports_streaming(&self) -> bool {
        self.capabilities()
            .contains(&ProviderCapability::ChatCompletionStream)
    }

    /// Check if image generation is supported
    ///
    /// # Returns
    /// True if image generation (like DALL-E) is supported
    fn supports_image_generation(&self) -> bool {
        self.capabilities()
            .contains(&ProviderCapability::ImageGeneration)
    }

    /// Check if embeddings are supported
    ///
    /// # Returns
    /// True if text embedding generation is supported
    fn supports_embeddings(&self) -> bool {
        self.capabilities()
            .contains(&ProviderCapability::Embeddings)
    }

    /// Check if vision capabilities are supported
    ///
    /// # Returns
    /// True if image analysis/vision is supported
    fn supports_vision(&self) -> bool {
        // Currently returns false, as ProviderCapability doesn't have Vision variant
        false
    }

    // ==================== Python LiteLLM Compatible Interface ====================

    /// Get supported OpenAI parameters
    ///
    /// Returns all OpenAI standard parameter names supported by this provider
    ///
    /// # Parameters
    /// * `model` - Model name to check parameters for
    ///
    /// # Returns
    /// List of supported parameter names
    ///
    /// # Example
    ///
    /// OpenAI provider might return:
    /// `["temperature", "max_tokens", "top_p", "frequency_penalty", "presence_penalty", "tools"]`
    ///
    /// Anthropic provider might return:
    /// `["temperature", "max_tokens", "top_p", "top_k", "tools"]`
    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str];

    /// Map OpenAI parameters to provider-specific parameters
    ///
    /// Convert standard OpenAI parameters to format understood by this provider
    ///
    /// # Parameters
    /// * `params` - Input parameter mapping (OpenAI format)
    /// * `model` - Target model name
    ///
    /// # Returns
    /// Converted parameter mapping (provider-specific format)
    ///
    /// # Example
    ///
    /// For Anthropic provider:
    /// - input: `{"max_tokens": 100, "temperature": 0.7}`
    /// - output: `{"max_tokens_to_sample": 100, "temperature": 0.7}`
    ///
    /// For Azure provider:
    /// - input: `{"user": "alice", "stream": true}`
    /// - output: `{"end_user_id": "alice", "stream": true}`
    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError>;

    /// Transform request format
    ///
    /// Convert standard ChatRequest to provider-specific request format
    ///
    /// # Parameters
    /// * `request` - Standard chat request
    /// * `context` - Request context with metadata
    ///
    /// # Returns
    /// Provider-specific request as JSON Value
    ///
    /// # Implementation Notes
    /// This method should:
    /// 1. Validate request parameters
    /// 2. Convert message format
    /// 3. Map model names if needed
    /// 4. Handle provider-specific options
    /// 5. Set authentication headers
    async fn transform_request(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Value, ProviderError>;

    /// Transform response format
    ///
    /// Convert provider-specific response to standard ChatResponse format
    ///
    /// # Parameters
    /// * `raw_response` - Raw response bytes from provider
    /// * `model` - Model name used for the request
    /// * `request_id` - Unique request identifier
    ///
    /// # Returns
    /// Standardized chat response
    ///
    /// # Implementation Notes
    /// This method should:
    /// 1. Parse provider response format
    /// 2. Extract choices and messages
    /// 3. Convert tool call format
    /// 4. Calculate token usage
    /// 5. Handle error cases gracefully
    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, ProviderError>;

    /// Get error mapper instance
    ///
    /// Returns the error mapper for this provider
    ///
    /// # Returns
    /// Error mapper that handles provider-specific error formats
    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>>;

    // ==================== Core Functionality: Chat Completion ====================

    /// Execute chat completion request
    ///
    /// This is the core method that all LLM providers must implement
    ///
    /// # Parameters
    /// * `request` - Chat completion request
    /// * `context` - Request context with metadata
    ///
    /// # Returns
    /// Chat completion response
    ///
    /// # Errors
    /// * `ProviderError::authentication()` - Authentication failed
    /// * `ProviderError::not_supported()` - Model or feature not supported
    /// * `ProviderError::network_error()` - Network or API error
    /// * `ProviderError::rate_limit()` - Rate limit exceeded
    /// * `ProviderError::parsing_error()` - Response parsing failed
    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError>;

    /// Execute streaming chat completion request
    ///
    /// Returns response chunks as a stream for real-time processing
    ///
    /// # Parameters
    /// * `request` - Chat completion request
    /// * `context` - Request context with metadata
    ///
    /// # Returns
    /// Stream where each item is a ChatChunk
    ///
    /// # Default Implementation
    /// Returns not supported error
    ///
    /// # Note
    /// Should only be called when supports_streaming() returns true
    async fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        Err(ProviderError::not_supported(
            self.error_provider_name(),
            "streaming",
        ))
    }

    // ==================== Optional Features ====================

    /// Generate text embeddings
    ///
    /// Convert text to high-dimensional vectors for semantic search, clustering, and other applications
    ///
    /// # Parameters
    /// * `request` - Embedding request with input text
    /// * `context` - Request context with metadata
    ///
    /// # Returns
    /// Embedding response with vectors
    ///
    /// # Default Implementation
    /// Returns not supported error
    ///
    /// # Use Cases
    /// - Semantic search
    /// - Document similarity calculation
    /// - Recommendation systems
    /// - RAG (Retrieval Augmented Generation) systems
    async fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::not_supported(
            self.error_provider_name(),
            "embeddings",
        ))
    }

    /// Generate images
    ///
    /// Generate images based on text descriptions
    ///
    /// # Parameters
    /// * `request` - Image generation request
    /// * `context` - Request context with metadata
    ///
    /// # Returns
    /// Image generation response with URLs or data
    ///
    /// # Default Implementation
    /// Returns not supported error
    ///
    /// # Supported Models
    /// - OpenAI DALL-E series
    /// - Midjourney (via proxy)
    /// - Stable Diffusion
    async fn image_generation(
        &self,
        _request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        Err(ProviderError::not_supported(
            self.error_provider_name(),
            "image_generation",
        ))
    }

    // ==================== Health Monitoring ====================

    /// Check provider health status
    ///
    /// Validate that the provider is operational and responding correctly
    ///
    /// # Returns
    /// HealthStatus enum containing Healthy, Degraded, Unhealthy, etc. states
    ///
    /// # Implementation Suggestions
    /// - Test API connectivity
    /// - Validate authentication
    /// - Send lightweight test request
    /// - Check rate limit status
    ///
    /// # Use Cases
    /// - Load balancer health checks
    /// - Service discovery updates
    /// - Failover decision making
    /// - Monitoring and alerting
    async fn health_check(&self) -> HealthStatus;

    // ==================== Cost Management ====================

    /// Calculate request cost
    ///
    /// # Parameters
    /// * `model` - Model name used
    /// * `input_tokens` - Number of input tokens
    /// * `output_tokens` - Number of output tokens
    ///
    /// # Returns
    /// Estimated cost in USD
    ///
    /// # Use Cases
    /// - Cost control and budget management
    /// - User quota management
    /// - Cost optimization decisions
    /// - Billing and statistics
    async fn calculate_cost(
        &self,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Result<f64, ProviderError>;

    // ==================== Performance Metrics ====================

    /// Get average response latency
    ///
    /// # Returns
    /// Average latency for this provider
    ///
    /// # Default Implementation
    /// Returns 100ms, subclasses should override with actual statistics
    ///
    /// # Use Cases
    /// - Route selection: Prefer providers with lower latency
    /// - Performance benchmarking
    /// - Performance monitoring and optimization
    async fn get_average_latency(&self) -> Result<std::time::Duration, ProviderError> {
        Ok(std::time::Duration::from_millis(100))
    }

    /// Get success rate
    ///
    /// # Returns
    /// Success rate between 0.0 and 1.0
    ///
    /// # Default Implementation
    /// Returns 0.99 (99% success rate)
    ///
    /// # Use Cases
    /// - Service quality assessment
    /// - Automatic failover
    /// - SLA monitoring
    async fn get_success_rate(&self) -> Result<f32, ProviderError> {
        Ok(0.99)
    }

    // ==================== Utility Methods ====================

    /// Estimate token count for text
    ///
    /// # Parameters
    /// * `text` - Text to analyze
    ///
    /// # Returns
    /// Estimated token count
    ///
    /// # Default Implementation
    /// Uses simple heuristic: approximately 4 characters equals 1 token
    ///
    /// # Implementation Suggestions
    /// Use model-specific tokenizers when possible:
    /// - OpenAI: Use tiktoken library
    /// - Anthropic: Use Claude tokenizer
    /// - Others: Can use online API or simple estimation
    ///
    /// # Use Cases
    /// - Pre-request validation
    /// - Cost estimation
    /// - Context length management
    async fn estimate_tokens(&self, text: &str) -> Result<u32, ProviderError> {
        // Simple estimation: 4 characters approximately equals 1 token
        // Subclasses should implement more accurate tokenization
        Ok((text.len() as f64 / 4.0).ceil() as u32)
    }
}
