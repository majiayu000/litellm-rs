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
        // Chat models
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
            input_price_per_1k: Some(0.005),
            output_price_per_1k: Some(0.015),
        });

        self.register_model(AzureAIModelSpec {
            id: "gpt-4".to_string(),
            name: "GPT-4".to_string(),
            provider: "openai".to_string(),
            model_type: AzureAIModelType::Chat,
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
            ],
            max_input_tokens: 8192,
            max_output_tokens: 4096,
            supports_streaming: true,
            supports_function_calling: true,
            supports_multimodal: false,
            input_price_per_1k: Some(0.03),
            output_price_per_1k: Some(0.06),
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
            max_input_tokens: 4096,
            max_output_tokens: 4096,
            supports_streaming: true,
            supports_function_calling: true,
            supports_multimodal: false,
            input_price_per_1k: Some(0.0005),
            output_price_per_1k: Some(0.0015),
        });

        // Cohere models
        self.register_model(AzureAIModelSpec {
            id: "command-r-plus".to_string(),
            name: "Cohere Command R Plus".to_string(),
            provider: "cohere".to_string(),
            model_type: AzureAIModelType::Chat,
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
            ],
            max_input_tokens: 128000,
            max_output_tokens: 4096,
            supports_streaming: true,
            supports_function_calling: true,
            supports_multimodal: false,
            input_price_per_1k: Some(0.003),
            output_price_per_1k: Some(0.015),
        });

        self.register_model(AzureAIModelSpec {
            id: "command-r".to_string(),
            name: "Cohere Command R".to_string(),
            provider: "cohere".to_string(),
            model_type: AzureAIModelType::Chat,
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
            ],
            max_input_tokens: 128000,
            max_output_tokens: 4096,
            supports_streaming: true,
            supports_function_calling: true,
            supports_multimodal: false,
            input_price_per_1k: Some(0.0015),
            output_price_per_1k: Some(0.015),
        });

        // Mistral models
        self.register_model(AzureAIModelSpec {
            id: "mistral-large-latest".to_string(),
            name: "Mistral Large".to_string(),
            provider: "mistral".to_string(),
            model_type: AzureAIModelType::Chat,
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
            ],
            max_input_tokens: 32000,
            max_output_tokens: 4096,
            supports_streaming: true,
            supports_function_calling: true,
            supports_multimodal: false,
            input_price_per_1k: Some(0.004),
            output_price_per_1k: Some(0.012),
        });

        // AI21 Jamba models
        self.register_model(AzureAIModelSpec {
            id: "ai21-jamba-instruct".to_string(),
            name: "AI21 Jamba Instruct".to_string(),
            provider: "ai21".to_string(),
            model_type: AzureAIModelType::Chat,
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
            ],
            max_input_tokens: 70000,
            max_output_tokens: 4096,
            supports_streaming: true,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: Some(0.0005),
            output_price_per_1k: Some(0.0007),
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
            input_price_per_1k: Some(0.00013),
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
            input_price_per_1k: Some(0.00002),
            output_price_per_1k: None,
        });

        self.register_model(AzureAIModelSpec {
            id: "cohere-embed-v3-multilingual".to_string(),
            name: "Cohere Embed V3 Multilingual".to_string(),
            provider: "cohere".to_string(),
            model_type: AzureAIModelType::MultimodalEmbedding,
            capabilities: vec![ProviderCapability::Embeddings],
            max_input_tokens: 512,
            max_output_tokens: 0,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: true,
            input_price_per_1k: Some(0.0001),
            output_price_per_1k: None,
        });

        // Image generation models
        self.register_model(AzureAIModelSpec {
            id: "dall-e-3".to_string(),
            name: "DALL-E 3".to_string(),
            provider: "openai".to_string(),
            model_type: AzureAIModelType::ImageGeneration,
            capabilities: vec![ProviderCapability::ImageGeneration],
            max_input_tokens: 4000,
            max_output_tokens: 0,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: Some(0.04), // Price per image
            output_price_per_1k: None,
        });

        self.register_model(AzureAIModelSpec {
            id: "flux-1.1-pro".to_string(),
            name: "FLUX 1.1 Pro".to_string(),
            provider: "flux".to_string(),
            model_type: AzureAIModelType::ImageGeneration,
            capabilities: vec![ProviderCapability::ImageGeneration],
            max_input_tokens: 4000,
            max_output_tokens: 0,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: Some(0.04), // Price per image
            output_price_per_1k: None,
        });

        self.register_model(AzureAIModelSpec {
            id: "flux.1-kontext-pro".to_string(),
            name: "FLUX.1 Kontext Pro".to_string(),
            provider: "flux".to_string(),
            model_type: AzureAIModelType::ImageGeneration,
            capabilities: vec![ProviderCapability::ImageGeneration],
            max_input_tokens: 4000,
            max_output_tokens: 0,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: Some(0.055), // Higher pricing for Kontext
            output_price_per_1k: None,
        });

        // Rerank models
        self.register_model(AzureAIModelSpec {
            id: "cohere-rerank-v3".to_string(),
            name: "Cohere Rerank V3".to_string(),
            provider: "cohere".to_string(),
            model_type: AzureAIModelType::Rerank,
            capabilities: vec![],
            max_input_tokens: 4096,
            max_output_tokens: 0,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: Some(0.002),
            output_price_per_1k: None,
        });

        self.register_model(AzureAIModelSpec {
            id: "cohere-rerank-v3.5".to_string(),
            name: "Cohere Rerank V3.5".to_string(),
            provider: "cohere".to_string(),
            model_type: AzureAIModelType::Rerank,
            capabilities: vec![],
            max_input_tokens: 4096,
            max_output_tokens: 0,
            supports_streaming: false,
            supports_function_calling: false,
            supports_multimodal: false,
            input_price_per_1k: Some(0.002),
            output_price_per_1k: None,
        });
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
        if let Some(model) = self.models.get(model_id) {
            // Known model: use precise capability mapping
            model.capabilities.contains(capability)
        } else {
            // Unknown model: use heuristic validation
            match capability {
                ProviderCapability::ChatCompletion => true,
                ProviderCapability::ChatCompletionStream => true,
                ProviderCapability::Embeddings => model_id.contains("embed"),
                ProviderCapability::ImageGeneration => {
                    model_id.contains("dall-e") || model_id.contains("flux")
                }
                _ => false, // Other capabilities remain conservative
            }
        }
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
                supports_multimodal: matches!(
                    spec.model_type,
                    AzureAIModelType::MultimodalEmbedding
                ),
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
    fn test_model_capabilities() {
        let registry = AzureAIModelRegistry::new();
        assert!(registry.supports_capability("gpt-4o", &ProviderCapability::ChatCompletion));
        assert!(
            registry.supports_capability("text-embedding-3-large", &ProviderCapability::Embeddings)
        );
        assert!(!registry.supports_capability("dall-e-3", &ProviderCapability::ChatCompletion));
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
