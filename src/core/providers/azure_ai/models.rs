//! Azure AI Models Registry
//!
//! Azure AI Foundry supported model definitions and capability mappings

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::types::{model::ModelInfo, model::ProviderCapability};

/// Model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureAIModelSpec {
    /// Model
    pub id: String,
    /// Model
    pub name: String,
    /// Provider (such as OpenAI, Cohere, etc.)
    pub provider: String,
    /// Model
    pub model_type: AzureAIModelType,
    /// Supported capabilities
    pub capabilities: Vec<ProviderCapability>,
    /// Maximum input token count
    pub max_input_tokens: u32,
    /// Maximum output token count
    pub max_output_tokens: u32,
    /// Response
    pub supports_streaming: bool,
    /// Whether function calling is supported
    pub supports_function_calling: bool,
    /// Whether multimodal input is supported
    pub supports_multimodal: bool,
    /// Pricing information (per 1K tokens)
    pub input_price_per_1k: Option<f64>,
    pub output_price_per_1k: Option<f64>,
}

/// Model
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum AzureAIModelType {
    Chat,
    Completion,
    Embedding,
    ImageGeneration,
    Rerank,
    MultimodalEmbedding,
}

/// Model
#[derive(Debug)]
pub struct AzureAIModelRegistry {
    models: HashMap<String, AzureAIModelSpec>,
    type_mapping: HashMap<AzureAIModelType, Vec<String>>,
}

impl AzureAIModelRegistry {
    /// Create
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
            type_mapping: HashMap::new(),
        };

        registry.register_default_models();
        registry
    }

    /// Default
    fn register_default_models(&mut self) {
        // Reviewed against Microsoft's model and retirement tables, 2026-10-03.
        // Only exact current Foundry identities belong here; a provider's own
        // aliases are not Azure model IDs. See docs/audit/model-catalog-azure-voyage-2026-10-03.md.
        // Deployment/region pricing comes from the existing pricing authority,
        // not a second hardcoded per-token price in this capability registry.
        // Chat models
        // Microsoft lists Phi-4 as an Azure AI Foundry chat-completion model
        // with 16,384-token input/output limits and no tool calling:
        // https://learn.microsoft.com/azure/ai-foundry/model-inference/concepts/models
        self.register_model(AzureAIModelSpec {
            id: "Phi-4".to_string(),
            name: "Phi-4".to_string(),
            provider: "microsoft".to_string(),
            model_type: AzureAIModelType::Chat,
            capabilities: vec![ProviderCapability::ChatCompletion],
            max_input_tokens: 16_384,
            max_output_tokens: 16_384,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: None,
            output_price_per_1k: None,
        });

        self.register_model(AzureAIModelSpec {
            id: "gpt-4o".to_string(),
            name: "GPT-4 Omni".to_string(),
            provider: "openai".to_string(),
            model_type: AzureAIModelType::Chat,
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
            ],
            max_input_tokens: 128000,
            max_output_tokens: 4096,
            supports_streaming: true,
            supports_function_calling: true,
            supports_multimodal: true,
            input_price_per_1k: None,
            output_price_per_1k: None,
        });

        self.register_model(AzureAIModelSpec {
            id: "gpt-35-turbo".to_string(),
            name: "GPT-3.5 Turbo".to_string(),
            provider: "openai".to_string(),
            model_type: AzureAIModelType::Chat,
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
            ],
            max_input_tokens: 16_385,
            max_output_tokens: 4096,
            supports_streaming: true,
            supports_function_calling: true,
            supports_multimodal: false,
            input_price_per_1k: None,
            output_price_per_1k: None,
        });

        // Embedding models
        self.register_model(AzureAIModelSpec {
            id: "text-embedding-3-large".to_string(),
            name: "OpenAI Text Embedding 3 Large".to_string(),
            provider: "openai".to_string(),
            model_type: AzureAIModelType::Embedding,
            capabilities: vec![ProviderCapability::Embeddings],
            max_input_tokens: 8192,
            max_output_tokens: 0,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: None,
            output_price_per_1k: None,
        });

        self.register_model(AzureAIModelSpec {
            id: "text-embedding-3-small".to_string(),
            name: "OpenAI Text Embedding 3 Small".to_string(),
            provider: "openai".to_string(),
            model_type: AzureAIModelType::Embedding,
            capabilities: vec![ProviderCapability::Embeddings],
            max_input_tokens: 8192,
            max_output_tokens: 0,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: None,
            output_price_per_1k: None,
        });

        self.register_model(AzureAIModelSpec {
            id: "Cohere-embed-v3-multilingual".to_string(),
            name: "Cohere Embed V3 Multilingual".to_string(),
            provider: "cohere".to_string(),
            model_type: AzureAIModelType::Embedding,
            capabilities: vec![ProviderCapability::Embeddings],
            max_input_tokens: 512,
            max_output_tokens: 0,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: None,
            output_price_per_1k: None,
        });

        // Image generation models

        self.register_model(AzureAIModelSpec {
            id: "FLUX-1.1-pro".to_string(),
            name: "FLUX 1.1 Pro".to_string(),
            provider: "flux".to_string(),
            model_type: AzureAIModelType::ImageGeneration,
            capabilities: vec![ProviderCapability::ImageGeneration],
            max_input_tokens: 5_000,
            max_output_tokens: 0,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: None,
            output_price_per_1k: None,
        });

        self.register_model(AzureAIModelSpec {
            id: "FLUX.1-Kontext-pro".to_string(),
            name: "FLUX.1 Kontext Pro".to_string(),
            provider: "flux".to_string(),
            model_type: AzureAIModelType::ImageGeneration,
            capabilities: vec![ProviderCapability::ImageGeneration],
            max_input_tokens: 5_000,
            max_output_tokens: 0,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: None,
            output_price_per_1k: None,
        });

        // Exact Foundry IDs from Microsoft's current sold-directly model table.
        // Rerank pricing is per search, so no per-token price is advertised.
        for (id, name) in [
            ("Cohere-rerank-v4.0-pro", "Cohere Rerank V4 Pro"),
            ("Cohere-rerank-v4.0-fast", "Cohere Rerank V4 Fast"),
        ] {
            self.register_model(AzureAIModelSpec {
                id: id.to_string(),
                name: name.to_string(),
                provider: "cohere".to_string(),
                model_type: AzureAIModelType::Rerank,
                capabilities: vec![ProviderCapability::Rerank],
                max_input_tokens: 32_768,
                max_output_tokens: 0,
                supports_streaming: false,
                supports_function_calling: false,
                supports_multimodal: false,
                input_price_per_1k: None,
                output_price_per_1k: None,
            });
        }
    }

    /// Model
    pub fn register_model(&mut self, model: AzureAIModelSpec) {
        let model_id = model.id.clone();
        let model_type = model.model_type.clone();

        // Add to main mapping
        self.models.insert(model_id.clone(), model);

        // Add to type mapping
        self.type_mapping
            .entry(model_type)
            .or_default()
            .push(model_id);
    }

    /// Model
    pub fn get_model(&self, model_id: &str) -> Option<&AzureAIModelSpec> {
        self.models.get(model_id)
    }

    /// Model
    pub fn get_all_models(&self) -> Vec<&AzureAIModelSpec> {
        self.models.values().collect()
    }

    /// Model
    pub fn get_models_by_type(&self, model_type: &AzureAIModelType) -> Vec<&AzureAIModelSpec> {
        self.type_mapping
            .get(model_type)
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|id| self.models.get(id))
            .collect()
    }

    /// Model
    pub fn get_model_capabilities(&self, model_id: &str) -> Vec<ProviderCapability> {
        self.models
            .get(model_id)
            .map(|model| model.capabilities.clone())
            .unwrap_or_default()
    }

    /// Check
    /// Model
    pub fn supports_capability(&self, model_id: &str, capability: &ProviderCapability) -> bool {
        self.models
            .get(model_id)
            .is_some_and(|model| model.capabilities.contains(capability))
    }

    /// Convert to ModelInfo format
    pub fn to_model_infos(&self) -> Vec<ModelInfo> {
        self.models
            .values()
            .map(|spec| ModelInfo {
                id: spec.id.clone(),
                name: spec.name.clone(),
                provider: spec.provider.clone(),
                max_context_length: spec.max_input_tokens,
                max_output_length: Some(spec.max_output_tokens),
                supports_streaming: spec.supports_streaming,
                supports_tools: spec.supports_function_calling,
                supports_multimodal: spec.supports_multimodal,
                input_cost_per_1k_tokens: spec.input_price_per_1k,
                output_cost_per_1k_tokens: spec.output_price_per_1k,
                currency: "USD".to_string(),
                capabilities: spec.capabilities.clone(),
                created_at: Some(std::time::SystemTime::now()),
                updated_at: Some(std::time::SystemTime::now()),
                metadata: std::collections::HashMap::new(),
            })
            .collect()
    }
}

impl Default for AzureAIModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Model
use std::sync::OnceLock;
static AZURE_AI_REGISTRY: OnceLock<AzureAIModelRegistry> = OnceLock::new();

pub fn get_azure_ai_registry() -> &'static AzureAIModelRegistry {
    AZURE_AI_REGISTRY.get_or_init(AzureAIModelRegistry::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_omits_retired_and_unverified_supplier_aliases() {
        let registry = AzureAIModelRegistry::new();
        for model in [
            "gpt-4",
            "command-r",
            "command-r-plus",
            "mistral-large-latest",
            "ai21-jamba-instruct",
            "dall-e-3",
            "cohere-rerank-v3",
            "cohere-rerank-v3.5",
            "flux-1.1-pro",
            "flux.1-kontext-pro",
            "cohere-embed-v3-multilingual",
        ] {
            assert!(
                registry.get_model(model).is_none(),
                "unexpected catalog identity {model}"
            );
            assert!(!registry.supports_capability(model, &ProviderCapability::ChatCompletion));
        }
    }

    #[test]
    fn current_foundry_ids_preserve_modalities_without_image_token_prices() {
        let registry = AzureAIModelRegistry::new();
        let models = registry.to_model_infos();
        let cohere = models
            .iter()
            .find(|m| m.id == "Cohere-embed-v3-multilingual")
            .unwrap();
        assert_eq!(cohere.capabilities, vec![ProviderCapability::Embeddings]);
        assert!(!cohere.supports_multimodal);
        assert_eq!(cohere.max_context_length, 512);
        assert!(
            models
                .iter()
                .find(|m| m.id == "gpt-4o")
                .unwrap()
                .supports_multimodal
        );
        assert_eq!(
            registry.get_model("gpt-35-turbo").unwrap().max_input_tokens,
            16_385
        );
        for id in ["FLUX-1.1-pro", "FLUX.1-Kontext-pro"] {
            let model = registry.get_model(id).unwrap();
            assert_eq!(model.max_input_tokens, 5_000);
            assert_eq!(
                model.capabilities,
                vec![ProviderCapability::ImageGeneration]
            );
            assert_eq!(model.input_price_per_1k, None);
            assert_eq!(model.output_price_per_1k, None);
        }
    }

    #[test]
    fn test_model_registry_creation() {
        let registry = AzureAIModelRegistry::new();
        assert!(!registry.models.is_empty());
    }

    #[test]
    fn test_model_lookup() {
        let registry = AzureAIModelRegistry::new();
        let model = registry.get_model("gpt-4o");
        assert!(model.is_some());
        assert_eq!(model.unwrap().provider, "openai");
    }

    #[test]
    fn phi_4_has_exact_provider_native_chat_metadata() {
        let registry = AzureAIModelRegistry::new();
        let model = registry
            .get_model("Phi-4")
            .expect("official Azure AI Phi-4 identity must be registered exactly");

        assert_eq!(model.provider, "microsoft");
        assert_eq!(model.model_type, AzureAIModelType::Chat);
        assert_eq!(model.max_input_tokens, 16_384);
        assert_eq!(model.max_output_tokens, 16_384);
        assert!(!model.supports_streaming);
        assert!(!model.supports_function_calling);
        assert_eq!(model.capabilities, vec![ProviderCapability::ChatCompletion]);
        assert!(registry.get_model("phi-4").is_none());
        assert!(registry.get_model("Phi-4-extra").is_none());
    }

    #[test]
    fn test_model_capabilities() {
        let registry = AzureAIModelRegistry::new();
        assert!(registry.supports_capability("gpt-4o", &ProviderCapability::ChatCompletion));
        assert!(
            registry.supports_capability("text-embedding-3-large", &ProviderCapability::Embeddings)
        );
        assert!(!registry.supports_capability("dall-e-3", &ProviderCapability::ChatCompletion));
    }

    #[test]
    fn unknown_models_do_not_inherit_capabilities_from_their_names() {
        let registry = AzureAIModelRegistry::new();

        for (model, capability) in [
            (
                "customer-chat-deployment",
                ProviderCapability::ChatCompletion,
            ),
            (
                "customer-stream-deployment",
                ProviderCapability::ChatCompletionStream,
            ),
            ("customer-embed-deployment", ProviderCapability::Embeddings),
            (
                "customer-flux-deployment",
                ProviderCapability::ImageGeneration,
            ),
        ] {
            assert!(
                !registry.supports_capability(model, &capability),
                "unknown model '{model}' must not gain {capability:?} from its name"
            );
        }
    }

    #[test]
    fn test_models_by_type() {
        let registry = AzureAIModelRegistry::new();
        let chat_models = registry.get_models_by_type(&AzureAIModelType::Chat);
        assert!(!chat_models.is_empty());

        let embedding_models = registry.get_models_by_type(&AzureAIModelType::Embedding);
        assert!(!embedding_models.is_empty());
    }

    #[test]
    fn test_global_registry() {
        let registry = get_azure_ai_registry();
        // Test that registry is not empty
        assert!(!registry.get_all_models().is_empty());
        // Test that we can get known model info
        assert!(registry.get_model("gpt-4o").is_some());
    }
}
