use crate::core::audio::types::{
    SpeechRequest, SpeechResponse, TranscriptionRequest, TranscriptionResponse, TranslationRequest,
    TranslationResponse,
};
use crate::core::providers::{Provider, ProviderError};
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::context::RequestContext;

impl Provider {
    /// Transcribe audio to text.
    ///
    /// Route selection must confirm `ProviderCapability::AudioTranscription`
    /// before calling this optional dispatch method.
    pub async fn audio_transcription(
        &self,
        request: TranscriptionRequest,
        context: RequestContext,
    ) -> Result<TranscriptionResponse, ProviderError> {
        dispatch_provider!(async_err, self, audio_transcription, request, context)
    }

    /// Translate audio to English text.
    ///
    /// Route selection must confirm `ProviderCapability::AudioTranslation`
    /// before calling this optional dispatch method.
    pub async fn audio_translation(
        &self,
        request: TranslationRequest,
        context: RequestContext,
    ) -> Result<TranslationResponse, ProviderError> {
        dispatch_provider!(async_err, self, audio_translation, request, context)
    }

    /// Generate speech audio from text.
    ///
    /// Route selection must confirm `ProviderCapability::TextToSpeech` before
    /// calling this optional dispatch method.
    pub async fn text_to_speech(
        &self,
        request: SpeechRequest,
        context: RequestContext,
    ) -> Result<SpeechResponse, ProviderError> {
        dispatch_provider!(async_err, self, text_to_speech, request, context)
    }
}
