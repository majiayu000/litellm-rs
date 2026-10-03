//! In-process extension point for providers implemented outside this crate.
//!
//! Wrap an implementation in `Provider::External(Arc::new(provider))`, then use
//! `Deployment::new` and `UnifiedRouter::add_deployment` as for built-in providers.
//! Implementations own their transport and must return the existing ProviderError
//! without discarding status codes or retry hints. This API does not load plugins.

use super::{LLMProvider, ProviderError};
use crate::core::audio::types::{
    SpeechRequest, SpeechResponse, TranscriptionRequest, TranscriptionResponse, TranslationRequest,
    TranslationResponse,
};
use crate::core::traits::error_mapper::{DefaultErrorMapper, trait_def::ErrorMapper};
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    embedding::EmbeddingRequest,
    health::HealthStatus,
    image::{ImageEditRequest, ImageGenerationRequest},
    model::{ModelInfo, ProviderCapability},
    responses::{ChatChunk, ChatResponse, EmbeddingResponse, ImageGenerationResponse},
};
use futures::{future::BoxFuture, stream::BoxStream};
use serde_json::Value;
use std::{collections::HashMap, fmt::Debug, sync::Arc};

/// Object-safe execution interface for external Rust providers.
///
/// Declare only implemented capabilities, both for the provider and each model.
/// Optional methods fail with NotSupported until overridden. Futures and streams
/// must be Send so the existing router can execute them on its runtime.
pub trait ExternalProvider: Send + Sync + Debug + 'static {
    fn name(&self) -> &str;
    fn capabilities(&self) -> &'static [ProviderCapability];
    fn models(&self) -> &[ModelInfo];
    fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> BoxFuture<'_, Result<ChatResponse, ProviderError>>;

    fn health_check(&self) -> BoxFuture<'_, HealthStatus> {
        Box::pin(async { HealthStatus::Unknown })
    }

    /// SDK cost queries remain explicit; unknown pricing is never reported as free.
    fn calculate_cost<'a>(
        &'a self,
        _model: &'a str,
        _input_tokens: u32,
        _output_tokens: u32,
    ) -> BoxFuture<'a, Result<f64, ProviderError>> {
        Box::pin(async { Err(ProviderError::not_supported("external", "calculate_cost")) })
    }

    fn chat_completion_stream(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError>>
    {
        Box::pin(async {
            Err(ProviderError::not_supported(
                "external",
                "chat_completion_stream",
            ))
        })
    }

    fn embeddings(
        &self,
        _request: EmbeddingRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<EmbeddingResponse, ProviderError>> {
        Box::pin(async { Err(ProviderError::not_supported("external", "embeddings")) })
    }

    fn image_generation(
        &self,
        _request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<ImageGenerationResponse, ProviderError>> {
        Box::pin(async { Err(ProviderError::not_supported("external", "image_generation")) })
    }

    fn image_edit(
        &self,
        _request: ImageEditRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<ImageGenerationResponse, ProviderError>> {
        Box::pin(async { Err(ProviderError::not_supported("external", "image_edit")) })
    }

    fn audio_transcription(
        &self,
        _request: TranscriptionRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<TranscriptionResponse, ProviderError>> {
        Box::pin(async {
            Err(ProviderError::not_supported(
                "external",
                "audio_transcription",
            ))
        })
    }

    fn audio_translation(
        &self,
        _request: TranslationRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<TranslationResponse, ProviderError>> {
        Box::pin(async {
            Err(ProviderError::not_supported(
                "external",
                "audio_translation",
            ))
        })
    }

    fn text_to_speech(
        &self,
        _request: SpeechRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<SpeechResponse, ProviderError>> {
        Box::pin(async { Err(ProviderError::not_supported("external", "text_to_speech")) })
    }
}

impl LLMProvider for Arc<dyn ExternalProvider> {
    fn name(&self) -> &str {
        self.as_ref().name()
    }
    fn capabilities(&self) -> &'static [ProviderCapability] {
        self.as_ref().capabilities()
    }
    fn models(&self) -> &[ModelInfo] {
        self.as_ref().models()
    }
    fn error_provider_name(&self) -> &'static str {
        "external"
    }
    fn get_supported_openai_params(&self, _model: &str) -> &'static [&'static str] {
        &[]
    }
    async fn map_openai_params(
        &self,
        _params: HashMap<String, Value>,
        _model: &str,
    ) -> Result<HashMap<String, Value>, ProviderError> {
        Err(ProviderError::not_supported(
            "external",
            "legacy parameter mapping",
        ))
    }
    async fn transform_request(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Value, ProviderError> {
        Err(ProviderError::not_supported(
            "external",
            "legacy request transformation",
        ))
    }
    async fn transform_response(
        &self,
        _raw: &[u8],
        _model: &str,
        _request_id: &str,
    ) -> Result<ChatResponse, ProviderError> {
        Err(ProviderError::not_supported(
            "external",
            "legacy response transformation",
        ))
    }
    fn get_error_mapper(&self) -> Box<dyn ErrorMapper<ProviderError>> {
        Box::new(DefaultErrorMapper)
    }
    async fn health_check(&self) -> HealthStatus {
        self.as_ref().health_check().await
    }
    async fn calculate_cost(
        &self,
        model: &str,
        input: u32,
        output: u32,
    ) -> Result<f64, ProviderError> {
        self.as_ref().calculate_cost(model, input, output).await
    }

    async fn chat_completion(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        self.as_ref().chat_completion(request, context).await
    }

    async fn chat_completion_stream(
        &self,
        request: ChatRequest,
        context: RequestContext,
    ) -> Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError> {
        self.as_ref().chat_completion_stream(request, context).await
    }

    async fn embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> Result<EmbeddingResponse, ProviderError> {
        self.as_ref().embeddings(request, context).await
    }

    async fn image_generation(
        &self,
        request: ImageGenerationRequest,
        context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        self.as_ref().image_generation(request, context).await
    }

    async fn image_edit(
        &self,
        request: ImageEditRequest,
        context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        self.as_ref().image_edit(request, context).await
    }

    async fn audio_transcription(
        &self,
        request: TranscriptionRequest,
        context: RequestContext,
    ) -> Result<TranscriptionResponse, ProviderError> {
        self.as_ref().audio_transcription(request, context).await
    }

    async fn audio_translation(
        &self,
        request: TranslationRequest,
        context: RequestContext,
    ) -> Result<TranslationResponse, ProviderError> {
        self.as_ref().audio_translation(request, context).await
    }

    async fn text_to_speech(
        &self,
        request: SpeechRequest,
        context: RequestContext,
    ) -> Result<SpeechResponse, ProviderError> {
        self.as_ref().text_to_speech(request, context).await
    }
}
