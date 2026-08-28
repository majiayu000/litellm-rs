//! Anthropic Model Registry
//!
//! Unified model registry system with integrated pricing and capability information

use std::collections::HashMap;
use std::sync::OnceLock;

pub use crate::core::cost::types::ModelPricing;
mod catalog;
mod cost;

pub use cost::CostCalculator;

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
    /// Claude Fable 5 models
    ClaudeFable5,
    /// Claude Opus 5 models
    ClaudeOpus5,
    /// Claude Sonnet 5 models
    ClaudeSonnet5,
    /// Claude Opus 4.8 models
    ClaudeOpus48,
    /// Claude Opus 4.7 models
    ClaudeOpus47,
    /// Claude Opus 4.6 models
    ClaudeOpus46,
    /// Claude Opus 4.5 models
    ClaudeOpus45,
    /// Claude Sonnet 4.6 models
    ClaudeSonnet46,
    /// Claude Sonnet 4.5 models (earlier balanced)
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

pub(super) fn pricing_per_million(
    input_price: f64,
    output_price: f64,
    cache_write_price: Option<f64>,
    cache_read_price: Option<f64>,
    batch_discount: Option<f64>,
) -> ModelPricing {
    ModelPricing {
        input_cost_per_1k_tokens: input_price / 1000.0,
        output_cost_per_1k_tokens: output_price / 1000.0,
        cache_creation_input_token_cost: cache_write_price.map(|price| price / 1000.0),
        cache_read_input_token_cost: cache_read_price.map(|price| price / 1000.0),
        batch_discount,
        currency: "USD".to_string(),
        updated_at: chrono::Utc::now(),
        ..Default::default()
    }
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

    /// Register a model
    pub(super) fn register_model(&mut self, id: &str, mut spec: ModelSpec) {
        spec.pricing.model = id.to_string();
        self.models.insert(id.to_string(), spec);
    }

    pub(super) fn register_alias(&mut self, alias: &str, target: &str) {
        if let Some(spec) = self.models.get(target) {
            let mut alias_spec = spec.clone();
            alias_spec.model_info.id = alias.to_string();
            alias_spec.pricing.model = alias.to_string();
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

    /// Get model pricing in the shared core cost model shape.
    pub fn get_core_model_pricing(&self, model_id: &str) -> Option<ModelPricing> {
        self.get_model_spec(model_id)
            .map(|spec| spec.pricing.clone())
    }

    /// Get model limits
    pub fn get_model_limits(&self, model_id: &str) -> Option<&ModelLimits> {
        self.get_model_spec(model_id).map(|spec| &spec.limits)
    }

    /// Get model family from name
    pub fn from_model_name(model_name: &str) -> Option<AnthropicModelFamily> {
        match model_name {
            "claude-fable-5" => return Some(AnthropicModelFamily::ClaudeFable5),
            "claude-opus-5" => return Some(AnthropicModelFamily::ClaudeOpus5),
            "claude-sonnet-5" => return Some(AnthropicModelFamily::ClaudeSonnet5),
            _ => {}
        }

        let model_lower = model_name.to_lowercase();

        // Check newest models first (most specific)
        if model_lower.contains("claude-opus-4-8") || model_lower.contains("claude-opus-4.8") {
            Some(AnthropicModelFamily::ClaudeOpus48)
        } else if model_lower.contains("claude-opus-4-7") || model_lower.contains("claude-opus-4.7")
        {
            Some(AnthropicModelFamily::ClaudeOpus47)
        } else if model_lower.contains("claude-opus-4-6") || model_lower.contains("claude-opus-4.6")
        {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::model::ProviderCapability;

    #[test]
    fn test_current_claude_5_catalog() {
        let registry = get_anthropic_registry();
        let cases = [
            (
                "claude-fable-5",
                AnthropicModelFamily::ClaudeFable5,
                0.010,
                0.050,
                0.0125,
                0.001,
            ),
            (
                "claude-opus-5",
                AnthropicModelFamily::ClaudeOpus5,
                0.005,
                0.025,
                0.00625,
                0.0005,
            ),
            (
                "claude-sonnet-5",
                AnthropicModelFamily::ClaudeSonnet5,
                0.002,
                0.010,
                0.0025,
                0.0002,
            ),
        ];

        for (id, family, input, output, cache_write, cache_read) in cases {
            let spec = registry
                .get_model_spec(id)
                .unwrap_or_else(|| panic!("{id} should be registered"));

            assert_eq!(spec.model_info.id, id);
            assert_eq!(spec.family, family);
            assert_eq!(spec.model_info.max_context_length, 1_000_000);
            assert_eq!(spec.model_info.max_output_length, Some(128_000));
            assert!(spec.model_info.supports_streaming);
            assert!(spec.model_info.supports_tools);
            assert!(spec.model_info.supports_multimodal);
            assert!(
                !spec
                    .model_info
                    .capabilities
                    .contains(&ProviderCapability::FunctionCalling),
                "legacy OpenAI functions are not an Anthropic wire capability"
            );
            assert!(
                !spec.features.contains(&ModelFeature::FunctionCalling),
                "legacy OpenAI functions must fail closed for Claude 5"
            );
            assert!(
                !spec.features.contains(&ModelFeature::ThinkingMode),
                "Claude 5 does not support manual budget-token thinking"
            );
            assert_eq!(
                spec.model_info.metadata.get("supports_adaptive_thinking"),
                Some(&serde_json::Value::Bool(true))
            );
            assert_eq!(
                spec.model_info
                    .metadata
                    .get("supports_manual_extended_thinking"),
                Some(&serde_json::Value::Bool(false))
            );
            assert!(spec.features.contains(&ModelFeature::ComputerUse));
            assert_eq!(spec.pricing.input_cost_per_1k_tokens, input);
            assert_eq!(spec.pricing.output_cost_per_1k_tokens, output);
            assert_eq!(
                spec.pricing.cache_creation_input_token_cost,
                Some(cache_write)
            );
            assert_eq!(spec.pricing.cache_read_input_token_cost, Some(cache_read));
            assert_eq!(spec.pricing.batch_discount, Some(0.5));
            assert_eq!(AnthropicModelRegistry::from_model_name(id), Some(family));
        }
    }

    #[test]
    fn test_current_claude_5_ids_are_exact_and_public() {
        let registry = get_anthropic_registry();

        for unsupported in [
            "claude-fable-5-20260801",
            "claude-opus-5-latest",
            "claude-opus-5-20260724",
            "claude-sonnet-5-preview",
            "prefix-claude-sonnet-5",
            "claude-mythos-5",
        ] {
            assert!(
                registry.get_model_spec(unsupported).is_none(),
                "{unsupported} must not be registered"
            );
            assert_eq!(
                AnthropicModelRegistry::from_model_name(unsupported),
                None,
                "{unsupported} must not resolve to a Claude 5 family"
            );
        }
    }

    #[test]
    fn test_model_registry() {
        let registry = get_anthropic_registry();

        // Test latest flagship model
        let opus_spec = registry.get_model_spec("claude-opus-4-8").unwrap();
        assert_eq!(opus_spec.family, AnthropicModelFamily::ClaudeOpus48);
        assert!(
            opus_spec
                .features
                .contains(&ModelFeature::MultimodalSupport)
        );
        assert!(opus_spec.features.contains(&ModelFeature::ComputerUse));

        // Test pricing
        assert_eq!(opus_spec.pricing.input_cost_per_1k_tokens, 0.005);
        assert_eq!(opus_spec.pricing.output_cost_per_1k_tokens, 0.025);
    }

    #[test]
    fn test_opus47_alias_and_limits() {
        let registry = get_anthropic_registry();

        let Some(alias_spec) = registry.get_model_spec("claude-opus-4-7-latest") else {
            panic!("claude-opus-4-7-latest should alias claude-opus-4-7");
        };
        assert_eq!(alias_spec.family, AnthropicModelFamily::ClaudeOpus47);
        assert_eq!(alias_spec.model_info.max_context_length, 1_000_000);
        assert_eq!(alias_spec.model_info.max_output_length, Some(128_000));

        let Some(limits) = registry.get_model_limits("claude-opus-4-7-latest") else {
            panic!("claude-opus-4-7-latest should expose Opus 4.7 limits");
        };
        assert_eq!(limits.max_context_length, 1_000_000);
        assert_eq!(limits.max_output_tokens, 128_000);

        // Verify the alias resolves to a spec whose pricing matches the canonical Opus 4.7 entry.
        // The alias spec carries its own id (`claude-opus-4-7-latest`), but the underlying
        // pricing/cost numbers must match the canonical `claude-opus-4-7` entry so callers
        // who route by alias do not see a cheaper or pricier model than the canonical id.
        let canonical_spec = registry
            .get_model_spec("claude-opus-4-7")
            .expect("canonical claude-opus-4-7 should exist");
        assert_eq!(
            alias_spec.model_info.input_cost_per_1k_tokens,
            canonical_spec.model_info.input_cost_per_1k_tokens,
            "alias must share input cost with canonical"
        );
        assert_eq!(
            alias_spec.model_info.output_cost_per_1k_tokens,
            canonical_spec.model_info.output_cost_per_1k_tokens,
            "alias must share output cost with canonical"
        );
        assert_eq!(
            alias_spec.pricing.input_cost_per_1k_tokens,
            canonical_spec.pricing.input_cost_per_1k_tokens,
            "alias must share pricing.input with canonical"
        );
        assert_eq!(
            alias_spec.pricing.output_cost_per_1k_tokens,
            canonical_spec.pricing.output_cost_per_1k_tokens,
            "alias must share pricing.output with canonical"
        );
        assert_eq!(
            alias_spec.family, canonical_spec.family,
            "alias must share family with canonical"
        );
    }

    #[test]
    fn test_core_model_pricing_conversion() {
        let registry = get_anthropic_registry();
        let pricing = registry
            .get_core_model_pricing("claude-opus-4-7")
            .expect("registry pricing should convert to core pricing");

        assert_eq!(pricing.model, "claude-opus-4-7");
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.005);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.025);
        assert_eq!(pricing.cache_creation_input_token_cost, Some(0.00625));
        assert_eq!(pricing.cache_read_input_token_cost, Some(0.0005));
        assert_eq!(pricing.batch_discount, Some(0.5));
        assert_eq!(pricing.currency, "USD");
    }

    #[test]
    fn test_model_family_detection() {
        assert_eq!(
            AnthropicModelRegistry::from_model_name("claude-opus-4-8"),
            Some(AnthropicModelFamily::ClaudeOpus48)
        );

        assert_eq!(
            AnthropicModelRegistry::from_model_name("claude-opus-4-7"),
            Some(AnthropicModelFamily::ClaudeOpus47)
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
        let cost = CostCalculator::calculate_cost("claude-opus-4-8", 1000, 500);
        assert!(cost.is_some());

        let cost_value = cost.unwrap();
        // Expected: (1000/1M * $5) + (500/1M * $25) = $0.005 + $0.0125 = $0.0175
        assert!((cost_value - 0.0175).abs() < 0.0001);
    }

    #[test]
    fn test_feature_support() {
        let registry = get_anthropic_registry();

        // Claude Opus 4.8 supports computer tools
        assert!(registry.supports_feature("claude-opus-4-8", &ModelFeature::ComputerUse));

        // Claude 2.1 does not support computer tools
        assert!(!registry.supports_feature("claude-2.1", &ModelFeature::ComputerUse));
    }
}
