//! OpenAI model registry type definitions
//!
//! Contains enums and structs used by the model registry:
//! - `OpenAIModelFeature` — feature flags for model capabilities
//! - `OpenAIModelFamily` — model family classification
//! - `OpenAIModelSpec` / `OpenAIModelConfig` — per-model metadata
//! - `OpenAIUseCase` — recommendation use cases

use std::collections::HashMap;

use crate::core::types::{model::ModelInfo, model::ProviderCapability};

/// OpenAI-specific model features
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OpenAIModelFeature {
    /// Chat completion support
    ChatCompletion,
    /// Streaming response support
    StreamingSupport,
    /// Function/tool calling support
    FunctionCalling,
    /// Vision support (multimodal)
    VisionSupport,
    /// System message support
    SystemMessages,
    /// JSON mode support
    JsonMode,
    /// O-series reasoning mode
    ReasoningMode,
    /// Audio input support
    AudioInput,
    /// Audio output support (TTS)
    AudioOutput,
    /// Image generation (DALL-E)
    ImageGeneration,
    /// Image editing
    ImageEditing,
    /// Audio transcription
    AudioTranscription,
    /// Fine-tuning support
    FineTuning,
    /// Embeddings generation
    Embeddings,
    /// Code completion optimized
    CodeCompletion,
    /// High context window (>32K)
    LargeContext,
    /// Real-time audio processing
    RealtimeAudio,
}

impl OpenAIModelFeature {
    /// Convert OpenAI model feature to provider capability
    pub fn to_provider_capability(&self) -> Option<ProviderCapability> {
        match self {
            OpenAIModelFeature::ChatCompletion => Some(ProviderCapability::ChatCompletion),
            OpenAIModelFeature::StreamingSupport => Some(ProviderCapability::ChatCompletionStream),
            OpenAIModelFeature::FunctionCalling => Some(ProviderCapability::ToolCalling),
            OpenAIModelFeature::ImageGeneration => Some(ProviderCapability::ImageGeneration),
            OpenAIModelFeature::AudioTranscription => Some(ProviderCapability::AudioTranscription),
            OpenAIModelFeature::Embeddings => Some(ProviderCapability::Embeddings),
            OpenAIModelFeature::AudioOutput => Some(ProviderCapability::TextToSpeech),
            OpenAIModelFeature::ImageEditing => Some(ProviderCapability::ImageEdit),
            // Features that don't map directly to provider capabilities
            OpenAIModelFeature::SystemMessages
            | OpenAIModelFeature::JsonMode
            | OpenAIModelFeature::ReasoningMode
            | OpenAIModelFeature::VisionSupport
            | OpenAIModelFeature::AudioInput
            | OpenAIModelFeature::FineTuning
            | OpenAIModelFeature::CodeCompletion
            | OpenAIModelFeature::LargeContext
            | OpenAIModelFeature::RealtimeAudio => None,
        }
    }
}

/// OpenAI model specification
#[derive(Debug, Clone)]
pub struct OpenAIModelSpec {
    /// Basic model information
    pub model_info: ModelInfo,
    /// Supported features
    pub features: Vec<OpenAIModelFeature>,
    /// Model family (gpt-4, gpt-3.5, dalle, whisper, etc.)
    pub family: OpenAIModelFamily,
    /// Model configuration
    pub config: OpenAIModelConfig,
}

/// OpenAI model families
#[derive(Debug, Clone, PartialEq)]
pub enum OpenAIModelFamily {
    GPT4,
    GPT4Turbo,
    GPT4O,
    GPT4OMini,
    GPT41,
    GPT41Mini,
    GPT41Nano,
    GPT35,
    GPT5,          // GPT-5 models (2025)
    GPT5Mini,      // GPT-5 Mini models (2025)
    GPT5Nano,      // GPT-5 Nano models (2025)
    GPT51,         // GPT-5.1 models (Nov 2025)
    GPT51Thinking, // GPT-5.1 Thinking mode (Nov 2025)
    GPT52,         // GPT-5.2 models (2025)
    GPT52Pro,      // GPT-5.2 Pro models (2025)
    GPT52Codex,    // GPT-5.2 Codex models (2025)
    O1,            // O1 reasoning models
    O1Pro,         // O1 Pro reasoning models
    O3,            // O3 reasoning models (2025)
    O3Pro,         // O3 Pro reasoning models
    O3Mini,        // O3 Mini reasoning models
    O4Mini,        // O4 Mini reasoning models (2025)
    DALLE2,
    DALLE3,
    Whisper,
    TTS,
    Embedding,
    Moderation,
    GPT4OAudio, // GPT-4O with audio capabilities
    GPTAudio,   // GPT Audio models (2025)
    GPTImage,   // GPT image generation models
    Realtime,   // Realtime API models
    GPT54,      // GPT-5.4 models (2026)
    GPT54Mini,  // GPT-5.4 Mini models (2026)
    GPT54Turbo, // GPT-5.4 Turbo models (2026)
}

/// Model-specific configuration
#[derive(Debug, Clone)]
pub struct OpenAIModelConfig {
    /// Maximum requests per minute
    pub max_rpm: Option<u32>,
    /// Maximum tokens per minute
    pub max_tpm: Option<u32>,
    /// Supports batch API
    pub supports_batch: bool,
    /// Default temperature
    pub default_temperature: Option<f32>,
    /// Supports streaming
    pub supports_streaming: bool,
    /// Custom parameters
    pub custom_params: HashMap<String, serde_json::Value>,
}

impl Default for OpenAIModelConfig {
    fn default() -> Self {
        Self {
            max_rpm: None,
            max_tpm: None,
            supports_batch: false,
            default_temperature: None,
            supports_streaming: true,
            custom_params: HashMap::new(),
        }
    }
}

/// OpenAI use cases for model recommendation
#[derive(Debug, Clone)]
pub enum OpenAIUseCase {
    GeneralChat,
    CodeGeneration,
    Reasoning,
    Vision,
    ImageGeneration,
    AudioTranscription,
    TextToSpeech,
    Embeddings,
    CostOptimized,
}
