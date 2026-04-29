//! LLM Provider sub-traits
//!
//! Focused capability interfaces extracted from the monolithic `LLMProvider` trait.
//! Each sub-trait groups related methods by concern:
//!
//! - [`LLMChat`] -- chat completion and request/response transformation
//! - [`LLMEmbed`] -- text embedding generation
//! - [`LLMStream`] -- streaming chat completion
//!
//! Blanket implementations are provided so that any type implementing
//! `LLMProvider` automatically satisfies these sub-traits. New code should
//! accept sub-trait bounds instead of the full `LLMProvider` where possible.

use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    responses::{ChatChunk, ChatResponse, EmbeddingResponse},
};

use super::trait_definition::LLMProvider;

// ---------------------------------------------------------------------------
// LLMChat -- chat completion methods
// ---------------------------------------------------------------------------

/// Chat completion capability.
///
/// Covers synchronous chat completion plus request/response transformation
/// and OpenAI-parameter mapping.
#[allow(async_fn_in_trait)]
pub trait LLMChat: Send + Sync {
    /// Execute chat completion request.
    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError>;

    /// Transform a standard ChatRequest into provider-specific format.
    async fn transform_request(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Value, ProviderError>;

    /// Transform raw provider response bytes into a ChatResponse.
    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, ProviderError>;

    /// Map OpenAI parameters to provider-specific parameters.
    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError>;

    /// Get supported OpenAI parameters for a model.
    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str];
}

/// Blanket implementation: every `LLMProvider` is automatically an `LLMChat`.
impl<T: LLMProvider> LLMChat for T {
    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        LLMProvider::chat_completion(self, request, context).await
    }

    async fn transform_request(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Value, ProviderError> {
        LLMProvider::transform_request(self, request, context).await
    }

    async fn transform_response(
        &self,
        raw_response: &[u8],
        model: &str,
        request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        LLMProvider::transform_response(self, raw_response, model, request_id).await
    }

    async fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        LLMProvider::map_openai_params(self, params, model).await
    }

    fn get_supported_openai_params(&self, model: &str) -> &'static [&'static str] {
        LLMProvider::get_supported_openai_params(self, model)
    }
}

// ---------------------------------------------------------------------------
// LLMEmbed -- embedding methods
// ---------------------------------------------------------------------------

/// Embedding capability.
///
/// Generate vector embeddings from text for semantic search, clustering, etc.
#[allow(async_fn_in_trait)]
pub trait LLMEmbed: Send + Sync {
    /// Generate text embeddings.
    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError>;
}

/// Blanket implementation: every `LLMProvider` is automatically an `LLMEmbed`.
impl<T: LLMProvider> LLMEmbed for T {
    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        LLMProvider::embeddings(self, request, context).await
    }
}

// ---------------------------------------------------------------------------
// LLMStream -- streaming methods
// ---------------------------------------------------------------------------

/// Streaming chat completion capability.
///
/// Returns response chunks as a stream for real-time processing via SSE.
#[allow(async_fn_in_trait)]
pub trait LLMStream: Send + Sync {
    /// Execute streaming chat completion request.
    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>;
}

/// Blanket implementation: every `LLMProvider` is automatically an `LLMStream`.
impl<T: LLMProvider> LLMStream for T {
    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        LLMProvider::chat_completion_stream(self, request, context).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::unified_provider::ProviderError;
    use crate::core::traits::error_mapper::trait_def::ErrorMapper;
    use crate::core::types::{
        health::HealthStatus,
        image::ImageGenerationRequest,
        model::{ModelInfo, ProviderCapability},
        responses::ImageGenerationResponse,
    };

    /// Minimal mock provider for testing blanket impls.
    #[derive(Debug)]
    struct MockProvider;

    #[allow(async_fn_in_trait)]
    impl LLMProvider for MockProvider {
        fn name(&self) -> &'static str {
            "mock"
        }
        fn capabilities(&self) -> &'static [ProviderCapability] {
            &[]
        }
        fn models(&self) -> &[ModelInfo] {
            &[]
        }
        fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
            &[]
        }
        async fn map_openai_params(
            &self,
            params: HashMap<String, Value>,
            _model: &str,
        ) -> Result<HashMap<String, Value>, ProviderError> {
            Ok(params)
        }
        async fn transform_request(
            &self,
            _request: ChatRequest,
            _context: RequestContext,
        ) -> Result<Value, ProviderError> {
            Ok(Value::Null)
        }
        async fn transform_response(
            &self,
            _raw: &[u8],
            _model: &str,
            _id: &str,
        ) -> Result<ChatResponse, ProviderError> {
            Err(ProviderError::not_supported("mock", "transform_response"))
        }
        fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
            Box::new(crate::core::traits::error_mapper::DefaultErrorMapper)
        }
        async fn chat_completion(
            &self,
            _request: ChatRequest,
            _context: RequestContext,
        ) -> Result<ChatResponse, ProviderError> {
            Err(ProviderError::not_supported("mock", "chat_completion"))
        }
        async fn image_generation(
            &self,
            _request: ImageGenerationRequest,
            _context: RequestContext,
        ) -> Result<ImageGenerationResponse, ProviderError> {
            Err(ProviderError::not_supported("mock", "image_generation"))
        }
        async fn health_check(&self) -> HealthStatus {
            HealthStatus::Healthy
        }
        async fn calculate_cost(
            &self,
            _model: &str,
            _input: u32,
            _output: u32,
        ) -> Result<f64, ProviderError> {
            Ok(0.0)
        }
    }

    #[tokio::test]
    async fn test_llm_chat_blanket_impl() {
        let provider = MockProvider;
        // LLMChat should be automatically available
        let result =
            LLMChat::chat_completion(&provider, ChatRequest::default(), RequestContext::default())
                .await;
        assert!(result.is_err()); // mock returns not_supported
    }

    #[tokio::test]
    async fn test_llm_embed_blanket_impl() {
        use crate::core::types::embedding::EmbeddingInput;

        let provider = MockProvider;
        let request = EmbeddingRequest {
            model: "test".to_string(),
            input: EmbeddingInput::Text("hello".to_string()),
            user: None,
            encoding_format: None,
            dimensions: None,
            task_type: None,
        };
        let result = LLMEmbed::embeddings(&provider, request, RequestContext::default()).await;
        assert!(result.is_err()); // default returns not_supported
    }

    #[tokio::test]
    async fn test_llm_stream_blanket_impl() {
        let provider = MockProvider;
        let result = LLMStream::chat_completion_stream(
            &provider,
            ChatRequest::default(),
            RequestContext::default(),
        )
        .await;
        assert!(result.is_err()); // default returns not_supported
    }

    /// Verify sub-traits can be used as trait bounds.
    fn _accepts_chat<T: LLMChat>(_t: &T) {}
    fn _accepts_embed<T: LLMEmbed>(_t: &T) {}
    fn _accepts_stream<T: LLMStream>(_t: &T) {}

    #[test]
    fn test_sub_trait_bounds() {
        let provider = MockProvider;
        _accepts_chat(&provider);
        _accepts_embed(&provider);
        _accepts_stream(&provider);
    }
}
