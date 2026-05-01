//! AI21 Model Registry
//!
//! Model registry system for AI21 Labs models (Jamba family)

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::core::pricing::get_pricing_db;
use crate::core::types::model::ModelInfo;

/// Model feature flags
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModelFeature {
    /// Function calling support
    FunctionCalling,
    /// Vision support
    VisionSupport,
    /// Streaming support
    StreamingSupport,
    /// System message support
    SystemMessages,
    /// Tool calling support
    ToolCalling,
    /// Document context support (AI21 specific)
    DocumentSupport,
}

/// Model specification with features
#[derive(Debug, Clone)]
pub struct ModelSpec {
    /// Model information
    pub model_info: ModelInfo,
    /// Supported features
    pub features: Vec<ModelFeature>,
    /// Model-specific configuration
    pub config: ModelConfig,
}

/// Model-specific configuration
#[derive(Debug, Clone, Default)]
pub struct ModelConfig {
    /// Maximum concurrent requests
    pub max_concurrent_requests: Option<u32>,
    /// Custom parameter mapping
    pub custom_params: HashMap<String, String>,
}

/// AI21 Model Registry
pub struct AI21ModelRegistry {
    models: HashMap<String, ModelSpec>,
}

impl Default for AI21ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AI21ModelRegistry {
    /// Create new registry
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
        };
        registry.load_models();
        registry
    }

    /// Load models from pricing database or defaults
    fn load_models(&mut self) {
        let pricing_db = get_pricing_db();
        let model_ids = pricing_db.get_provider_models("ai21");

        for model_id in &model_ids {
            if let Some(model_info) = pricing_db.to_model_info(model_id, "ai21") {
                let features = self.detect_features(&model_info);
                let config = self.create_config(&model_info);

                self.models.insert(
                    model_id.clone(),
                    ModelSpec {
                        model_info,
                        features,
                        config,
                    },
                );
            }
        }

        // Add default models if none loaded
        if self.models.is_empty() {
            self.add_default_models();
        }
    }

    /// Detect features based on model info
    fn detect_features(&self, model_info: &ModelInfo) -> Vec<ModelFeature> {
        let mut features = vec![ModelFeature::SystemMessages, ModelFeature::StreamingSupport];

        if model_info.supports_tools {
            features.push(ModelFeature::FunctionCalling);
            features.push(ModelFeature::ToolCalling);
        }

        if model_info.supports_multimodal {
            features.push(ModelFeature::VisionSupport);
        }

        // AI21 Jamba models support document context
        if model_info.id.contains("jamba") {
            features.push(ModelFeature::DocumentSupport);
        }

        features
    }

    /// Create model configuration
    fn create_config(&self, model_info: &ModelInfo) -> ModelConfig {
        ModelConfig {
            max_concurrent_requests: Some(match model_info.id.as_str() {
                "jamba-1.5-large" => 5,
                "jamba-1.5-mini" => 10,
                _ => 5,
            }),
            ..Default::default()
        }
    }

    /// Add default AI21 models
    fn add_default_models(&mut self) {
        // AI21 Jamba models (as of 2025)
        let default_models = vec![
            // Jamba 1.5 Large - flagship model
            (
                "jamba-1.5-large",
                "Jamba 1.5 Large",
                256_000,
                Some(4_096),
                0.002, // $2/1M input tokens
                0.008, // $8/1M output tokens
                true,  // supports tools
                false, // no multimodal
            ),
            // Jamba 1.5 Mini - faster, cost-effective
            (
                "jamba-1.5-mini",
                "Jamba 1.5 Mini",
                256_000,
                Some(4_096),
                0.0002, // $0.2/1M input tokens
                0.0004, // $0.4/1M output tokens
                true,   // supports tools
                false,  // no multimodal
            ),
            // Jamba Instruct (older model)
            (
                "jamba-instruct",
                "Jamba Instruct",
                256_000,
                Some(4_096),
                0.0005,
                0.0007,
                true,
                false,
            ),
        ];

        for (
            id,
            name,
            context_len,
            output_len,
            input_cost,
            output_cost,
            supports_tools,
            supports_multimodal,
        ) in default_models
        {
            let model_info = ModelInfo {
                id: id.to_string(),
                name: name.to_string(),
                provider: "ai21".to_string(),
                max_context_length: context_len,
                max_output_length: output_len,
                supports_streaming: true,
                supports_tools,
                supports_multimodal,
                input_cost_per_1k_tokens: Some(input_cost),
                output_cost_per_1k_tokens: Some(output_cost),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            };

            let features = self.detect_features(&model_info);
            let config = self.create_config(&model_info);

            self.models.insert(
                id.to_string(),
                ModelSpec {
                    model_info,
                    features,
                    config,
                },
            );
        }
    }

    /// Get all models
    pub fn get_all_models(&self) -> Vec<ModelInfo> {
        self.models
            .values()
            .map(|spec| spec.model_info.clone())
            .collect()
    }

    /// Get model specification
    pub fn get_model_spec(&self, model_id: &str) -> Option<&ModelSpec> {
        self.models.get(model_id)
    }

    /// Check if model supports a feature
    pub fn supports_feature(&self, model_id: &str, feature: &ModelFeature) -> bool {
        self.models
            .get(model_id)
            .map(|spec| spec.features.contains(feature))
            .unwrap_or(false)
    }

    /// Get models with a specific feature
    pub fn get_models_with_feature(&self, feature: &ModelFeature) -> Vec<String> {
        self.models
            .iter()
            .filter_map(|(id, spec)| {
                if spec.features.contains(feature) {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

/// Global registry instance
static AI21_REGISTRY: OnceLock<AI21ModelRegistry> = OnceLock::new();

/// Get global AI21 model registry
pub fn get_ai21_registry() -> &'static AI21ModelRegistry {
    AI21_REGISTRY.get_or_init(AI21ModelRegistry::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_registry_creation() {
        let registry = AI21ModelRegistry::new();
        assert!(!registry.get_all_models().is_empty());
    }

    #[test]
    fn test_feature_detection() {
        let registry = get_ai21_registry();
        let models = registry.get_all_models();

        assert!(!models.is_empty());

        for model in &models {
            assert!(registry.supports_feature(&model.id, &ModelFeature::SystemMessages));
            assert!(registry.supports_feature(&model.id, &ModelFeature::StreamingSupport));
        }
    }

    #[test]
    fn test_models_with_feature() {
        let registry = get_ai21_registry();
        let tool_models = registry.get_models_with_feature(&ModelFeature::ToolCalling);
        assert!(!tool_models.is_empty());
    }

    #[test]
    fn test_default_impl() {
        let registry = AI21ModelRegistry::default();
        assert!(!registry.get_all_models().is_empty());
    }

    #[test]
    fn test_get_model_spec() {
        let registry = get_ai21_registry();
        let spec = registry.get_model_spec("jamba-1.5-large");
        assert!(spec.is_some());
        let spec = spec.unwrap();
        assert_eq!(spec.model_info.provider, "ai21");
    }

    #[test]
    fn test_get_model_spec_nonexistent() {
        let registry = get_ai21_registry();
        let spec = registry.get_model_spec("nonexistent-model");
        assert!(spec.is_none());
    }

    #[test]
    fn test_jamba_document_support() {
        let registry = get_ai21_registry();

        // Jamba models should have document support
        let document_models = registry.get_models_with_feature(&ModelFeature::DocumentSupport);
        for model in &document_models {
            assert!(model.contains("jamba"));
        }
    }

    #[test]
    fn test_model_config() {
        let registry = get_ai21_registry();

        if let Some(spec) = registry.get_model_spec("jamba-1.5-large") {
            assert_eq!(spec.config.max_concurrent_requests, Some(5));
        }

        if let Some(spec) = registry.get_model_spec("jamba-1.5-mini") {
            assert_eq!(spec.config.max_concurrent_requests, Some(10));
        }
    }

    #[test]
    fn test_model_feature_equality() {
        assert_eq!(ModelFeature::FunctionCalling, ModelFeature::FunctionCalling);
        assert_ne!(ModelFeature::FunctionCalling, ModelFeature::VisionSupport);
    }

    #[test]
    fn test_model_info_properties() {
        let registry = get_ai21_registry();
        let models = registry.get_all_models();

        for model in models {
            assert!(!model.id.is_empty());
            assert!(!model.name.is_empty());
            assert_eq!(model.provider, "ai21");
            assert!(model.max_context_length > 0);
            assert_eq!(model.currency, "USD");
            assert!(model.input_cost_per_1k_tokens.is_some());
            assert!(model.output_cost_per_1k_tokens.is_some());
        }
    }

    #[test]
    fn test_global_registry() {
        let registry1 = get_ai21_registry();
        let registry2 = get_ai21_registry();

        assert_eq!(
            registry1.get_all_models().len(),
            registry2.get_all_models().len()
        );
    }

    #[test]
    fn test_jamba_large_model() {
        let registry = get_ai21_registry();
        let spec = registry.get_model_spec("jamba-1.5-large").unwrap();

        assert_eq!(spec.model_info.max_context_length, 256_000);
        assert!(registry.supports_feature("jamba-1.5-large", &ModelFeature::ToolCalling));
        assert!(registry.supports_feature("jamba-1.5-large", &ModelFeature::FunctionCalling));
        assert!(registry.supports_feature("jamba-1.5-large", &ModelFeature::DocumentSupport));
    }

    #[test]
    fn test_supports_feature_nonexistent() {
        let registry = get_ai21_registry();
        assert!(!registry.supports_feature("nonexistent", &ModelFeature::FunctionCalling));
        assert!(!registry.supports_feature("nonexistent", &ModelFeature::ToolCalling));
    }
}
