//! Anthropic Model Registry
//!
//! Unified model registry system with integrated pricing and capability information

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::core::types::model::ModelInfo;

/// Model
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModelFeature {
    /// Multimodal support (images, documents)
    MultimodalSupport,
    /// Tool calling support
    ToolCalling,
    /// Function calling support
    FunctionCalling,
    /// Streaming response support
    StreamingSupport,
    /// Cache control support
    CacheControl,
    /// System message support
    SystemMessages,
    /// Batch processing support
    BatchProcessing,
    /// Thinking mode support
    ThinkingMode,
    /// Computer tool support
    ComputerUse,
}

/// Model
#[derive(Debug, Clone, PartialEq)]
pub enum AnthropicModelFamily {
    /// Claude Opus 4.6 models (latest flagship)
    ClaudeOpus46,
    /// Claude Opus 4.5 models (latest flagship)
    ClaudeOpus45,
    /// Claude Sonnet 4.6 models
    ClaudeSonnet46,
    /// Claude Sonnet 4.5 models (latest balanced)
    ClaudeSonnet45,
    /// Claude Haiku 4.5 models
    ClaudeHaiku45,
    /// Claude Opus 4.1 models
    ClaudeOpus41,
    /// Claude Opus 4 models
    ClaudeOpus4,
    /// Claude Sonnet 4 models
    ClaudeSonnet4,
    /// Claude 3.5 Sonnet models
    Claude35Sonnet,
    /// Claude 3 Opus models
    Claude3Opus,
    /// Claude 3 Sonnet models
    Claude3Sonnet,
    /// Claude 3 Haiku models
    Claude3Haiku,
    /// Claude 2.1 models
    Claude21,
    /// Claude 2 models
    Claude2,
    /// Claude Instant models
    ClaudeInstant,
}

/// Model pricing information
#[derive(Debug, Clone)]
pub struct ModelPricing {
    /// Input token price (USD per million tokens)
    pub input_price: f64,
    /// Output token price (USD per million tokens)
    pub output_price: f64,
    /// Cache write price (optional)
    pub cache_write_price: Option<f64>,
    /// Cache read price (optional)
    pub cache_read_price: Option<f64>,
    /// Batch processing discount
    pub batch_discount: Option<f64>,
}

/// Model limits and constraints
#[derive(Debug, Clone)]
pub struct ModelLimits {
    /// Maximum context length
    pub max_context_length: u32,
    /// Maximum output tokens
    pub max_output_tokens: u32,
    /// Maximum number of images
    pub max_images: Option<u32>,
    /// Maximum document size (MB)
    pub max_document_size_mb: Option<u32>,
}

/// Model specification
#[derive(Debug, Clone)]
pub struct ModelSpec {
    /// Model information
    pub model_info: ModelInfo,
    /// Model family
    pub family: AnthropicModelFamily,
    /// Supported features
    pub features: Vec<ModelFeature>,
    /// Pricing information
    pub pricing: ModelPricing,
    /// Limits information
    pub limits: ModelLimits,
    /// Model configuration
    pub config: ModelConfig,
}

/// Model configuration settings
#[derive(Debug, Clone, Default)]
pub struct ModelConfig {
    /// Requires special formatting
    pub requires_special_formatting: bool,
    /// Maximum concurrent requests
    pub max_concurrent_requests: Option<u32>,
    /// Custom parameter mapping
    pub custom_params: HashMap<String, String>,
}

/// Model registry
#[derive(Debug, Clone)]
pub struct AnthropicModelRegistry {
    models: HashMap<String, ModelSpec>,
}

impl AnthropicModelRegistry {
    /// Create
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
        };
        registry.initialize_models();
        registry
    }

    /// Initialize model registry
    fn initialize_models(&mut self) {
        // Claude Opus 4.6 (Latest flagship model - January 2026)
        self.register_model(
            "claude-opus-4-6",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-opus-4-6".to_string(),
                    name: "Claude Opus 4.6".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(128_000),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.005), // $5/1M input
                    output_cost_per_1k_tokens: Some(0.025), // $25/1M output
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
                family: AnthropicModelFamily::ClaudeOpus46,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                    ModelFeature::ThinkingMode,
                    ModelFeature::ComputerUse,
                ],
                pricing: ModelPricing {
                    input_price: 5.0,
                    output_price: 25.0,
                    cache_write_price: Some(6.25),
                    cache_read_price: Some(0.50),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 128_000,
                    max_images: Some(100),
                    max_document_size_mb: Some(100),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude Opus 4.5 (Latest flagship model - November 2025)
        self.register_model(
            "claude-opus-4-5-20251101",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-opus-4-5-20251101".to_string(),
                    name: "Claude Opus 4.5".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(64_000),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.005), // $5/1M input
                    output_cost_per_1k_tokens: Some(0.025), // $25/1M output
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
                family: AnthropicModelFamily::ClaudeOpus45,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                    ModelFeature::ThinkingMode,
                    ModelFeature::ComputerUse,
                ],
                pricing: ModelPricing {
                    input_price: 5.0,              // $5/1M input (updated from OpenRouter)
                    output_price: 25.0,            // $25/1M output (updated from OpenRouter)
                    cache_write_price: Some(6.25), // 1.25x input
                    cache_read_price: Some(0.50),  // 0.1x input
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 64_000,
                    max_images: Some(100),
                    max_document_size_mb: Some(100),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude Sonnet 4.5 (Latest balanced model - September 2025)
        self.register_model(
            "claude-sonnet-4-5-20250929",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-sonnet-4-5-20250929".to_string(),
                    name: "Claude Sonnet 4.5".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(64_000),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.003),
                    output_cost_per_1k_tokens: Some(0.015),
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
                family: AnthropicModelFamily::ClaudeSonnet45,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                    ModelFeature::ThinkingMode,
                    ModelFeature::ComputerUse,
                ],
                pricing: ModelPricing {
                    input_price: 3.0,
                    output_price: 15.0,
                    cache_write_price: Some(3.75),
                    cache_read_price: Some(0.30),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 64_000,
                    max_images: Some(100),
                    max_document_size_mb: Some(100),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude Sonnet 4.6 (October 2025)
        self.register_model(
            "claude-sonnet-4-6-20251001",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-sonnet-4-6-20251001".to_string(),
                    name: "Claude Sonnet 4.6".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(64_000),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.003),
                    output_cost_per_1k_tokens: Some(0.015),
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
                family: AnthropicModelFamily::ClaudeSonnet46,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                    ModelFeature::ThinkingMode,
                    ModelFeature::ComputerUse,
                ],
                pricing: ModelPricing {
                    input_price: 3.0,
                    output_price: 15.0,
                    cache_write_price: Some(3.75),
                    cache_read_price: Some(0.30),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 64_000,
                    max_images: Some(100),
                    max_document_size_mb: Some(100),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude Haiku 4.5 (October 2025)
        self.register_model(
            "claude-haiku-4-5-20251001",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-haiku-4-5-20251001".to_string(),
                    name: "Claude Haiku 4.5".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(64_000),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.001),
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
                family: AnthropicModelFamily::ClaudeHaiku45,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                    ModelFeature::ThinkingMode,
                ],
                pricing: ModelPricing {
                    input_price: 1.0,
                    output_price: 5.0,
                    cache_write_price: Some(1.25),
                    cache_read_price: Some(0.10),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 64_000,
                    max_images: Some(100),
                    max_document_size_mb: Some(100),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude Sonnet 4 (May 2025)
        self.register_model(
            "claude-sonnet-4-20250514",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-sonnet-4-20250514".to_string(),
                    name: "Claude Sonnet 4".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(16_000),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.003),
                    output_cost_per_1k_tokens: Some(0.015),
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
                family: AnthropicModelFamily::ClaudeSonnet4,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                    ModelFeature::ThinkingMode,
                    ModelFeature::ComputerUse,
                ],
                pricing: ModelPricing {
                    input_price: 3.0,
                    output_price: 15.0,
                    cache_write_price: Some(3.75),
                    cache_read_price: Some(0.30),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 16_000,
                    max_images: Some(100),
                    max_document_size_mb: Some(100),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude Opus 4.1 (August 2025)
        self.register_model(
            "claude-opus-4-1-20250805",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-opus-4-1-20250805".to_string(),
                    name: "Claude Opus 4.1".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(32_000),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.015), // $15/1M input
                    output_cost_per_1k_tokens: Some(0.075), // $75/1M output
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
                family: AnthropicModelFamily::ClaudeOpus41,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                    ModelFeature::ThinkingMode,
                    ModelFeature::ComputerUse,
                ],
                pricing: ModelPricing {
                    input_price: 15.0,
                    output_price: 75.0,
                    cache_write_price: Some(18.75),
                    cache_read_price: Some(1.50),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 32_000,
                    max_images: Some(100),
                    max_document_size_mb: Some(100),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude Opus 4 (May 2025)
        self.register_model(
            "claude-opus-4-20250514",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-opus-4-20250514".to_string(),
                    name: "Claude Opus 4".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(32_000),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.015), // $15/1M input
                    output_cost_per_1k_tokens: Some(0.075), // $75/1M output
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
                family: AnthropicModelFamily::ClaudeOpus4,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                    ModelFeature::ThinkingMode,
                    ModelFeature::ComputerUse,
                ],
                pricing: ModelPricing {
                    input_price: 15.0,
                    output_price: 75.0,
                    cache_write_price: Some(18.75),
                    cache_read_price: Some(1.50),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 32_000,
                    max_images: Some(100),
                    max_document_size_mb: Some(100),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude 3.5 Haiku (October 2024)
        self.register_model(
            "claude-3-5-haiku-20241022",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-3-5-haiku-20241022".to_string(),
                    name: "Claude 3.5 Haiku".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(8_192),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.001),
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
                family: AnthropicModelFamily::Claude3Haiku,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                ],
                pricing: ModelPricing {
                    input_price: 1.0,
                    output_price: 5.0,
                    cache_write_price: Some(1.25),
                    cache_read_price: Some(0.10),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 8_192,
                    max_images: Some(20),
                    max_document_size_mb: Some(32),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude 3.5 Sonnet
        self.register_model(
            "claude-3-5-sonnet-20241022",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-3-5-sonnet-20241022".to_string(),
                    name: "Claude 3.5 Sonnet".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(8_192),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.003),
                    output_cost_per_1k_tokens: Some(0.015),
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
                family: AnthropicModelFamily::Claude35Sonnet,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                    ModelFeature::ThinkingMode,
                    ModelFeature::ComputerUse,
                ],
                pricing: ModelPricing {
                    input_price: 3.0,
                    output_price: 15.0,
                    cache_write_price: Some(3.75),
                    cache_read_price: Some(0.30),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 8_192,
                    max_images: Some(20),
                    max_document_size_mb: Some(32),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude 3 Opus
        self.register_model(
            "claude-3-opus-20240229",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-3-opus-20240229".to_string(),
                    name: "Claude 3 Opus".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(4_096),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.015),
                    output_cost_per_1k_tokens: Some(0.075),
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
                family: AnthropicModelFamily::Claude3Opus,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                ],
                pricing: ModelPricing {
                    input_price: 15.0,
                    output_price: 75.0,
                    cache_write_price: Some(18.75),
                    cache_read_price: Some(1.50),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 4_096,
                    max_images: Some(20),
                    max_document_size_mb: Some(32),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude 3 Sonnet
        self.register_model(
            "claude-3-sonnet-20240229",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-3-sonnet-20240229".to_string(),
                    name: "Claude 3 Sonnet".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(4_096),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.003),
                    output_cost_per_1k_tokens: Some(0.015),
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
                family: AnthropicModelFamily::Claude3Sonnet,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                ],
                pricing: ModelPricing {
                    input_price: 3.0,
                    output_price: 15.0,
                    cache_write_price: Some(3.75),
                    cache_read_price: Some(0.30),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 4_096,
                    max_images: Some(20),
                    max_document_size_mb: Some(32),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude 3 Haiku
        self.register_model(
            "claude-3-haiku-20240307",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-3-haiku-20240307".to_string(),
                    name: "Claude 3 Haiku".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(4_096),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.00025),
                    output_cost_per_1k_tokens: Some(0.00125),
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
                family: AnthropicModelFamily::Claude3Haiku,
                features: vec![
                    ModelFeature::MultimodalSupport,
                    ModelFeature::ToolCalling,
                    ModelFeature::FunctionCalling,
                    ModelFeature::StreamingSupport,
                    ModelFeature::CacheControl,
                    ModelFeature::SystemMessages,
                    ModelFeature::BatchProcessing,
                ],
                pricing: ModelPricing {
                    input_price: 0.25,
                    output_price: 1.25,
                    cache_write_price: Some(0.30),
                    cache_read_price: Some(0.03),
                    batch_discount: Some(0.5),
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 4_096,
                    max_images: Some(20),
                    max_document_size_mb: Some(32),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude 2.1
        self.register_model(
            "claude-2.1",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-2.1".to_string(),
                    name: "Claude 2.1".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 200_000,
                    max_output_length: Some(4_096),
                    supports_streaming: true,
                    supports_tools: false,
                    supports_multimodal: false,
                    input_cost_per_1k_tokens: Some(0.008),
                    output_cost_per_1k_tokens: Some(0.024),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: AnthropicModelFamily::Claude21,
                features: vec![ModelFeature::StreamingSupport, ModelFeature::SystemMessages],
                pricing: ModelPricing {
                    input_price: 8.0,
                    output_price: 24.0,
                    cache_write_price: None,
                    cache_read_price: None,
                    batch_discount: None,
                },
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 4_096,
                    max_images: None,
                    max_document_size_mb: None,
                },
                config: ModelConfig::default(),
            },
        );

        // Claude Instant
        self.register_model(
            "claude-instant-1.2",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-instant-1.2".to_string(),
                    name: "Claude Instant 1.2".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 100_000,
                    max_output_length: Some(4_096),
                    supports_streaming: true,
                    supports_tools: false,
                    supports_multimodal: false,
                    input_cost_per_1k_tokens: Some(0.0008),
                    output_cost_per_1k_tokens: Some(0.0024),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    ],
                    created_at: None,
                    updated_at: None,
                    metadata: std::collections::HashMap::new(),
                },
                family: AnthropicModelFamily::ClaudeInstant,
                features: vec![ModelFeature::StreamingSupport, ModelFeature::SystemMessages],
                pricing: ModelPricing {
                    input_price: 0.80,
                    output_price: 2.40,
                    cache_write_price: None,
                    cache_read_price: None,
                    batch_discount: None,
                },
                limits: ModelLimits {
                    max_context_length: 100_000,
                    max_output_tokens: 4_096,
                    max_images: None,
                    max_document_size_mb: None,
                },
                config: ModelConfig::default(),
            },
        );

        // Stable aliases and partner-platform naming variants.
        self.register_alias("claude-opus-4-6-20260205", "claude-opus-4-6");
        self.register_alias("claude-opus-4-5", "claude-opus-4-5-20251101");
        self.register_alias("claude-opus-4-5-20251110", "claude-opus-4-5-20251101");
        self.register_alias("claude-opus-4-1", "claude-opus-4-1-20250805");
        self.register_alias("claude-opus-4", "claude-opus-4-20250514");
        self.register_alias("claude-opus-4-0", "claude-opus-4-20250514");
        self.register_alias("claude-sonnet-4-6", "claude-sonnet-4-6-20251001");
        self.register_alias("claude-haiku-4-5", "claude-haiku-4-5-20251001");
        self.register_alias("claude-sonnet-4-5", "claude-sonnet-4-5-20250929");
        self.register_alias("claude-sonnet-4-5-20251101", "claude-sonnet-4-5-20250929");
        self.register_alias("claude-sonnet-4-0", "claude-sonnet-4-20250514");
        self.register_alias("claude-sonnet-4", "claude-sonnet-4-20250514");
        self.register_alias("claude-3-5-sonnet", "claude-3-5-sonnet-20241022");
        self.register_alias("claude-3.5-sonnet", "claude-3-5-sonnet-20241022");
        self.register_alias("claude-3-5-haiku", "claude-3-5-haiku-20241022");
        self.register_alias("claude-3.5-haiku", "claude-3-5-haiku-20241022");
        self.register_alias("claude-3-opus", "claude-3-opus-20240229");
        self.register_alias("claude-3-sonnet", "claude-3-sonnet-20240229");
        self.register_alias("claude-3-haiku", "claude-3-haiku-20240307");
    }

    /// Register a model
    fn register_model(&mut self, id: &str, spec: ModelSpec) {
        self.models.insert(id.to_string(), spec);
    }

    fn register_alias(&mut self, alias: &str, target: &str) {
        if let Some(spec) = self.models.get(target) {
            let mut alias_spec = spec.clone();
            alias_spec.model_info.id = alias.to_string();
            self.models.insert(alias.to_string(), alias_spec);
        }
    }

    /// Get model specification
    pub fn get_model_spec(&self, model_id: &str) -> Option<&ModelSpec> {
        self.models.get(model_id)
    }

    /// List all models
    pub fn list_models(&self) -> Vec<&ModelSpec> {
        self.models.values().collect()
    }

    /// Check if model supports feature
    pub fn supports_feature(&self, model_id: &str, feature: &ModelFeature) -> bool {
        self.get_model_spec(model_id)
            .map(|spec| spec.features.contains(feature))
            .unwrap_or(false)
    }

    /// Get model family
    pub fn get_model_family(&self, model_id: &str) -> Option<&AnthropicModelFamily> {
        self.get_model_spec(model_id).map(|spec| &spec.family)
    }

    /// Get model pricing
    pub fn get_model_pricing(&self, model_id: &str) -> Option<&ModelPricing> {
        self.get_model_spec(model_id).map(|spec| &spec.pricing)
    }

    /// Get model limits
    pub fn get_model_limits(&self, model_id: &str) -> Option<&ModelLimits> {
        self.get_model_spec(model_id).map(|spec| &spec.limits)
    }

    /// Get model family from name
    pub fn from_model_name(model_name: &str) -> Option<AnthropicModelFamily> {
        let model_lower = model_name.to_lowercase();

        // Check newest models first (most specific)
        if model_lower.contains("claude-opus-4-6") || model_lower.contains("claude-opus-4.6") {
            Some(AnthropicModelFamily::ClaudeOpus46)
        } else if model_lower.contains("claude-opus-4-5") || model_lower.contains("claude-opus-4.5")
        {
            Some(AnthropicModelFamily::ClaudeOpus45)
        } else if model_lower.contains("claude-opus-4-1") || model_lower.contains("claude-opus-4.1")
        {
            Some(AnthropicModelFamily::ClaudeOpus41)
        } else if model_lower.contains("claude-opus-4")
            && !model_lower.contains("claude-opus-4-1")
            && !model_lower.contains("claude-opus-4-5")
            && !model_lower.contains("claude-opus-4-6")
        {
            Some(AnthropicModelFamily::ClaudeOpus4)
        } else if model_lower.contains("claude-sonnet-4-6")
            || model_lower.contains("claude-sonnet-4.6")
        {
            Some(AnthropicModelFamily::ClaudeSonnet46)
        } else if model_lower.contains("claude-haiku-4-5")
            || model_lower.contains("claude-haiku-4.5")
        {
            Some(AnthropicModelFamily::ClaudeHaiku45)
        } else if model_lower.contains("claude-sonnet-4-5")
            || model_lower.contains("claude-sonnet-4.5")
        {
            Some(AnthropicModelFamily::ClaudeSonnet45)
        } else if model_lower.contains("claude-sonnet-4")
            && !model_lower.contains("claude-sonnet-4-5")
            && !model_lower.contains("claude-sonnet-4-6")
        {
            Some(AnthropicModelFamily::ClaudeSonnet4)
        } else if model_lower.contains("claude-3-5-sonnet")
            || model_lower.contains("claude-3.5-sonnet")
        {
            Some(AnthropicModelFamily::Claude35Sonnet)
        } else if model_lower.contains("claude-3-5-haiku")
            || model_lower.contains("claude-3.5-haiku")
        {
            Some(AnthropicModelFamily::Claude3Haiku)
        } else if model_lower.contains("claude-3-opus") {
            Some(AnthropicModelFamily::Claude3Opus)
        } else if model_lower.contains("claude-3-sonnet") {
            Some(AnthropicModelFamily::Claude3Sonnet)
        } else if model_lower.contains("claude-3-haiku") {
            Some(AnthropicModelFamily::Claude3Haiku)
        } else if model_lower.contains("claude-2.1") {
            Some(AnthropicModelFamily::Claude21)
        } else if model_lower.contains("claude-2") {
            Some(AnthropicModelFamily::Claude2)
        } else if model_lower.contains("claude-instant") {
            Some(AnthropicModelFamily::ClaudeInstant)
        } else {
            None
        }
    }
}

impl Default for AnthropicModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Get global model registry
pub fn get_anthropic_registry() -> &'static AnthropicModelRegistry {
    static REGISTRY: OnceLock<AnthropicModelRegistry> = OnceLock::new();
    REGISTRY.get_or_init(AnthropicModelRegistry::new)
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
        let registry = get_anthropic_registry();
        let pricing = registry.get_model_pricing(model_id)?;

        let input_cost = (prompt_tokens as f64 / 1_000_000.0) * pricing.input_price;
        let output_cost = (completion_tokens as f64 / 1_000_000.0) * pricing.output_price;

        Some(input_cost + output_cost)
    }

    /// Calculate extended cost (including cache)
    pub fn calculate_extended_cost(
        model_id: &str,
        prompt_tokens: u32,
        completion_tokens: u32,
        cache_read_tokens: Option<u32>,
        cache_write_tokens: Option<u32>,
        is_batch: bool,
    ) -> Option<f64> {
        let registry = get_anthropic_registry();
        let pricing = registry.get_model_pricing(model_id)?;

        let batch_multiplier = if is_batch {
            pricing.batch_discount.unwrap_or(1.0)
        } else {
            1.0
        };

        let mut total_cost = 0.0;
        let mut remaining_prompt_tokens = prompt_tokens;

        // Handle cache read tokens
        if let (Some(cache_read), Some(cache_read_price)) =
            (cache_read_tokens, pricing.cache_read_price)
        {
            let cache_cost = (cache_read as f64 / 1_000_000.0) * cache_read_price;
            total_cost += cache_cost;
            remaining_prompt_tokens = remaining_prompt_tokens.saturating_sub(cache_read);
        }

        // Handle cache write tokens
        if let (Some(cache_write), Some(cache_write_price)) =
            (cache_write_tokens, pricing.cache_write_price)
        {
            let cache_write_cost =
                (cache_write as f64 / 1_000_000.0) * cache_write_price * batch_multiplier;
            total_cost += cache_write_cost;
            remaining_prompt_tokens = remaining_prompt_tokens.saturating_sub(cache_write);
        }

        // Regular input tokens
        let input_cost =
            (remaining_prompt_tokens as f64 / 1_000_000.0) * pricing.input_price * batch_multiplier;
        total_cost += input_cost;

        // Output tokens
        let output_cost =
            (completion_tokens as f64 / 1_000_000.0) * pricing.output_price * batch_multiplier;
        total_cost += output_cost;

        Some(total_cost)
    }

    /// Estimate token count
    pub fn estimate_tokens(text: &str) -> u32 {
        // Anthropic uses approximately 4 characters = 1 token ratio (English)
        (text.len() as f32 / 4.0).ceil() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_registry() {
        let registry = get_anthropic_registry();

        // Test latest flagship model
        let opus_spec = registry.get_model_spec("claude-opus-4-6").unwrap();
        assert_eq!(opus_spec.family, AnthropicModelFamily::ClaudeOpus46);
        assert!(
            opus_spec
                .features
                .contains(&ModelFeature::MultimodalSupport)
        );
        assert!(opus_spec.features.contains(&ModelFeature::ComputerUse));

        // Test pricing
        assert_eq!(opus_spec.pricing.input_price, 5.0);
        assert_eq!(opus_spec.pricing.output_price, 25.0);
    }

    #[test]
    fn test_model_family_detection() {
        assert_eq!(
            AnthropicModelRegistry::from_model_name("claude-opus-4-6"),
            Some(AnthropicModelFamily::ClaudeOpus46)
        );

        assert_eq!(
            AnthropicModelRegistry::from_model_name("claude-3-5-sonnet-20241022"),
            Some(AnthropicModelFamily::Claude35Sonnet)
        );

        assert_eq!(
            AnthropicModelRegistry::from_model_name("claude-3-opus-20240229"),
            Some(AnthropicModelFamily::Claude3Opus)
        );

        assert_eq!(
            AnthropicModelRegistry::from_model_name("unknown-model"),
            None
        );
    }

    #[test]
    fn test_cost_calculation() {
        let cost = CostCalculator::calculate_cost("claude-opus-4-6", 1000, 500);
        assert!(cost.is_some());

        let cost_value = cost.unwrap();
        // Expected: (1000/1M * $5) + (500/1M * $25) = $0.005 + $0.0125 = $0.0175
        assert!((cost_value - 0.0175).abs() < 0.0001);
    }

    #[test]
    fn test_feature_support() {
        let registry = get_anthropic_registry();

        // Claude Opus 4.6 supports computer tools
        assert!(registry.supports_feature("claude-opus-4-6", &ModelFeature::ComputerUse));

        // Claude 2.1 does not support computer tools
        assert!(!registry.supports_feature("claude-2.1", &ModelFeature::ComputerUse));
    }
}
