use super::{
    AnthropicModelFamily, AnthropicModelRegistry, ModelConfig, ModelFeature, ModelLimits,
    ModelSpec, pricing_per_million,
};
use crate::core::types::model::ModelInfo;

mod claude5;

impl AnthropicModelRegistry {
    /// Initialize model registry
    pub(super) fn initialize_models(&mut self) {
        claude5::register(self);

        // Claude Opus 4.8 (generally available flagship - May 2026)
        // Pricing source: https://docs.anthropic.com/en/docs/about-claude/pricing
        self.register_model(
            "claude-opus-4-8",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-opus-4-8".to_string(),
                    name: "Claude Opus 4.8".to_string(),
                    provider: "anthropic".to_string(),
                    max_context_length: 1_000_000,
                    max_output_length: Some(128_000),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: true,
                    input_cost_per_1k_tokens: Some(0.005),
                    output_cost_per_1k_tokens: Some(0.025),
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
                family: AnthropicModelFamily::ClaudeOpus48,
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
                pricing: pricing_per_million(5.0, 25.0, Some(6.25), Some(0.50), Some(0.5)),
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 128_000,
                    max_images: Some(100),
                    max_document_size_mb: Some(100),
                },
                config: ModelConfig::default(),
            },
        );

        if let Some(mut spec) = self.models.get("claude-opus-4-8").cloned() {
            spec.model_info.id = "claude-opus-4-7".to_string();
            spec.model_info.name = "Claude Opus 4.7".to_string();
            spec.family = AnthropicModelFamily::ClaudeOpus47;
            self.register_model("claude-opus-4-7", spec);
        }

        // Claude Opus 4.6 (Previous flagship model - January 2026)
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
                pricing: pricing_per_million(5.0, 25.0, Some(6.25), Some(0.50), Some(0.5)),
                limits: ModelLimits {
                    max_context_length: 1_000_000,
                    max_output_tokens: 128_000,
                    max_images: Some(100),
                    max_document_size_mb: Some(100),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude Opus 4.5 (Previous flagship model - November 2025)
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
                pricing: pricing_per_million(5.0, 25.0, Some(6.25), Some(0.50), Some(0.5)),
                limits: ModelLimits {
                    max_context_length: 200_000,
                    max_output_tokens: 64_000,
                    max_images: Some(100),
                    max_document_size_mb: Some(100),
                },
                config: ModelConfig::default(),
            },
        );

        // Claude Sonnet 4.5 (Earlier balanced model - September 2025)
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
                pricing: pricing_per_million(3.0, 15.0, Some(3.75), Some(0.30), Some(0.5)),
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
            "claude-sonnet-4-6",
            ModelSpec {
                model_info: ModelInfo {
                    id: "claude-sonnet-4-6".to_string(),
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
                pricing: pricing_per_million(3.0, 15.0, Some(3.75), Some(0.30), Some(0.5)),
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
                pricing: pricing_per_million(1.0, 5.0, Some(1.25), Some(0.10), Some(0.5)),
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
                pricing: pricing_per_million(3.0, 15.0, Some(3.75), Some(0.30), Some(0.5)),
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
                pricing: pricing_per_million(15.0, 75.0, Some(18.75), Some(1.50), Some(0.5)),
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
                pricing: pricing_per_million(15.0, 75.0, Some(18.75), Some(1.50), Some(0.5)),
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
                pricing: pricing_per_million(1.0, 5.0, Some(1.25), Some(0.10), Some(0.5)),
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
                pricing: pricing_per_million(3.0, 15.0, Some(3.75), Some(0.30), Some(0.5)),
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
                pricing: pricing_per_million(15.0, 75.0, Some(18.75), Some(1.50), Some(0.5)),
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
                pricing: pricing_per_million(3.0, 15.0, Some(3.75), Some(0.30), Some(0.5)),
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
                pricing: pricing_per_million(0.25, 1.25, Some(0.30), Some(0.03), Some(0.5)),
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
                pricing: pricing_per_million(8.0, 24.0, None, None, None),
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
                pricing: pricing_per_million(0.80, 2.40, None, None, None),
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
        self.register_alias("claude-opus-4-7-latest", "claude-opus-4-7");
        self.register_alias("claude-opus-4-6-20260205", "claude-opus-4-6");
        self.register_alias("claude-opus-4-5", "claude-opus-4-5-20251101");
        self.register_alias("claude-opus-4-5-20251110", "claude-opus-4-5-20251101");
        self.register_alias("claude-opus-4-1", "claude-opus-4-1-20250805");
        self.register_alias("claude-opus-4", "claude-opus-4-20250514");
        self.register_alias("claude-opus-4-0", "claude-opus-4-20250514");
        self.register_alias("claude-sonnet-4-6-20251001", "claude-sonnet-4-6");
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
}
