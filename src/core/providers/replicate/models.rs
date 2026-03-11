//! Replicate Model Registry
//!
//! Model registry for Replicate with support for LLM and image generation models

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::core::types::{model::ModelInfo, model::ProviderCapability};

/// Model type for Replicate
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ReplicateModelType {
    /// Text generation / Chat completion (LLM)
    TextGeneration,
    /// Image generation (Stable Diffusion, SDXL, Flux, etc.)
    ImageGeneration,
    /// Audio generation
    AudioGeneration,
    /// Video generation
    VideoGeneration,
    /// Other model types
    Other,
}

/// Model specification for Replicate
#[derive(Debug, Clone)]
pub struct ReplicateModelSpec {
    /// Model info
    pub model_info: ModelInfo,
    /// Model type
    pub model_type: ReplicateModelType,
    /// Default input parameters
    pub default_params: HashMap<String, serde_json::Value>,
}

/// Replicate model registry
pub struct ReplicateModelRegistry {
    models: HashMap<String, ReplicateModelSpec>,
}

impl Default for ReplicateModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplicateModelRegistry {
    /// Create a new model registry
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
        };
        registry.load_models();
        registry
    }

    /// Load default models
    fn load_models(&mut self) {
        // LLM Models
        self.add_llm_models();

        // Image Generation Models
        self.add_image_models();
    }

    /// Add LLM models to the registry
    fn add_llm_models(&mut self) {
        let llm_models = vec![
            // Meta Llama 2 models
            (
                "meta/llama-2-70b-chat",
                "Llama 2 70B Chat",
                4096,
                Some(2048),
                0.00065, // ~$0.65/1M tokens
                0.00275, // ~$2.75/1M tokens
            ),
            (
                "meta/llama-2-13b-chat",
                "Llama 2 13B Chat",
                4096,
                Some(2048),
                0.0001,
                0.0005,
            ),
            (
                "meta/llama-2-7b-chat",
                "Llama 2 7B Chat",
                4096,
                Some(2048),
                0.00005,
                0.00025,
            ),
            // Meta Llama 3 models
            (
                "meta/meta-llama-3-70b-instruct",
                "Llama 3 70B Instruct",
                8192,
                Some(4096),
                0.00065,
                0.00275,
            ),
            (
                "meta/meta-llama-3-8b-instruct",
                "Llama 3 8B Instruct",
                8192,
                Some(4096),
                0.00005,
                0.00025,
            ),
            // Meta Llama 3.1 models
            (
                "meta/meta-llama-3.1-405b-instruct",
                "Llama 3.1 405B Instruct",
                128_000,
                Some(4096),
                0.0095, // Higher cost for largest model
                0.0095,
            ),
            // Mistral models
            (
                "mistralai/mistral-7b-instruct-v0.2",
                "Mistral 7B Instruct v0.2",
                32_768,
                Some(4096),
                0.00005,
                0.00025,
            ),
            (
                "mistralai/mixtral-8x7b-instruct-v0.1",
                "Mixtral 8x7B Instruct",
                32_768,
                Some(4096),
                0.00027,
                0.00027,
            ),
        ];

        for (id, name, context_len, output_len, input_cost, output_cost) in llm_models {
            let model_info = ModelInfo {
                id: id.to_string(),
                name: name.to_string(),
                provider: "replicate".to_string(),
                max_context_length: context_len,
                max_output_length: output_len,
                supports_streaming: true,
                supports_tools: false, // Replicate LLMs generally don't support tool calling
                supports_multimodal: false,
                input_cost_per_1k_tokens: Some(input_cost),
                output_cost_per_1k_tokens: Some(output_cost),
                currency: "USD".to_string(),
                capabilities: vec![ProviderCapability::ChatCompletion],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            };

            self.models.insert(
                id.to_string(),
                ReplicateModelSpec {
                    model_info,
                    model_type: ReplicateModelType::TextGeneration,
                    default_params: HashMap::new(),
                },
            );
        }
    }

    /// Add image generation models to the registry
    fn add_image_models(&mut self) {
        let image_models = vec![
            // Stable Diffusion XL
            (
                "stability-ai/sdxl",
                "Stable Diffusion XL",
                "1024x1024",
                0.003, // ~$0.003 per image
            ),
            (
                "stability-ai/stable-diffusion",
                "Stable Diffusion 2.1",
                "768x768",
                0.002,
            ),
            // FLUX models
            (
                "black-forest-labs/flux-schnell",
                "FLUX Schnell",
                "1024x1024",
                0.003,
            ),
            ("black-forest-labs/flux-dev", "FLUX Dev", "1024x1024", 0.025),
            ("black-forest-labs/flux-pro", "FLUX Pro", "1024x1024", 0.05),
            // Other popular models
            (
                "bytedance/sdxl-lightning-4step",
                "SDXL Lightning 4-Step",
                "1024x1024",
                0.002,
            ),
            (
                "lucataco/playground-v2.5-1024px-aesthetic",
                "Playground v2.5",
                "1024x1024",
                0.004,
            ),
        ];

        for (id, name, default_size, cost_per_image) in image_models {
            let mut metadata = HashMap::new();
            metadata.insert(
                "default_size".to_string(),
                serde_json::Value::String(default_size.to_string()),
            );
            metadata.insert(
                "cost_per_image".to_string(),
                serde_json::json!(cost_per_image),
            );

            let model_info = ModelInfo {
                id: id.to_string(),
                name: name.to_string(),
                provider: "replicate".to_string(),
                max_context_length: 0, // Not applicable for image models
                max_output_length: None,
                supports_streaming: false,
                supports_tools: false,
                supports_multimodal: false,
                input_cost_per_1k_tokens: None, // Image models don't use token pricing
                output_cost_per_1k_tokens: None,
                currency: "USD".to_string(),
                capabilities: vec![ProviderCapability::ImageGeneration],
                created_at: None,
                updated_at: None,
                metadata,
            };

            let mut default_params = HashMap::new();
            // Parse default size into width and height
            if let Some((w, h)) = default_size.split_once('x')
                && let (Ok(width), Ok(height)) = (w.parse::<i64>(), h.parse::<i64>())
            {
                default_params.insert("width".to_string(), serde_json::json!(width));
                default_params.insert("height".to_string(), serde_json::json!(height));
            }

            self.models.insert(
                id.to_string(),
                ReplicateModelSpec {
                    model_info,
                    model_type: ReplicateModelType::ImageGeneration,
                    default_params,
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

    /// Get models by type
    pub fn get_models_by_type(&self, model_type: &ReplicateModelType) -> Vec<ModelInfo> {
        self.models
            .values()
            .filter(|spec| &spec.model_type == model_type)
            .map(|spec| spec.model_info.clone())
            .collect()
    }

    /// Get model specification
    pub fn get_model_spec(&self, model_id: &str) -> Option<&ReplicateModelSpec> {
        self.models.get(model_id)
    }

    /// Check if model exists
    pub fn has_model(&self, model_id: &str) -> bool {
        self.models.contains_key(model_id)
    }

    /// Get model type
    pub fn get_model_type(&self, model_id: &str) -> Option<ReplicateModelType> {
        self.models
            .get(model_id)
            .map(|spec| spec.model_type.clone())
    }

    /// Get default parameters for a model
    pub fn get_default_params(
        &self,
        model_id: &str,
    ) -> Option<&HashMap<String, serde_json::Value>> {
        self.models.get(model_id).map(|spec| &spec.default_params)
    }

    /// Get LLM models (for chat completion)
    pub fn get_llm_models(&self) -> Vec<ModelInfo> {
        self.get_models_by_type(&ReplicateModelType::TextGeneration)
    }

    /// Get image generation models
    pub fn get_image_models(&self) -> Vec<ModelInfo> {
        self.get_models_by_type(&ReplicateModelType::ImageGeneration)
    }
}

/// Global model registry
static REPLICATE_REGISTRY: OnceLock<ReplicateModelRegistry> = OnceLock::new();

/// Get the global Replicate model registry
pub fn get_replicate_registry() -> &'static ReplicateModelRegistry {
    REPLICATE_REGISTRY.get_or_init(ReplicateModelRegistry::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = ReplicateModelRegistry::new();
        assert!(!registry.get_all_models().is_empty());
    }

    #[test]
    fn test_registry_has_llm_models() {
        let registry = get_replicate_registry();
        let llm_models = registry.get_llm_models();
        assert!(!llm_models.is_empty());

        // Check for specific Llama model
        let has_llama = llm_models.iter().any(|m| m.id.contains("llama"));
        assert!(has_llama);
    }

    #[test]
    fn test_registry_has_image_models() {
        let registry = get_replicate_registry();
        let image_models = registry.get_image_models();
        assert!(!image_models.is_empty());

        // Check for SDXL
        let has_sdxl = image_models.iter().any(|m| m.id.contains("sdxl"));
        assert!(has_sdxl);
    }

    #[test]
    fn test_get_model_spec() {
        let registry = get_replicate_registry();
        let spec = registry.get_model_spec("meta/llama-2-70b-chat");
        assert!(spec.is_some());

        let spec = spec.unwrap();
        assert_eq!(spec.model_type, ReplicateModelType::TextGeneration);
    }

    #[test]
    fn test_get_model_type() {
        let registry = get_replicate_registry();

        assert_eq!(
            registry.get_model_type("meta/llama-2-70b-chat"),
            Some(ReplicateModelType::TextGeneration)
        );
        assert_eq!(
            registry.get_model_type("stability-ai/sdxl"),
            Some(ReplicateModelType::ImageGeneration)
        );
        assert_eq!(registry.get_model_type("nonexistent"), None);
    }

    #[test]
    fn test_has_model() {
        let registry = get_replicate_registry();
        assert!(registry.has_model("meta/llama-2-70b-chat"));
        assert!(registry.has_model("stability-ai/sdxl"));
        assert!(!registry.has_model("nonexistent"));
    }

    #[test]
    fn test_model_info_properties() {
        let registry = get_replicate_registry();
        let models = registry.get_all_models();

        for model in models {
            assert!(!model.id.is_empty());
            assert!(!model.name.is_empty());
            assert_eq!(model.provider, "replicate");
            assert_eq!(model.currency, "USD");
        }
    }

    #[test]
    fn test_llm_model_capabilities() {
        let registry = get_replicate_registry();
        let llm_models = registry.get_llm_models();

        for model in llm_models {
            assert!(
                model
                    .capabilities
                    .contains(&ProviderCapability::ChatCompletion)
            );
            assert!(model.max_context_length > 0);
        }
    }

    #[test]
    fn test_image_model_capabilities() {
        let registry = get_replicate_registry();
        let image_models = registry.get_image_models();

        for model in image_models {
            assert!(
                model
                    .capabilities
                    .contains(&ProviderCapability::ImageGeneration)
            );
        }
    }

    #[test]
    fn test_image_model_default_params() {
        let registry = get_replicate_registry();
        let params = registry.get_default_params("stability-ai/sdxl");
        assert!(params.is_some());

        let params = params.unwrap();
        assert!(params.contains_key("width"));
        assert!(params.contains_key("height"));
    }

    #[test]
    fn test_model_type_equality() {
        assert_eq!(
            ReplicateModelType::TextGeneration,
            ReplicateModelType::TextGeneration
        );
        assert_ne!(
            ReplicateModelType::TextGeneration,
            ReplicateModelType::ImageGeneration
        );
    }

    #[test]
    fn test_default_impl() {
        let registry = ReplicateModelRegistry::default();
        assert!(!registry.get_all_models().is_empty());
    }

    #[test]
    fn test_global_registry() {
        let registry1 = get_replicate_registry();
        let registry2 = get_replicate_registry();

        // Should be the same instance
        assert_eq!(
            registry1.get_all_models().len(),
            registry2.get_all_models().len()
        );
    }

    #[test]
    fn test_flux_models() {
        let registry = get_replicate_registry();

        assert!(registry.has_model("black-forest-labs/flux-schnell"));
        assert!(registry.has_model("black-forest-labs/flux-dev"));
        assert!(registry.has_model("black-forest-labs/flux-pro"));
    }

    #[test]
    fn test_llama_3_models() {
        let registry = get_replicate_registry();

        assert!(registry.has_model("meta/meta-llama-3-70b-instruct"));
        assert!(registry.has_model("meta/meta-llama-3-8b-instruct"));
    }
}
