//! OpenAI Model Registry
//!
//! Dynamic model discovery and capability detection system.
//! Types are defined in `registry_types`, static model data in `static_models`.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::core::providers::base::get_pricing_db;
use crate::core::types::model::ModelInfo;

use super::registry_types::{
    OpenAIModelConfig, OpenAIModelFamily, OpenAIModelFeature, OpenAIModelSpec, OpenAIUseCase,
};
use super::static_models::static_model_entries;

/// OpenAI model registry
#[derive(Debug)]
pub struct OpenAIModelRegistry {
    models: HashMap<String, OpenAIModelSpec>,
}

impl Default for OpenAIModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAIModelRegistry {
    /// Create new registry instance
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
        };
        registry.load_models();
        registry
    }

    /// Load models from pricing database and add static definitions
    fn load_models(&mut self) {
        // Always load built-in static models first so we keep a comprehensive
        // fallback catalog even when pricing DB is partially populated.
        self.add_static_models();

        let pricing_db = get_pricing_db();
        let model_ids = pricing_db.get_provider_models("openai");

        // Load from pricing database
        for model_id in &model_ids {
            if let Some(mut model_info) = pricing_db.to_model_info(model_id, "openai") {
                let features = self.detect_features(&model_info);

                // Convert features to capabilities
                model_info.capabilities = features
                    .iter()
                    .filter_map(|f| f.to_provider_capability())
                    .collect();

                let family = self.determine_family(&model_info);
                let config = self.create_config(&model_info);

                self.models.insert(
                    model_id.clone(),
                    OpenAIModelSpec {
                        model_info,
                        features,
                        family,
                        config,
                    },
                );
            }
        }
    }

    /// Detect model features based on model info
    fn detect_features(&self, model_info: &ModelInfo) -> Vec<OpenAIModelFeature> {
        let mut features = vec![OpenAIModelFeature::SystemMessages];

        let model_id = &model_info.id;

        // Keep streaming feature aligned with create_config().
        if !model_id.contains("embedding") && !model_id.starts_with("whisper") {
            features.push(OpenAIModelFeature::StreamingSupport);
        }

        if model_id.starts_with("gpt-") {
            features.push(OpenAIModelFeature::ChatCompletion);
            features.push(OpenAIModelFeature::JsonMode);
        }

        if model_info.supports_tools {
            features.push(OpenAIModelFeature::FunctionCalling);
        }

        if model_info.supports_multimodal || model_id.contains("vision") {
            features.push(OpenAIModelFeature::VisionSupport);
        }

        if model_id.starts_with("o1") || model_id.starts_with("o3") || model_id.starts_with("o4") {
            features.push(OpenAIModelFeature::ReasoningMode);
        }

        if model_id.contains("gpt-4o-audio") {
            features.push(OpenAIModelFeature::AudioInput);
            features.push(OpenAIModelFeature::AudioOutput);
        }

        if model_id.starts_with("dall-e")
            || model_id.starts_with("gpt-image-")
            || model_id.starts_with("chatgpt-image-")
        {
            features.push(OpenAIModelFeature::ImageGeneration);
            if model_id.contains("dall-e-3") {
                features.push(OpenAIModelFeature::ImageEditing);
            }
        }

        if model_id.starts_with("whisper") {
            features.push(OpenAIModelFeature::AudioTranscription);
        }

        if model_id.starts_with("tts") {
            features.push(OpenAIModelFeature::AudioOutput);
        }

        if model_id.contains("embedding") {
            features.push(OpenAIModelFeature::Embeddings);
        }

        if model_id.contains("code") || model_id.contains("codex") {
            features.push(OpenAIModelFeature::CodeCompletion);
        }

        if model_info.max_context_length > 32000 {
            features.push(OpenAIModelFeature::LargeContext);
        }

        if matches!(
            model_id.as_str(),
            "gpt-3.5-turbo" | "gpt-4" | "gpt-4-turbo" | "babbage-002" | "davinci-002"
        ) {
            features.push(OpenAIModelFeature::FineTuning);
        }

        features
    }

    /// Determine model family
    fn determine_family(&self, model_info: &ModelInfo) -> OpenAIModelFamily {
        let model_id = &model_info.id;

        if model_id.starts_with("gpt-4o-mini") {
            OpenAIModelFamily::GPT4OMini
        } else if model_id.starts_with("gpt-4.1-nano") {
            OpenAIModelFamily::GPT41Nano
        } else if model_id.starts_with("gpt-4.1-mini") {
            OpenAIModelFamily::GPT41Mini
        } else if model_id.starts_with("gpt-4.1") {
            OpenAIModelFamily::GPT41
        } else if model_id.starts_with("gpt-4o-audio") || model_id.contains("audio-preview") {
            OpenAIModelFamily::GPT4OAudio
        } else if model_id.starts_with("gpt-4o-realtime") {
            OpenAIModelFamily::Realtime
        } else if model_id.starts_with("gpt-4o") {
            OpenAIModelFamily::GPT4O
        } else if model_id.starts_with("gpt-4-turbo")
            || model_id.starts_with("gpt-4-1106")
            || model_id.starts_with("gpt-4-0125")
        {
            OpenAIModelFamily::GPT4Turbo
        } else if model_id.starts_with("gpt-4") {
            OpenAIModelFamily::GPT4
        } else if model_id.starts_with("gpt-3.5") {
            OpenAIModelFamily::GPT35
        } else if model_id.starts_with("gpt-5.4-mini") {
            OpenAIModelFamily::GPT54Mini
        } else if model_id.starts_with("gpt-5.4-turbo") {
            OpenAIModelFamily::GPT54Turbo
        } else if model_id.starts_with("gpt-5.4") {
            OpenAIModelFamily::GPT54
        } else if model_id.starts_with("gpt-5.2-pro") {
            OpenAIModelFamily::GPT52Pro
        } else if model_id.starts_with("gpt-5.2-codex") || model_id.starts_with("gpt-5-codex") {
            OpenAIModelFamily::GPT52Codex
        } else if model_id.starts_with("gpt-5.2") || model_id.contains("gpt-5.2") {
            OpenAIModelFamily::GPT52
        } else if model_id.starts_with("gpt-5.1-thinking") || model_id.contains("5.1-thinking") {
            OpenAIModelFamily::GPT51Thinking
        } else if model_id.starts_with("gpt-5.1") || model_id.contains("gpt-5.1") {
            OpenAIModelFamily::GPT51
        } else if model_id.starts_with("gpt-5-nano") {
            OpenAIModelFamily::GPT5Nano
        } else if model_id.starts_with("gpt-5-mini") {
            OpenAIModelFamily::GPT5Mini
        } else if model_id.starts_with("gpt-5") {
            OpenAIModelFamily::GPT5
        } else if model_id.starts_with("gpt-audio") {
            OpenAIModelFamily::GPTAudio
        } else if model_id.starts_with("o4-mini") {
            OpenAIModelFamily::O4Mini
        } else if model_id.starts_with("o3-pro") {
            OpenAIModelFamily::O3Pro
        } else if model_id.starts_with("o3-mini") {
            OpenAIModelFamily::O3Mini
        } else if model_id.starts_with("o3") {
            OpenAIModelFamily::O3
        } else if model_id.starts_with("o1-pro") {
            OpenAIModelFamily::O1Pro
        } else if model_id.starts_with("o1") {
            OpenAIModelFamily::O1
        } else if model_id.starts_with("gpt-image-") || model_id.starts_with("chatgpt-image-") {
            OpenAIModelFamily::GPTImage
        } else if model_id.starts_with("dall-e-2") {
            OpenAIModelFamily::DALLE2
        } else if model_id.starts_with("dall-e-3") {
            OpenAIModelFamily::DALLE3
        } else if model_id.starts_with("whisper") {
            OpenAIModelFamily::Whisper
        } else if model_id.starts_with("tts") {
            OpenAIModelFamily::TTS
        } else if model_id.contains("embedding") {
            OpenAIModelFamily::Embedding
        } else {
            OpenAIModelFamily::GPT4 // Default fallback
        }
    }

    /// Create model configuration
    fn create_config(&self, model_info: &ModelInfo) -> OpenAIModelConfig {
        let mut config = OpenAIModelConfig::default();
        let model_id = &model_info.id;

        match model_id.as_str() {
            m if m.starts_with("gpt-5") => {
                config.max_rpm = Some(6000);
                config.max_tpm = Some(400000);
            }
            m if m.starts_with("gpt-4") => {
                config.max_rpm = Some(10000);
                config.max_tpm = Some(300000);
            }
            m if m.starts_with("gpt-3.5") => {
                config.max_rpm = Some(10000);
                config.max_tpm = Some(1000000);
            }
            m if m.starts_with("o1") || m.starts_with("o3") || m.starts_with("o4") => {
                config.max_rpm = Some(5000);
                config.max_tpm = Some(100000);
                config.default_temperature = Some(1.0);
            }
            _ => {
                config.max_rpm = Some(5000);
                config.max_tpm = Some(200000);
            }
        }

        config.supports_batch = matches!(
            model_id.as_str(),
            "gpt-4"
                | "gpt-4-turbo"
                | "gpt-3.5-turbo"
                | "text-embedding-ada-002"
                | "text-embedding-3-small"
                | "text-embedding-3-large"
        );

        config.supports_streaming =
            !model_id.contains("embedding") && !model_id.contains("whisper");

        config
    }

    /// Add static model definitions as fallback (data from `static_models` module)
    fn add_static_models(&mut self) {
        for (id, name, family, max_context, max_output, input_cost, output_cost) in
            static_model_entries()
        {
            let mut model_info = ModelInfo {
                id: id.to_string(),
                name: name.to_string(),
                provider: "openai".to_string(),
                max_context_length: max_context,
                max_output_length: max_output,
                supports_streaming: family != OpenAIModelFamily::Embedding
                    && family != OpenAIModelFamily::Whisper,
                supports_tools: matches!(
                    family,
                    OpenAIModelFamily::GPT4
                        | OpenAIModelFamily::GPT4Turbo
                        | OpenAIModelFamily::GPT4O
                        | OpenAIModelFamily::GPT4OMini
                        | OpenAIModelFamily::GPT35
                        | OpenAIModelFamily::GPT5
                        | OpenAIModelFamily::GPT5Mini
                        | OpenAIModelFamily::GPT5Nano
                        | OpenAIModelFamily::GPT51
                        | OpenAIModelFamily::GPT51Thinking
                        | OpenAIModelFamily::GPT52
                        | OpenAIModelFamily::GPT52Pro
                        | OpenAIModelFamily::GPT52Codex
                        | OpenAIModelFamily::O1
                        | OpenAIModelFamily::O1Pro
                        | OpenAIModelFamily::O3
                        | OpenAIModelFamily::O3Mini
                        | OpenAIModelFamily::O4Mini
                        | OpenAIModelFamily::GPT4OAudio
                        | OpenAIModelFamily::GPTAudio
                        | OpenAIModelFamily::GPT54
                        | OpenAIModelFamily::GPT54Mini
                        | OpenAIModelFamily::GPT54Turbo
                ),
                supports_multimodal: matches!(
                    family,
                    OpenAIModelFamily::GPT4O
                        | OpenAIModelFamily::GPT4OMini
                        | OpenAIModelFamily::GPT4OAudio
                        | OpenAIModelFamily::GPT5
                        | OpenAIModelFamily::GPT5Mini
                        | OpenAIModelFamily::GPT51
                        | OpenAIModelFamily::GPT51Thinking
                        | OpenAIModelFamily::GPT52
                        | OpenAIModelFamily::GPT52Pro
                        | OpenAIModelFamily::GPT52Codex
                        | OpenAIModelFamily::GPTAudio
                        | OpenAIModelFamily::O1
                        | OpenAIModelFamily::O1Pro
                        | OpenAIModelFamily::O3
                        | OpenAIModelFamily::O3Mini
                        | OpenAIModelFamily::O4Mini
                        | OpenAIModelFamily::GPT54
                        | OpenAIModelFamily::GPT54Mini
                        | OpenAIModelFamily::GPT54Turbo
                ) || id.contains("vision"),
                input_cost_per_1k_tokens: Some(input_cost),
                output_cost_per_1k_tokens: Some(output_cost),
                currency: "USD".to_string(),
                capabilities: vec![],
                created_at: None,
                updated_at: None,
                metadata: HashMap::new(),
            };

            // Mark known deprecated/removed models
            if matches!(
                id,
                "gpt-3.5-turbo"
                    | "gpt-3.5-turbo-0125"
                    | "o1-preview"
                    | "o1-mini"
                    | "o1-mini-2024-09-12"
                    | "codex-mini-latest"
            ) {
                model_info
                    .metadata
                    .insert("deprecated".to_string(), serde_json::json!(true));
            }

            let features = self.detect_features(&model_info);
            model_info.capabilities = features
                .iter()
                .filter_map(|f| f.to_provider_capability())
                .collect();
            let config = self.create_config(&model_info);

            self.models.insert(
                id.to_string(),
                OpenAIModelSpec {
                    model_info,
                    features,
                    family,
                    config,
                },
            );
        }
    }

    /// Get all model information
    pub fn get_all_models(&self) -> Vec<ModelInfo> {
        self.models
            .values()
            .map(|spec| spec.model_info.clone())
            .collect()
    }

    /// Get specific model specification
    pub fn get_model_spec(&self, model_id: &str) -> Option<&OpenAIModelSpec> {
        self.models.get(model_id)
    }

    /// Check if model supports a feature
    pub fn supports_feature(&self, model_id: &str, feature: &OpenAIModelFeature) -> bool {
        self.models
            .get(model_id)
            .map(|spec| spec.features.contains(feature))
            .unwrap_or(false)
    }

    /// Get models by family
    pub fn get_models_by_family(&self, family: &OpenAIModelFamily) -> Vec<String> {
        self.models
            .iter()
            .filter_map(|(id, spec)| {
                if &spec.family == family {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get models supporting specific feature
    pub fn get_models_with_feature(&self, feature: &OpenAIModelFeature) -> Vec<String> {
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

    /// Get the best model for a specific use case
    pub fn get_recommended_model(&self, use_case: OpenAIUseCase) -> Option<String> {
        match use_case {
            OpenAIUseCase::GeneralChat => Some("gpt-5.2-chat".to_string()),
            OpenAIUseCase::CodeGeneration => Some("gpt-5.2-codex".to_string()),
            OpenAIUseCase::Reasoning => Some("o3-pro".to_string()),
            OpenAIUseCase::Vision => Some("gpt-5.2".to_string()),
            OpenAIUseCase::ImageGeneration => Some("gpt-image-1.5".to_string()),
            OpenAIUseCase::AudioTranscription => Some("whisper-1".to_string()),
            OpenAIUseCase::TextToSpeech => Some("tts-1-hd".to_string()),
            OpenAIUseCase::Embeddings => Some("text-embedding-3-large".to_string()),
            OpenAIUseCase::CostOptimized => Some("gpt-5-nano".to_string()),
        }
    }
}

/// Global model registry instance
static OPENAI_REGISTRY: OnceLock<OpenAIModelRegistry> = OnceLock::new();

/// Get global OpenAI model registry
pub fn get_openai_registry() -> &'static OpenAIModelRegistry {
    OPENAI_REGISTRY.get_or_init(OpenAIModelRegistry::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_registry_creation() {
        let registry = OpenAIModelRegistry::new();
        let models = registry.get_all_models();
        assert!(!models.is_empty());
    }

    #[test]
    fn test_feature_detection() {
        let registry = get_openai_registry();

        assert!(registry.supports_feature("gpt-4", &OpenAIModelFeature::ChatCompletion));
        assert!(registry.supports_feature("gpt-4", &OpenAIModelFeature::FunctionCalling));
        assert!(registry.supports_feature("gpt-4", &OpenAIModelFeature::StreamingSupport));

        let has_o1_reasoning =
            registry.supports_feature("o1-preview", &OpenAIModelFeature::ReasoningMode);
        if !has_o1_reasoning {
            eprintln!("Warning: o1-preview model not found or doesn't support ReasoningMode");
        }

        let has_dalle_generation =
            registry.supports_feature("dall-e-3", &OpenAIModelFeature::ImageGeneration);
        if !has_dalle_generation {
            eprintln!("Warning: dall-e-3 model not found or doesn't support ImageGeneration");
        }
    }

    #[test]
    fn test_model_families() {
        let registry = get_openai_registry();
        let gpt4_models = registry.get_models_by_family(&OpenAIModelFamily::GPT4);
        assert!(!gpt4_models.is_empty());
    }

    #[test]
    fn test_model_recommendations() {
        let registry = get_openai_registry();

        assert_eq!(
            registry.get_recommended_model(OpenAIUseCase::GeneralChat),
            Some("gpt-5.2-chat".to_string())
        );
        assert_eq!(
            registry.get_recommended_model(OpenAIUseCase::Reasoning),
            Some("o3-pro".to_string())
        );
        assert_eq!(
            registry.get_recommended_model(OpenAIUseCase::CostOptimized),
            Some("gpt-5-nano".to_string())
        );
    }
}
