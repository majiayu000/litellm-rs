use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

use super::client::OpenAIProvider;
use super::models::{OpenAIModelFamily, OpenAIModelFeature, OpenAIUseCase, get_openai_registry};

impl OpenAIProvider {
    /// Get model recommendations for specific use cases
    pub fn get_recommended_model(&self, use_case: OpenAIUseCase) -> Option<String> {
        get_openai_registry().get_recommended_model(use_case)
    }

    /// Check if a model supports a specific feature
    pub fn model_supports_feature(&self, model_id: &str, feature: &OpenAIModelFeature) -> bool {
        get_openai_registry().supports_feature(model_id, feature)
    }

    /// Get models by family (e.g., all GPT-4 variants)
    pub fn get_models_by_family(&self, family: &OpenAIModelFamily) -> Vec<String> {
        get_openai_registry().get_models_by_family(family)
    }

    /// Get models supporting specific feature
    pub fn get_models_with_feature(&self, feature: &OpenAIModelFeature) -> Vec<String> {
        get_openai_registry().get_models_with_feature(feature)
    }

    /// List available models from static registry
    pub fn list_available_models(&self) -> Vec<String> {
        self.models().iter().map(|m| m.id.clone()).collect()
    }

    /// Get model pricing information
    pub fn get_model_pricing(&self, model_id: &str) -> Option<(f64, f64)> {
        if let Ok(pricing) = crate::core::cost::calculator::get_model_pricing(model_id, "openai") {
            return Some((
                pricing.input_cost_per_1k_tokens,
                pricing.output_cost_per_1k_tokens,
            ));
        }

        if let Some(model_info) = self
            .model_registry
            .get_model_spec(model_id)
            .map(|spec| &spec.model_info)
            && let (Some(input_cost), Some(output_cost)) = (
                model_info.input_cost_per_1k_tokens,
                model_info.output_cost_per_1k_tokens,
            )
        {
            return Some((input_cost, output_cost));
        }
        None
    }

    /// Get model context window information
    pub fn get_model_context_window(&self, model_id: &str) -> Result<u32, ProviderError> {
        let model_info = self.get_model_info(model_id)?;
        Ok(model_info.max_context_length)
    }

    /// Check if model supports vision/multimodal input
    pub fn model_supports_vision(&self, model_id: &str) -> bool {
        self.model_supports_feature(model_id, &OpenAIModelFeature::VisionSupport)
    }

    /// Check if model supports function/tool calling
    pub fn model_supports_tools(&self, model_id: &str) -> bool {
        self.model_supports_feature(model_id, &OpenAIModelFeature::FunctionCalling)
    }

    /// Check if model supports streaming
    pub fn model_supports_streaming(&self, model_id: &str) -> bool {
        self.model_supports_feature(model_id, &OpenAIModelFeature::StreamingSupport)
    }

    /// Get best model for specific task
    pub fn get_best_model_for_task(&self, task: OpenAITask) -> Option<String> {
        match task {
            OpenAITask::GeneralChat => self.get_recommended_model(OpenAIUseCase::GeneralChat),
            OpenAITask::CodeGeneration => self.get_recommended_model(OpenAIUseCase::CodeGeneration),
            OpenAITask::ComplexReasoning => self.get_recommended_model(OpenAIUseCase::Reasoning),
            OpenAITask::VisionAnalysis => self.get_recommended_model(OpenAIUseCase::Vision),
            OpenAITask::ImageGeneration => {
                self.get_recommended_model(OpenAIUseCase::ImageGeneration)
            }
            OpenAITask::AudioTranscription => {
                self.get_recommended_model(OpenAIUseCase::AudioTranscription)
            }
            OpenAITask::Embeddings => self.get_recommended_model(OpenAIUseCase::Embeddings),
            OpenAITask::CostSensitive => self.get_recommended_model(OpenAIUseCase::CostOptimized),
        }
    }
}

/// OpenAI task categories for model selection
#[derive(Debug, Clone)]
pub enum OpenAITask {
    GeneralChat,
    CodeGeneration,
    ComplexReasoning,
    VisionAnalysis,
    ImageGeneration,
    AudioTranscription,
    Embeddings,
    CostSensitive,
}
