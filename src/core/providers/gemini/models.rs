//! Gemini Model Registry
//!
//! Unified model registry system containing capabilities and pricing information for all Gemini models

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::core::types::model::ModelInfo;

/// Model features
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModelFeature {
    /// Multimodal support (images, videos, audio)
    MultimodalSupport,
    /// Tool calling support
    ToolCalling,
    /// Function calling support
    FunctionCalling,
    /// Streaming support
    StreamingSupport,
    /// Context caching support
    ContextCaching,
    /// System instructions support
    SystemInstructions,
    /// Batch processing support
    BatchProcessing,
    /// JSON mode support
    JsonMode,
    /// Code execution support
    CodeExecution,
    /// Search grounding support
    SearchGrounding,
    /// Video understanding support
    VideoUnderstanding,
    /// Audio understanding support  
    AudioUnderstanding,
    /// Real-time streaming support
    RealtimeStreaming,
}

/// Model family classification
#[derive(Debug, Clone, PartialEq)]
pub enum GeminiModelFamily {
    /// Gemini 3.1 series (2026 - Latest)
    Gemini31ProPreview,
    Gemini31Flash,
    Gemini31FlashLite,

    /// Gemini 3.0 series (2025 - Deprecated 2026-03-09)
    Gemini3Pro,
    Gemini3ProDeepThink,
    Gemini3Flash,
    Gemini3ProImage,

    /// Gemini 2.5 series (2025)
    Gemini25Pro,
    Gemini25Flash,
    Gemini25FlashLite,

    /// Gemini 2.0 series
    Gemini20Flash,
    Gemini20FlashThinking,

    /// Gemini 1.5 series
    Gemini15Pro,
    Gemini15Flash,
    Gemini15Flash8B,

    /// Gemini 1.0 series
    Gemini10Pro,
    Gemini10ProVision,

    /// Experimental models
    GeminiExperimental,
}

/// Model
#[derive(Debug, Clone)]
pub struct ModelPricing {
    /// Input token price (USD per million tokens)
    pub input_price: f64,
    /// Output token price (USD per million tokens)
    pub output_price: f64,
    /// Cached input price (optional)
    pub cached_input_price: Option<f64>,
    /// Image price (per image)
    pub image_price: Option<f64>,
    /// Video price (per second)
    pub video_price_per_second: Option<f64>,
    /// Audio price (per second)
    pub audio_price_per_second: Option<f64>,
}

/// Model limits
#[derive(Debug, Clone)]
pub struct ModelLimits {
    /// Maximum context length
    pub max_context_length: u32,
    /// Maximum output tokens
    pub max_output_tokens: u32,
    /// Maximum image count
    pub max_images: Option<u32>,
    /// Maximum video length (seconds)
    pub max_video_seconds: Option<u32>,
    /// Maximum audio length (seconds)
    pub max_audio_seconds: Option<u32>,
    /// Requests per minute limit
    pub rpm_limit: Option<u32>,
    /// Tokens per minute limit
    pub tpm_limit: Option<u32>,
}

/// Model
#[derive(Debug, Clone)]
pub struct ModelSpec {
    /// Model
    pub model_info: ModelInfo,
    /// Model
    pub family: GeminiModelFamily,
    /// Supported features
    pub features: Vec<ModelFeature>,
    /// Pricing information
    pub pricing: ModelPricing,
    /// Limit information
    pub limits: ModelLimits,
}

/// Model
#[derive(Debug, Clone)]
pub struct GeminiModelRegistry {
    models: HashMap<String, ModelSpec>,
}

impl GeminiModelRegistry {
    /// Expected number of Gemini models for capacity hint
    const EXPECTED_MODEL_COUNT: usize = 12;

    /// Create
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::with_capacity(Self::EXPECTED_MODEL_COUNT),
        };
        registry.initialize_models();
        registry
    }

    /// Initialize all Gemini models
    fn initialize_models(&mut self) {
        // ==================== Gemini 3.1 Series (2026 - Latest) ====================

        // Gemini 3.1 Pro Preview
        self.register_model(
            "gemini-3.1-pro-preview",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-3.1-pro-preview".to_string(),
                    name: "Gemini 3.1 Pro Preview".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_048_576,
                    max_output_length: Some(65536),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.002),
                    output_cost_per_1k_tokens: Some(0.012),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini31ProPreview,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                    ModelFeature::JsonMode,
                    ModelFeature::CodeExecution,
                    ModelFeature::SearchGrounding,
                    ModelFeature::VideoUnderstanding,
                    ModelFeature::AudioUnderstanding,
                ],
                pricing: ModelPricing {
                    input_price: 2.0,
                    output_price: 12.0,
                    cached_input_price: Some(0.5),
                    image_price: Some(0.005),
                    video_price_per_second: Some(0.005),
                    audio_price_per_second: Some(0.0005),
                },
                limits: ModelLimits {
                    max_context_length: 1_048_576,
                    max_output_tokens: 65536,
                    max_images: Some(3000),
                    max_video_seconds: Some(3600),
                    max_audio_seconds: Some(9600),
                    rpm_limit: Some(1000),
                    tpm_limit: Some(4_000_000),
                },
            },
        );

        // Gemini 3.1 Flash
        self.register_model(
            "gemini-3.1-flash",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-3.1-flash".to_string(),
                    name: "Gemini 3.1 Flash".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_048_576,
                    max_output_length: Some(65536),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.000075),
                    output_cost_per_1k_tokens: Some(0.0003),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini31Flash,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                    ModelFeature::JsonMode,
                    ModelFeature::CodeExecution,
                    ModelFeature::SearchGrounding,
                    ModelFeature::VideoUnderstanding,
                    ModelFeature::AudioUnderstanding,
                ],
                pricing: ModelPricing {
                    input_price: 0.075,
                    output_price: 0.30,
                    cached_input_price: Some(0.01875),
                    image_price: Some(0.0002),
                    video_price_per_second: Some(0.0002),
                    audio_price_per_second: Some(0.00002),
                },
                limits: ModelLimits {
                    max_context_length: 1_048_576,
                    max_output_tokens: 65536,
                    max_images: Some(3000),
                    max_video_seconds: Some(3600),
                    max_audio_seconds: Some(9600),
                    rpm_limit: Some(2000),
                    tpm_limit: Some(4_000_000),
                },
            },
        );

        // Gemini 3.1 Flash Lite
        self.register_model(
            "gemini-3.1-flash-lite",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-3.1-flash-lite".to_string(),
                    name: "Gemini 3.1 Flash Lite".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_048_576,
                    max_output_length: Some(32768),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.0000375),
                    output_cost_per_1k_tokens: Some(0.00015),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini31FlashLite,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::SystemInstructions,
                    ModelFeature::JsonMode,
                ],
                pricing: ModelPricing {
                    input_price: 0.0375,
                    output_price: 0.15,
                    cached_input_price: Some(0.01),
                    image_price: None,
                    video_price_per_second: None,
                    audio_price_per_second: None,
                },
                limits: ModelLimits {
                    max_context_length: 1_048_576,
                    max_output_tokens: 32768,
                    max_images: Some(1000),
                    max_video_seconds: None,
                    max_audio_seconds: None,
                    rpm_limit: Some(4000),
                    tpm_limit: Some(4_000_000),
                },
            },
        );

        // ==================== Gemini 3.0 Series (2025 - Deprecated 2026-03-09) ====================

        // Gemini 3 Pro
        self.register_model(
            "gemini-3-pro",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-3-pro".to_string(),
                    name: "Gemini 3 Pro".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(65536),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.002),
                    output_cost_per_1k_tokens: Some(0.012),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini3Pro,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                    ModelFeature::JsonMode,
                    ModelFeature::CodeExecution,
                    ModelFeature::SearchGrounding,
                    ModelFeature::VideoUnderstanding,
                    ModelFeature::AudioUnderstanding,
                ],
                pricing: ModelPricing {
                    input_price: 2.0,   // $2 per 1M tokens (<=200K)
                    output_price: 12.0, // $12 per 1M tokens (<=200K)
                    cached_input_price: Some(0.5),
                    image_price: Some(0.005),
                    video_price_per_second: Some(0.005),
                    audio_price_per_second: Some(0.0005),
                },
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 65536,
                    max_images: Some(3000),
                    max_video_seconds: Some(3600),
                    max_audio_seconds: Some(9600),
                    rpm_limit: Some(1000),
                    tpm_limit: Some(4_000_000),
                },
            },
        );

        // Gemini 3 Pro Deep Think
        self.register_model(
            "gemini-3-pro-deep-think",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-3-pro-deep-think".to_string(),
                    name: "Gemini 3 Pro Deep Think".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(65536),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.004),
                    output_cost_per_1k_tokens: Some(0.024),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini3ProDeepThink,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::JsonMode,
                    ModelFeature::CodeExecution,
                    ModelFeature::SearchGrounding,
                    ModelFeature::VideoUnderstanding,
                    ModelFeature::AudioUnderstanding,
                ],
                pricing: ModelPricing {
                    input_price: 4.0,   // $4 per 1M tokens (deep think mode)
                    output_price: 24.0, // $24 per 1M tokens (deep think mode)
                    cached_input_price: Some(1.0),
                    image_price: Some(0.01),
                    video_price_per_second: Some(0.01),
                    audio_price_per_second: Some(0.001),
                },
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 65536,
                    max_images: Some(3000),
                    max_video_seconds: Some(3600),
                    max_audio_seconds: Some(9600),
                    rpm_limit: Some(500),
                    tpm_limit: Some(2_000_000),
                },
            },
        );

        // Gemini 3 Flash Preview
        self.register_model(
            "gemini-3-flash-preview",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-3-flash-preview".to_string(),
                    name: "Gemini 3 Flash Preview".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_048_576,
                    max_output_length: Some(65536),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.0005),
                    output_cost_per_1k_tokens: Some(0.003),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini3Flash,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                    ModelFeature::JsonMode,
                    ModelFeature::CodeExecution,
                    ModelFeature::SearchGrounding,
                    ModelFeature::VideoUnderstanding,
                    ModelFeature::AudioUnderstanding,
                ],
                pricing: ModelPricing {
                    input_price: 0.5,  // $0.50 per 1M tokens
                    output_price: 3.0, // $3 per 1M tokens
                    cached_input_price: Some(0.125),
                    image_price: Some(0.002),
                    video_price_per_second: Some(0.002),
                    audio_price_per_second: Some(0.0002),
                },
                limits: ModelLimits {
                    max_context_length: 1_048_576,
                    max_output_tokens: 65536,
                    max_images: Some(3000),
                    max_video_seconds: Some(3600),
                    max_audio_seconds: Some(9600),
                    rpm_limit: Some(2000),
                    tpm_limit: Some(8_000_000),
                },
            },
        );

        // Gemini 3 Pro Image Preview
        self.register_model(
            "gemini-3-pro-image-preview",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-3-pro-image-preview".to_string(),
                    name: "Gemini 3 Pro Image Preview".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 65536,
                    max_output_length: Some(8192),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.002),
                    output_cost_per_1k_tokens: Some(0.012),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ImageGeneration,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini3ProImage,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::StreamingSupport,
                    ModelFeature::SystemInstructions,
                    ModelFeature::JsonMode,
                ],
                pricing: ModelPricing {
                    input_price: 2.0,   // $2 per 1M tokens
                    output_price: 12.0, // $12 per 1M tokens
                    cached_input_price: Some(0.5),
                    image_price: Some(0.04), // Image generation pricing
                    video_price_per_second: None,
                    audio_price_per_second: None,
                },
                limits: ModelLimits {
                    max_context_length: 65536,
                    max_output_tokens: 8192,
                    max_images: Some(16),
                    max_video_seconds: None,
                    max_audio_seconds: None,
                    rpm_limit: Some(500),
                    tpm_limit: Some(1_000_000),
                },
            },
        );

        // ==================== Gemini 2.5 Series (2025) ====================

        // Gemini 2.5 Pro
        self.register_model(
            "gemini-2.5-pro",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-2.5-pro".to_string(),
                    name: "Gemini 2.5 Pro".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(65536),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.00125),
                    output_cost_per_1k_tokens: Some(0.010),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini25Pro,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                    ModelFeature::JsonMode,
                    ModelFeature::CodeExecution,
                    ModelFeature::SearchGrounding,
                    ModelFeature::VideoUnderstanding,
                    ModelFeature::AudioUnderstanding,
                ],
                pricing: ModelPricing {
                    input_price: 1.25,  // $1.25 per 1M tokens (<=200K)
                    output_price: 10.0, // $10 per 1M tokens (<=200K)
                    cached_input_price: Some(0.3125),
                    image_price: Some(0.005),
                    video_price_per_second: Some(0.005),
                    audio_price_per_second: Some(0.0005),
                },
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 65536,
                    max_images: Some(3000),
                    max_video_seconds: Some(3600),
                    max_audio_seconds: Some(9600),
                    rpm_limit: Some(1000),
                    tpm_limit: Some(4_000_000),
                },
            },
        );

        // Gemini 2.5 Flash
        self.register_model(
            "gemini-2.5-flash",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-2.5-flash".to_string(),
                    name: "Gemini 2.5 Flash".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(65536),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.0003),
                    output_cost_per_1k_tokens: Some(0.0025),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini25Flash,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                    ModelFeature::JsonMode,
                    ModelFeature::CodeExecution,
                    ModelFeature::SearchGrounding,
                    ModelFeature::VideoUnderstanding,
                    ModelFeature::AudioUnderstanding,
                ],
                pricing: ModelPricing {
                    input_price: 0.30,  // $0.30 per 1M tokens
                    output_price: 2.50, // $2.50 per 1M tokens
                    cached_input_price: Some(0.075),
                    image_price: Some(0.0002),
                    video_price_per_second: Some(0.0002),
                    audio_price_per_second: Some(0.0001),
                },
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 65536,
                    max_images: Some(3000),
                    max_video_seconds: Some(3600),
                    max_audio_seconds: Some(9600),
                    rpm_limit: Some(2000),
                    tpm_limit: Some(4_000_000),
                },
            },
        );

        // Gemini 2.5 Flash-Lite
        self.register_model(
            "gemini-2.5-flash-lite",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-2.5-flash-lite".to_string(),
                    name: "Gemini 2.5 Flash-Lite".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(65536),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.0001),
                    output_cost_per_1k_tokens: Some(0.0004),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini25FlashLite,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                    ModelFeature::JsonMode,
                    ModelFeature::CodeExecution,
                    ModelFeature::SearchGrounding,
                ],
                pricing: ModelPricing {
                    input_price: 0.10,  // $0.10 per 1M tokens
                    output_price: 0.40, // $0.40 per 1M tokens
                    cached_input_price: Some(0.025),
                    image_price: Some(0.0001),
                    video_price_per_second: None,
                    audio_price_per_second: None,
                },
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 65536,
                    max_images: Some(3000),
                    max_video_seconds: None,
                    max_audio_seconds: None,
                    rpm_limit: Some(4000),
                    tpm_limit: Some(4_000_000),
                },
            },
        );

        // ==================== Gemini 2.0 Series ====================

        // Gemini 2.0 Flash
        self.register_model(
            "gemini-2.0-flash-exp",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-2.0-flash-exp".to_string(),
                    name: "Gemini 2.0 Flash".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(8192),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.00001),
                    output_cost_per_1k_tokens: Some(0.00004),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini20Flash,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                    ModelFeature::JsonMode,
                    ModelFeature::CodeExecution,
                    ModelFeature::SearchGrounding,
                    ModelFeature::VideoUnderstanding,
                    ModelFeature::AudioUnderstanding,
                ],
                pricing: ModelPricing {
                    input_price: 0.01,  // $0.01 per 1M tokens
                    output_price: 0.04, // $0.04 per 1M tokens
                    cached_input_price: Some(0.0025),
                    image_price: Some(0.0001),
                    video_price_per_second: Some(0.001),
                    audio_price_per_second: Some(0.0001),
                },
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 8192,
                    max_images: Some(3000),
                    max_video_seconds: Some(3600),
                    max_audio_seconds: Some(9600),
                    rpm_limit: Some(2000),
                    tpm_limit: Some(4_000_000),
                },
            },
        );

        // Gemini 2.0 Flash Thinking (experimental)
        self.register_model(
            "gemini-2.0-flash-thinking-exp",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-2.0-flash-thinking-exp".to_string(),
                    name: "Gemini 2.0 Flash Thinking".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 32_000,
                    max_output_length: Some(8192),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.00001),
                    output_cost_per_1k_tokens: Some(0.00004),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini20FlashThinking,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::StreamingSupport,
                    ModelFeature::SystemInstructions,
                ],
                pricing: ModelPricing {
                    input_price: 0.01,
                    output_price: 0.04,
                    cached_input_price: None,
                    image_price: Some(0.0001),
                    video_price_per_second: None,
                    audio_price_per_second: None,
                },
                limits: ModelLimits {
                    max_context_length: 32_000,
                    max_output_tokens: 8192,
                    max_images: Some(50),
                    max_video_seconds: None,
                    max_audio_seconds: None,
                    rpm_limit: Some(100),
                    tpm_limit: Some(100_000),
                },
            },
        );

        // Gemini 1.5 Pro
        self.register_model(
            "gemini-1.5-pro",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-1.5-pro".to_string(),
                    name: "Gemini 1.5 Pro".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 2_000_000,
                    max_output_length: Some(8192),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.00125),
                    output_cost_per_1k_tokens: Some(0.005),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini15Pro,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                    ModelFeature::JsonMode,
                    ModelFeature::CodeExecution,
                    ModelFeature::SearchGrounding,
                    ModelFeature::VideoUnderstanding,
                    ModelFeature::AudioUnderstanding,
                ],
                pricing: ModelPricing {
                    input_price: 1.25, // $1.25 per 1M tokens (<=128K)
                    output_price: 5.0, // $5.00 per 1M tokens (<=128K)
                    cached_input_price: Some(0.3125),
                    image_price: Some(0.002625),
                    video_price_per_second: Some(0.002625),
                    audio_price_per_second: Some(0.000125),
                },
                limits: ModelLimits {
                    max_context_length: 2_000_000,
                    max_output_tokens: 8192,
                    max_images: Some(3000),
                    max_video_seconds: Some(3600),
                    max_audio_seconds: Some(9600),
                    rpm_limit: Some(360),
                    tpm_limit: Some(4_000_000),
                },
            },
        );

        // Gemini 1.5 Flash
        self.register_model(
            "gemini-1.5-flash",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-1.5-flash".to_string(),
                    name: "Gemini 1.5 Flash".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(8192),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.000075),
                    output_cost_per_1k_tokens: Some(0.0003),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini15Flash,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                    ModelFeature::JsonMode,
                    ModelFeature::CodeExecution,
                    ModelFeature::SearchGrounding,
                    ModelFeature::VideoUnderstanding,
                    ModelFeature::AudioUnderstanding,
                ],
                pricing: ModelPricing {
                    input_price: 0.075, // $0.075 per 1M tokens (<=128K)
                    output_price: 0.30, // $0.30 per 1M tokens (<=128K)
                    cached_input_price: Some(0.01875),
                    image_price: Some(0.0002),
                    video_price_per_second: Some(0.0002),
                    audio_price_per_second: Some(0.0001),
                },
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 8192,
                    max_images: Some(3000),
                    max_video_seconds: Some(3600),
                    max_audio_seconds: Some(9600),
                    rpm_limit: Some(1500),
                    tpm_limit: Some(4_000_000),
                },
            },
        );

        // Gemini 1.5 Flash-8B
        self.register_model(
            "gemini-1.5-flash-8b",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-1.5-flash-8b".to_string(),
                    name: "Gemini 1.5 Flash 8B".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(8192),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.0000375),
                    output_cost_per_1k_tokens: Some(0.00015),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini15Flash8B,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::ContextCaching,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                    ModelFeature::JsonMode,
                    ModelFeature::VideoUnderstanding,
                    ModelFeature::AudioUnderstanding,
                ],
                pricing: ModelPricing {
                    input_price: 0.0375, // $0.0375 per 1M tokens
                    output_price: 0.15,  // $0.15 per 1M tokens
                    cached_input_price: Some(0.01),
                    image_price: Some(0.0001),
                    video_price_per_second: Some(0.0001),
                    audio_price_per_second: Some(0.00005),
                },
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 8192,
                    max_images: Some(3000),
                    max_video_seconds: Some(3600),
                    max_audio_seconds: Some(9600),
                    rpm_limit: Some(4000),
                    tpm_limit: Some(4_000_000),
                },
            },
        );

        // Gemini 1.0 Pro
        self.register_model(
            "gemini-1.0-pro",
            ModelSpec {
                model_info: ModelInfo {
                    id: "gemini-1.0-pro".to_string(),
                    name: "Gemini 1.0 Pro".to_string(),
                    provider: "gemini".to_string(),
                    max_context_length: 32_000,
                    max_output_length: Some(8192),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: false,
                    input_cost_per_1k_tokens: Some(0.0005),
                    output_cost_per_1k_tokens: Some(0.0015),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                        crate::core::types::model::ProviderCapability::ToolCalling,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: GeminiModelFamily::Gemini10Pro,
                features: vec![
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::SystemInstructions,
                    ModelFeature::BatchProcessing,
                ],
                pricing: ModelPricing {
                    input_price: 0.50,  // $0.50 per 1M tokens
                    output_price: 1.50, // $1.50 per 1M tokens
                    cached_input_price: None,
                    image_price: None,
                    video_price_per_second: None,
                    audio_price_per_second: None,
                },
                limits: ModelLimits {
                    max_context_length: 32_000,
                    max_output_tokens: 8192,
                    max_images: None,
                    max_video_seconds: None,
                    max_audio_seconds: None,
                    rpm_limit: Some(300),
                    tpm_limit: Some(300_000),
                },
            },
        );
    }

    /// Model
    fn register_model(&mut self, id: &str, spec: ModelSpec) {
        self.models.insert(id.to_string(), spec);
    }

    /// Model
    pub fn get_model_spec(&self, model_id: &str) -> Option<&ModelSpec> {
        self.models.get(model_id)
    }

    /// Model
    pub fn list_models(&self) -> Vec<&ModelSpec> {
        self.models.values().collect()
    }

    /// Check
    pub fn supports_feature(&self, model_id: &str, feature: &ModelFeature) -> bool {
        self.get_model_spec(model_id)
            .map(|spec| spec.features.contains(feature))
            .unwrap_or(false)
    }

    /// Model
    pub fn get_model_family(&self, model_id: &str) -> Option<&GeminiModelFamily> {
        self.get_model_spec(model_id).map(|spec| &spec.family)
    }

    /// Model
    pub fn get_model_pricing(&self, model_id: &str) -> Option<&ModelPricing> {
        self.get_model_spec(model_id).map(|spec| &spec.pricing)
    }

    /// Model
    pub fn get_model_limits(&self, model_id: &str) -> Option<&ModelLimits> {
        self.get_model_spec(model_id).map(|spec| &spec.limits)
    }

    /// Detect model family from model name string
    pub fn from_model_name(model_name: &str) -> Option<GeminiModelFamily> {
        let model_lower = model_name.to_lowercase();

        // Gemini 3.1 series (check before 3.0 as more specific)
        if model_lower.contains("gemini-3.1-flash-lite") {
            Some(GeminiModelFamily::Gemini31FlashLite)
        } else if model_lower.contains("gemini-3.1-flash") {
            Some(GeminiModelFamily::Gemini31Flash)
        } else if model_lower.contains("gemini-3.1-pro") {
            Some(GeminiModelFamily::Gemini31ProPreview)
        }
        // Gemini 3.0 series (deprecated 2026-03-09)
        else if model_lower.contains("gemini-3") && model_lower.contains("deep-think") {
            Some(GeminiModelFamily::Gemini3ProDeepThink)
        } else if model_lower.contains("gemini-3") && model_lower.contains("image") {
            Some(GeminiModelFamily::Gemini3ProImage)
        } else if model_lower.contains("gemini-3-flash") || model_lower.contains("gemini-3.0-flash")
        {
            Some(GeminiModelFamily::Gemini3Flash)
        } else if model_lower.contains("gemini-3-pro") || model_lower.contains("gemini-3.0-pro") {
            Some(GeminiModelFamily::Gemini3Pro)
        }
        // Gemini 2.5 series
        else if model_lower.contains("gemini-2.5-flash-lite") {
            Some(GeminiModelFamily::Gemini25FlashLite)
        } else if model_lower.contains("gemini-2.5-flash") {
            Some(GeminiModelFamily::Gemini25Flash)
        } else if model_lower.contains("gemini-2.5-pro") {
            Some(GeminiModelFamily::Gemini25Pro)
        }
        // Gemini 2.0 series
        else if model_lower.contains("gemini-2.0-flash-thinking") {
            Some(GeminiModelFamily::Gemini20FlashThinking)
        } else if model_lower.contains("gemini-2.0-flash") || model_lower.contains("gemini-2-flash")
        {
            Some(GeminiModelFamily::Gemini20Flash)
        }
        // Gemini 1.5 series
        else if model_lower.contains("gemini-1.5-pro") || model_lower.contains("gemini-15-pro") {
            Some(GeminiModelFamily::Gemini15Pro)
        } else if model_lower.contains("gemini-1.5-flash-8b") {
            Some(GeminiModelFamily::Gemini15Flash8B)
        } else if model_lower.contains("gemini-1.5-flash")
            || model_lower.contains("gemini-15-flash")
        {
            Some(GeminiModelFamily::Gemini15Flash)
        }
        // Gemini 1.0 series
        else if model_lower.contains("gemini-1.0-pro-vision") {
            Some(GeminiModelFamily::Gemini10ProVision)
        } else if model_lower.contains("gemini-1.0-pro") || model_lower.contains("gemini-pro") {
            Some(GeminiModelFamily::Gemini10Pro)
        }
        // Experimental
        else if model_lower.contains("gemini-exp") {
            Some(GeminiModelFamily::GeminiExperimental)
        } else {
            None
        }
    }
}

impl Default for GeminiModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Model
pub fn get_gemini_registry() -> &'static GeminiModelRegistry {
    static REGISTRY: OnceLock<GeminiModelRegistry> = OnceLock::new();
    REGISTRY.get_or_init(GeminiModelRegistry::new)
}

/// Cost calculation utility
pub struct CostCalculator;

impl CostCalculator {
    /// Calculate basic cost
    pub fn calculate_cost(
        model_id: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
    ) -> Option<f64> {
        let registry = get_gemini_registry();
        let pricing = registry.get_model_pricing(model_id)?;

        let input_cost = (prompt_tokens as f64 / 1_000_000.0) * pricing.input_price;
        let output_cost = (completion_tokens as f64 / 1_000_000.0) * pricing.output_price;

        Some(input_cost + output_cost)
    }

    /// Calculate multimodal cost
    pub fn calculate_multimodal_cost(
        model_id: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
        cached_tokens: Option<u32>,
        images: Option<u32>,
        video_seconds: Option<u32>,
        audio_seconds: Option<u32>,
    ) -> Option<f64> {
        let registry = get_gemini_registry();
        let pricing = registry.get_model_pricing(model_id)?;

        let mut total_cost = 0.0;
        let mut remaining_prompt_tokens = prompt_tokens;

        // Handle
        if let (Some(cached), Some(cached_price)) = (cached_tokens, pricing.cached_input_price) {
            let cached_cost = (cached as f64 / 1_000_000.0) * cached_price;
            total_cost += cached_cost;
            remaining_prompt_tokens = remaining_prompt_tokens.saturating_sub(cached);
        }

        // Regular input tokens
        let input_cost = (remaining_prompt_tokens as f64 / 1_000_000.0) * pricing.input_price;
        total_cost += input_cost;

        // Output tokens
        let output_cost = (completion_tokens as f64 / 1_000_000.0) * pricing.output_price;
        total_cost += output_cost;

        // Image cost
        if let (Some(img_count), Some(img_price)) = (images, pricing.image_price) {
            total_cost += img_count as f64 * img_price;
        }

        // Video cost
        if let (Some(video_secs), Some(video_price)) =
            (video_seconds, pricing.video_price_per_second)
        {
            total_cost += video_secs as f64 * video_price;
        }

        // Audio cost
        if let (Some(audio_secs), Some(audio_price)) =
            (audio_seconds, pricing.audio_price_per_second)
        {
            total_cost += audio_secs as f64 * audio_price;
        }

        Some(total_cost)
    }

    /// Estimate token count
    pub fn estimate_tokens(text: &str) -> u32 {
        // Gemini uses approximately 4 characters = 1 token ratio (English)
        (text.len() as f32 / 4.0).ceil() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_registry() {
        let registry = get_gemini_registry();

        // Test Gemini 2.0 Flash
        let flash_spec = registry.get_model_spec("gemini-2.0-flash-exp").unwrap();
        assert_eq!(flash_spec.family, GeminiModelFamily::Gemini20Flash);
        assert!(
            flash_spec
                .features
                .contains(&ModelFeature::MultimodalSupport)
        );
        assert!(
            flash_spec
                .features
                .contains(&ModelFeature::VideoUnderstanding)
        );

        // Test pricing
        assert_eq!(flash_spec.pricing.input_price, 0.01);
        assert_eq!(flash_spec.pricing.output_price, 0.04);
    }

    #[test]
    fn test_model_family_detection() {
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-2.0-flash-exp"),
            Some(GeminiModelFamily::Gemini20Flash)
        );

        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-1.5-pro-latest"),
            Some(GeminiModelFamily::Gemini15Pro)
        );

        assert_eq!(GeminiModelRegistry::from_model_name("unknown-model"), None);
    }

    #[test]
    fn test_cost_calculation() {
        let cost = CostCalculator::calculate_cost("gemini-1.5-flash", 1000, 500);
        assert!(cost.is_some());

        let cost_value = cost.unwrap();
        // Expected: (1000/1M * $0.075) + (500/1M * $0.30) = $0.000075 + $0.00015 = $0.000225
        assert!((cost_value - 0.000225).abs() < 0.000001);
    }

    #[test]
    fn test_feature_support() {
        let registry = get_gemini_registry();

        // Gemini 2.0 Flash supports video understanding
        assert!(
            registry.supports_feature("gemini-2.0-flash-exp", &ModelFeature::VideoUnderstanding)
        );

        // Gemini 1.0 Pro does not support multimodal
        assert!(!registry.supports_feature("gemini-1.0-pro", &ModelFeature::VideoUnderstanding));
    }

    #[test]
    fn test_registry_default() {
        let registry = GeminiModelRegistry::default();
        assert!(!registry.models.is_empty());
    }

    #[test]
    fn test_list_models() {
        let registry = get_gemini_registry();
        let models = registry.list_models();
        assert!(!models.is_empty());
        assert!(models.len() >= 10); // We have at least 10 models registered
    }

    #[test]
    fn test_get_model_family() {
        let registry = get_gemini_registry();
        let family = registry.get_model_family("gemini-1.5-pro");
        assert!(family.is_some());
        assert_eq!(*family.unwrap(), GeminiModelFamily::Gemini15Pro);

        let family_unknown = registry.get_model_family("unknown-model");
        assert!(family_unknown.is_none());
    }

    #[test]
    fn test_get_model_pricing() {
        let registry = get_gemini_registry();
        let pricing = registry.get_model_pricing("gemini-1.5-flash");
        assert!(pricing.is_some());
        let pricing_value = pricing.unwrap();
        assert_eq!(pricing_value.input_price, 0.075);
        assert_eq!(pricing_value.output_price, 0.30);
        assert!(pricing_value.cached_input_price.is_some());
    }

    #[test]
    fn test_get_model_limits() {
        let registry = get_gemini_registry();
        let limits = registry.get_model_limits("gemini-1.5-pro");
        assert!(limits.is_some());
        let limits_value = limits.unwrap();
        assert_eq!(limits_value.max_context_length, 2_000_000);
        assert_eq!(limits_value.max_output_tokens, 8192);
    }

    #[test]
    fn test_model_family_detection_gemini_3() {
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-3-pro"),
            Some(GeminiModelFamily::Gemini3Pro)
        );
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-3-pro-deep-think"),
            Some(GeminiModelFamily::Gemini3ProDeepThink)
        );
    }

    #[test]
    fn test_model_family_detection_gemini_25() {
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-2.5-pro"),
            Some(GeminiModelFamily::Gemini25Pro)
        );
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-2.5-flash"),
            Some(GeminiModelFamily::Gemini25Flash)
        );
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-2.5-flash-lite"),
            Some(GeminiModelFamily::Gemini25FlashLite)
        );
    }

    #[test]
    fn test_model_family_detection_gemini_20() {
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-2.0-flash-thinking-exp"),
            Some(GeminiModelFamily::Gemini20FlashThinking)
        );
    }

    #[test]
    fn test_model_family_detection_gemini_15() {
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-1.5-flash-8b"),
            Some(GeminiModelFamily::Gemini15Flash8B)
        );
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-1.5-flash"),
            Some(GeminiModelFamily::Gemini15Flash)
        );
    }

    #[test]
    fn test_model_family_detection_gemini_10() {
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-1.0-pro"),
            Some(GeminiModelFamily::Gemini10Pro)
        );
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-1.0-pro-vision"),
            Some(GeminiModelFamily::Gemini10ProVision)
        );
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-pro"),
            Some(GeminiModelFamily::Gemini10Pro)
        );
    }

    #[test]
    fn test_model_family_detection_experimental() {
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-exp-something"),
            Some(GeminiModelFamily::GeminiExperimental)
        );
    }

    #[test]
    fn test_cost_calculation_unknown_model() {
        let cost = CostCalculator::calculate_cost("unknown-model", 1000, 500);
        assert!(cost.is_none());
    }

    #[test]
    fn test_multimodal_cost_calculation() {
        let cost = CostCalculator::calculate_multimodal_cost(
            "gemini-1.5-flash",
            1000,
            500,
            Some(200),
            Some(5),
            None,
            None,
        );
        assert!(cost.is_some());
        let cost_value = cost.unwrap();
        // Should include cached tokens discount and image cost
        assert!(cost_value > 0.0);
    }

    #[test]
    fn test_multimodal_cost_with_video_and_audio() {
        let cost = CostCalculator::calculate_multimodal_cost(
            "gemini-2.0-flash-exp",
            1000,
            500,
            None,
            Some(5),
            Some(60),
            Some(120),
        );
        assert!(cost.is_some());
        let cost_value = cost.unwrap();
        // Should include image, video, and audio costs
        assert!(cost_value > 0.0);
    }

    #[test]
    fn test_estimate_tokens() {
        let tokens = CostCalculator::estimate_tokens("Hello, world!");
        // "Hello, world!" is 13 characters, ~4 tokens (13/4 = 3.25, ceil = 4)
        assert!((3..=5).contains(&tokens));
    }

    #[test]
    fn test_feature_support_unknown_model() {
        let registry = get_gemini_registry();
        assert!(!registry.supports_feature("unknown-model", &ModelFeature::MultimodalSupport));
    }

    #[test]
    fn test_gemini_15_pro_features() {
        let registry = get_gemini_registry();
        let spec = registry.get_model_spec("gemini-1.5-pro").unwrap();

        assert!(spec.features.contains(&ModelFeature::ToolCalling));
        assert!(spec.features.contains(&ModelFeature::FunctionCalling));
        assert!(spec.features.contains(&ModelFeature::StreamingSupport));
        assert!(spec.features.contains(&ModelFeature::ContextCaching));
        assert!(spec.features.contains(&ModelFeature::SystemInstructions));
        assert!(spec.features.contains(&ModelFeature::BatchProcessing));
        assert!(spec.features.contains(&ModelFeature::JsonMode));
        assert!(spec.features.contains(&ModelFeature::CodeExecution));
        assert!(spec.features.contains(&ModelFeature::SearchGrounding));
        assert!(spec.features.contains(&ModelFeature::VideoUnderstanding));
        assert!(spec.features.contains(&ModelFeature::AudioUnderstanding));
    }

    #[test]
    fn test_gemini_10_pro_limited_features() {
        let registry = get_gemini_registry();
        let spec = registry.get_model_spec("gemini-1.0-pro").unwrap();

        // Gemini 1.0 Pro should not have multimodal support
        assert!(!spec.features.contains(&ModelFeature::MultimodalSupport));
        assert!(!spec.features.contains(&ModelFeature::VideoUnderstanding));
        assert!(!spec.features.contains(&ModelFeature::AudioUnderstanding));

        // But should have basic features
        assert!(spec.features.contains(&ModelFeature::ToolCalling));
        assert!(spec.features.contains(&ModelFeature::StreamingSupport));
    }

    #[test]
    fn test_model_info_structure() {
        let registry = get_gemini_registry();
        let spec = registry.get_model_spec("gemini-2.5-flash").unwrap();

        assert_eq!(spec.model_info.id, "gemini-2.5-flash");
        assert_eq!(spec.model_info.provider, "gemini");
        assert!(spec.model_info.supports_streaming);
        assert!(spec.model_info.supports_tools);
        assert!(spec.model_info.supports_multimodal);
    }
}
