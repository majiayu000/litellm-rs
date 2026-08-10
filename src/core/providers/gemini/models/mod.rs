//! Gemini Model Registry
//!
//! Unified model registry system containing capabilities and pricing information for all Gemini models

use std::collections::HashMap;
use std::sync::OnceLock;

mod catalog;
mod surface;

pub use surface::GoogleGeminiApiSurface;

pub use crate::core::cost::types::ModelPricing;
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
    /// Gemini 3.5 series (2026 - Latest)
    Gemini35Flash,

    /// Gemini 3.1 series (2026)
    Gemini31ProPreview,
    Gemini31Flash,
    Gemini31FlashLite,

    /// Gemini 3 series (2025-2026)
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

fn pricing_per_million(
    input_price: f64,
    output_price: f64,
    cached_input_price: Option<f64>,
    image_price: Option<f64>,
    video_price_per_second: Option<f64>,
    audio_price_per_second: Option<f64>,
) -> ModelPricing {
    ModelPricing {
        input_cost_per_1k_tokens: input_price / 1000.0,
        output_cost_per_1k_tokens: output_price / 1000.0,
        cache_read_input_token_cost: cached_input_price.map(|price| price / 1000.0),
        cost_per_image: image_price.map(|price| {
            let mut costs = HashMap::new();
            costs.insert("default".to_string(), price);
            costs
        }),
        video_cost_per_second: video_price_per_second,
        audio_cost_per_second: audio_price_per_second,
        currency: "USD".to_string(),
        updated_at: chrono::Utc::now(),
        ..Default::default()
    }
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
    const EXPECTED_MODEL_COUNT: usize = 17;

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
        catalog::register_all(self);
    }

    fn register_model(&mut self, id: &str, mut spec: ModelSpec) {
        spec.pricing.model = id.to_string();
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

    /// List model metadata for a concrete Google API surface.
    pub fn list_model_infos_for_surface(&self, surface: GoogleGeminiApiSurface) -> Vec<ModelInfo> {
        let mut models = self
            .models
            .values()
            .filter(|spec| surface.includes(spec))
            .map(|spec| surface.overlay_model_info(spec))
            .collect::<Vec<_>>();
        models.sort_by(|left, right| left.id.cmp(&right.id));
        models
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

    /// Get model pricing in the shared core cost model shape.
    pub fn get_core_model_pricing(&self, model_id: &str) -> Option<ModelPricing> {
        self.get_model_spec(model_id)
            .map(|spec| spec.pricing.clone())
    }

    /// Model
    pub fn get_model_limits(&self, model_id: &str) -> Option<&ModelLimits> {
        self.get_model_spec(model_id).map(|spec| &spec.limits)
    }

    /// Detect model family from model name string
    pub fn from_model_name(model_name: &str) -> Option<GeminiModelFamily> {
        let model_lower = model_name.to_lowercase();

        // Gemini 3.5 series
        if model_lower.contains("gemini-3.5-flash") {
            Some(GeminiModelFamily::Gemini35Flash)
        }
        // Gemini 3.1 series (check before 3.0 as more specific)
        else if model_lower.contains("gemini-3.1-flash-lite") {
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
    ) -> Result<f64, crate::ProviderError> {
        super::calculate_gemini_cost(model_id, prompt_tokens, completion_tokens)
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
        let pricing = registry.get_core_model_pricing(model_id)?;

        let mut total_cost = 0.0;
        let mut remaining_prompt_tokens = prompt_tokens;

        // Handle
        if let (Some(cached), Some(cached_price)) =
            (cached_tokens, pricing.cache_read_input_token_cost)
        {
            let cached_cost = (cached as f64 / 1000.0) * cached_price;
            total_cost += cached_cost;
            remaining_prompt_tokens = remaining_prompt_tokens.saturating_sub(cached);
        }

        // Regular input tokens
        let input_cost =
            (remaining_prompt_tokens as f64 / 1000.0) * pricing.input_cost_per_1k_tokens;
        total_cost += input_cost;

        // Output tokens
        let output_cost = (completion_tokens as f64 / 1000.0) * pricing.output_cost_per_1k_tokens;
        total_cost += output_cost;

        // Image cost
        let image_price = pricing
            .cost_per_image
            .as_ref()
            .and_then(|costs| costs.get("default"))
            .copied();
        if let (Some(img_count), Some(img_price)) = (images, image_price) {
            total_cost += img_count as f64 * img_price;
        }

        // Video cost
        if let (Some(video_secs), Some(video_price)) =
            (video_seconds, pricing.video_cost_per_second)
        {
            total_cost += video_secs as f64 * video_price;
        }

        // Audio cost
        if let (Some(audio_secs), Some(audio_price)) =
            (audio_seconds, pricing.audio_cost_per_second)
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
    use crate::core::types::model::ProviderCapability;

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
        assert_eq!(flash_spec.pricing.input_cost_per_1k_tokens, 0.00001);
        assert_eq!(flash_spec.pricing.output_cost_per_1k_tokens, 0.00004);
    }

    #[test]
    fn test_model_family_detection() {
        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-3.5-flash"),
            Some(GeminiModelFamily::Gemini35Flash)
        );

        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-2.0-flash-exp"),
            Some(GeminiModelFamily::Gemini20Flash)
        );

        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-3.1-flash"),
            Some(GeminiModelFamily::Gemini31Flash)
        );

        assert_eq!(
            GeminiModelRegistry::from_model_name("gemini-1.5-pro-latest"),
            Some(GeminiModelFamily::Gemini15Pro)
        );

        assert_eq!(GeminiModelRegistry::from_model_name("unknown-model"), None);
    }

    #[test]
    fn test_cost_calculation() {
        let cost = CostCalculator::calculate_cost("gemini-2.5-flash", 1000, 500);
        assert!(cost.is_ok());

        let cost_value = cost.unwrap();
        // Expected: (1000/1M * $0.30) + (500/1M * $2.50) = $0.0003 + $0.00125 = $0.00155
        assert!((cost_value - 0.00155).abs() < 0.000001);
    }

    #[test]
    fn test_cost_calculation_does_not_use_registry_fallback() {
        assert!(matches!(
            CostCalculator::calculate_cost("gemini-1.0-pro", 1000, 500),
            Err(crate::ProviderError::ModelNotFound { .. })
        ));
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
        assert_eq!(pricing_value.input_cost_per_1k_tokens, 0.000075);
        assert_eq!(pricing_value.output_cost_per_1k_tokens, 0.0003);
        assert!(pricing_value.cache_read_input_token_cost.is_some());
    }

    #[test]
    fn test_core_model_pricing_conversion() {
        let registry = get_gemini_registry();
        let pricing = registry
            .get_core_model_pricing("gemini-1.5-flash")
            .expect("registry pricing should convert to core pricing");

        assert_eq!(pricing.model, "gemini-1.5-flash");
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.000075);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.0003);
        assert_eq!(pricing.cache_read_input_token_cost, Some(0.00001875));
        assert_eq!(pricing.video_cost_per_second, Some(0.0002));
        assert_eq!(pricing.audio_cost_per_second, Some(0.0001));
        assert_eq!(
            pricing
                .cost_per_image
                .as_ref()
                .and_then(|costs| costs.get("default"))
                .copied(),
            Some(0.0002)
        );
        assert_eq!(pricing.currency, "USD");
    }

    #[test]
    fn test_gemini_35_flash_metadata() {
        let registry = get_gemini_registry();
        let Some(spec) = registry.get_model_spec("gemini-3.5-flash") else {
            panic!("gemini-3.5-flash should be registered");
        };

        assert_eq!(spec.family, GeminiModelFamily::Gemini35Flash);
        assert_eq!(spec.limits.max_context_length, 1_048_576);
        assert_eq!(spec.limits.max_output_tokens, 65_536);
        assert_eq!(spec.limits.max_images, Some(3000));
        assert_eq!(spec.limits.max_video_seconds, Some(3600));
        assert_eq!(spec.limits.max_audio_seconds, Some(9600));
        assert_eq!(spec.limits.rpm_limit, None);
        assert_eq!(spec.limits.tpm_limit, None);
        assert!(spec.features.contains(&ModelFeature::ToolCalling));
        assert!(spec.features.contains(&ModelFeature::ContextCaching));
        assert!(spec.features.contains(&ModelFeature::SearchGrounding));
        assert_eq!(spec.pricing.input_cost_per_1k_tokens, 0.0015);
        assert_eq!(spec.pricing.output_cost_per_1k_tokens, 0.009);
        assert_eq!(spec.pricing.cache_read_input_token_cost, Some(0.00015));
        assert_eq!(spec.pricing.cost_per_image, None);
        assert_eq!(spec.pricing.video_cost_per_second, None);
        assert_eq!(spec.pricing.audio_cost_per_second, None);
    }

    #[test]
    fn test_gemini_family_capabilities_include_declared_advanced_features() {
        let registry = get_gemini_registry();
        let full_capability_models = [
            "gemini-2.5-pro",
            "gemini-2.5-flash",
            "gemini-2.5-flash-lite",
            "gemini-3-pro",
            "gemini-3-pro-deep-think",
            "gemini-3-flash-preview",
            "gemini-3.1-pro-preview",
            "gemini-3.1-flash",
            "gemini-3.1-flash-lite",
            "gemini-2.0-flash-exp",
            "gemini-1.5-pro",
            "gemini-1.5-flash",
            "gemini-1.5-flash-8b",
        ];

        for model_id in full_capability_models {
            let Some(spec) = registry.get_model_spec(model_id) else {
                panic!("{model_id} should be registered");
            };
            for capability in [
                ProviderCapability::FunctionCalling,
                ProviderCapability::CodeExecution,
                ProviderCapability::BatchProcessing,
            ] {
                assert!(
                    spec.model_info.capabilities.contains(&capability),
                    "{model_id} should expose {capability:?}"
                );
            }
        }

        let Some(gemini_10) = registry.get_model_spec("gemini-1.0-pro") else {
            panic!("gemini-1.0-pro should be registered");
        };
        assert!(
            gemini_10
                .model_info
                .capabilities
                .contains(&ProviderCapability::FunctionCalling)
        );
        assert!(
            gemini_10
                .model_info
                .capabilities
                .contains(&ProviderCapability::BatchProcessing)
        );
        assert!(
            !gemini_10
                .model_info
                .capabilities
                .contains(&ProviderCapability::CodeExecution)
        );
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
        assert!(matches!(
            cost,
            Err(crate::ProviderError::ModelNotFound { .. })
        ));
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
