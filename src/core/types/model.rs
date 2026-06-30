//! Model information types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Provider capability enum
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCapability {
    /// Chat completion
    ChatCompletion,
    /// Streaming chat completion
    ChatCompletionStream,
    /// Embeddings generation
    Embeddings,
    /// Image generation
    ImageGeneration,
    /// Image editing
    ImageEdit,
    /// Image variation
    ImageVariation,
    /// Audio transcription
    AudioTranscription,
    /// Audio translation
    AudioTranslation,
    /// Text to speech
    TextToSpeech,
    /// Moderation
    Moderation,
    /// Reranking
    Rerank,
    /// Tool calling
    ToolCalling,
    /// Function calling (backward compatibility)
    FunctionCalling,
    /// Code execution
    CodeExecution,
    /// File upload
    FileUpload,
    /// Fine-tuning
    FineTuning,
    /// Batch processing
    BatchProcessing,
    /// Real-time API
    RealtimeApi,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model ID
    pub id: String,
    /// Model name
    pub name: String,
    /// Provider name
    pub provider: String,
    /// Maximum context length
    pub max_context_length: u32,
    /// Maximum output length
    pub max_output_length: Option<u32>,
    /// Supports streaming
    pub supports_streaming: bool,
    /// Supports tool calling
    pub supports_tools: bool,
    /// Supports multimodal
    pub supports_multimodal: bool,
    /// Input price (per 1K tokens)
    pub input_cost_per_1k_tokens: Option<f64>,
    /// Output price (per 1K tokens)
    pub output_cost_per_1k_tokens: Option<f64>,
    /// Currency unit
    pub currency: String,
    /// Supported features
    pub capabilities: Vec<ProviderCapability>,
    /// Created at
    pub created_at: Option<SystemTime>,
    /// Updated at
    pub updated_at: Option<SystemTime>,
    /// Extra metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Default for ModelInfo {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            provider: String::new(),
            max_context_length: 4096,
            max_output_length: None,
            supports_streaming: false,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_1k_tokens: None,
            output_cost_per_1k_tokens: None,
            currency: "USD".to_string(),
            capabilities: Vec::new(),
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ProviderCapability Tests ====================

    #[test]
    fn test_provider_capability_chat_completion() {
        let cap = ProviderCapability::ChatCompletion;
        let json = serde_json::to_string(&cap).unwrap();
        assert_eq!(json, "\"chat_completion\"");
    }

    #[test]
    fn test_provider_capability_streaming() {
        let cap = ProviderCapability::ChatCompletionStream;
        let json = serde_json::to_string(&cap).unwrap();
        assert_eq!(json, "\"chat_completion_stream\"");
    }

    #[test]
    fn test_provider_capability_embeddings() {
        let cap = ProviderCapability::Embeddings;
        let json = serde_json::to_string(&cap).unwrap();
        assert_eq!(json, "\"embeddings\"");
    }

    #[test]
    fn test_provider_capability_image_generation() {
        let cap = ProviderCapability::ImageGeneration;
        let json = serde_json::to_string(&cap).unwrap();
        assert_eq!(json, "\"image_generation\"");
    }

    #[test]
    fn test_provider_capability_rerank() {
        let cap = ProviderCapability::Rerank;
        let json = serde_json::to_string(&cap).unwrap();
        assert_eq!(json, "\"rerank\"");
    }

    #[test]
    fn test_provider_capability_audio_transcription() {
        let cap = ProviderCapability::AudioTranscription;
        let json = serde_json::to_string(&cap).unwrap();
        assert_eq!(json, "\"audio_transcription\"");
    }

    #[test]
    fn test_provider_capability_tool_calling() {
        let cap = ProviderCapability::ToolCalling;
        let json = serde_json::to_string(&cap).unwrap();
        assert_eq!(json, "\"tool_calling\"");
    }

    #[test]
    fn test_provider_capability_deserialization() {
        let cap: ProviderCapability = serde_json::from_str("\"embeddings\"").unwrap();
        assert_eq!(cap, ProviderCapability::Embeddings);
    }

    #[test]
    fn test_provider_capability_equality() {
        assert_eq!(
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletion
        );
        assert_ne!(
            ProviderCapability::ChatCompletion,
            ProviderCapability::Embeddings
        );
    }

    #[test]
    fn test_provider_capability_clone() {
        let cap = ProviderCapability::FineTuning;
        let cloned = cap.clone();
        assert_eq!(cap, cloned);
    }

    #[test]
    fn test_all_provider_capabilities_serialize() {
        let capabilities = vec![
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
            ProviderCapability::Embeddings,
            ProviderCapability::ImageGeneration,
            ProviderCapability::ImageEdit,
            ProviderCapability::ImageVariation,
            ProviderCapability::AudioTranscription,
            ProviderCapability::AudioTranslation,
            ProviderCapability::TextToSpeech,
            ProviderCapability::Moderation,
            ProviderCapability::Rerank,
            ProviderCapability::ToolCalling,
            ProviderCapability::FunctionCalling,
            ProviderCapability::CodeExecution,
            ProviderCapability::FileUpload,
            ProviderCapability::FineTuning,
            ProviderCapability::BatchProcessing,
            ProviderCapability::RealtimeApi,
        ];

        for cap in capabilities {
            let json = serde_json::to_string(&cap).unwrap();
            assert!(!json.is_empty());
        }
    }

    // ==================== ModelInfo Tests ====================

    #[test]
    fn test_model_info_default() {
        let info = ModelInfo::default();
        assert!(info.id.is_empty());
        assert!(info.name.is_empty());
        assert_eq!(info.max_context_length, 4096);
        assert!(!info.supports_streaming);
        assert!(!info.supports_tools);
        assert!(!info.supports_multimodal);
        assert_eq!(info.currency, "USD");
        assert!(info.capabilities.is_empty());
    }

    #[test]
    fn test_model_info_structure() {
        let info = ModelInfo {
            id: "gpt-4".to_string(),
            name: "GPT-4".to_string(),
            provider: "openai".to_string(),
            max_context_length: 128000,
            max_output_length: Some(4096),
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: true,
            input_cost_per_1k_tokens: Some(0.03),
            output_cost_per_1k_tokens: Some(0.06),
            currency: "USD".to_string(),
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
            ],
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        };

        assert_eq!(info.id, "gpt-4");
        assert_eq!(info.provider, "openai");
        assert_eq!(info.max_context_length, 128000);
        assert!(info.supports_streaming);
    }

    #[test]
    fn test_model_info_with_costs() {
        let info = ModelInfo {
            id: "claude-3".to_string(),
            name: "Claude 3".to_string(),
            provider: "anthropic".to_string(),
            max_context_length: 200000,
            max_output_length: Some(8192),
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: true,
            input_cost_per_1k_tokens: Some(0.015),
            output_cost_per_1k_tokens: Some(0.075),
            currency: "USD".to_string(),
            capabilities: vec![],
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        };

        assert!((info.input_cost_per_1k_tokens.unwrap() - 0.015).abs() < f64::EPSILON);
        assert!((info.output_cost_per_1k_tokens.unwrap() - 0.075).abs() < f64::EPSILON);
    }

    #[test]
    fn test_model_info_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("version".to_string(), serde_json::json!("2024-01"));
        metadata.insert("deprecated".to_string(), serde_json::json!(false));

        let info = ModelInfo {
            id: "model".to_string(),
            name: "Model".to_string(),
            provider: "provider".to_string(),
            max_context_length: 4096,
            max_output_length: None,
            supports_streaming: false,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_1k_tokens: None,
            output_cost_per_1k_tokens: None,
            currency: "USD".to_string(),
            capabilities: vec![],
            created_at: None,
            updated_at: None,
            metadata,
        };

        assert_eq!(
            info.metadata.get("version"),
            Some(&serde_json::json!("2024-01"))
        );
    }

    #[test]
    fn test_model_info_serialization() {
        let info = ModelInfo {
            id: "test-model".to_string(),
            name: "Test Model".to_string(),
            provider: "test".to_string(),
            max_context_length: 8192,
            max_output_length: Some(2048),
            supports_streaming: true,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_1k_tokens: Some(0.01),
            output_cost_per_1k_tokens: None,
            currency: "USD".to_string(),
            capabilities: vec![ProviderCapability::ChatCompletion],
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        };

        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["id"], "test-model");
        assert_eq!(json["provider"], "test");
        assert_eq!(json["max_context_length"], 8192);
        assert!(json["capabilities"].is_array());
    }

    #[test]
    fn test_model_info_clone() {
        let info = ModelInfo {
            id: "clone-test".to_string(),
            name: "Clone Test".to_string(),
            provider: "test".to_string(),
            max_context_length: 4096,
            max_output_length: None,
            supports_streaming: false,
            supports_tools: false,
            supports_multimodal: false,
            input_cost_per_1k_tokens: None,
            output_cost_per_1k_tokens: None,
            currency: "USD".to_string(),
            capabilities: vec![],
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        };

        let cloned = info.clone();
        assert_eq!(info.id, cloned.id);
        assert_eq!(info.provider, cloned.provider);
    }

    #[test]
    fn test_model_info_with_capabilities() {
        let info = ModelInfo {
            id: "full-model".to_string(),
            name: "Full Model".to_string(),
            provider: "provider".to_string(),
            max_context_length: 16384,
            max_output_length: Some(4096),
            supports_streaming: true,
            supports_tools: true,
            supports_multimodal: true,
            input_cost_per_1k_tokens: Some(0.02),
            output_cost_per_1k_tokens: Some(0.04),
            currency: "USD".to_string(),
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
                ProviderCapability::ToolCalling,
                ProviderCapability::ImageGeneration,
            ],
            created_at: None,
            updated_at: None,
            metadata: HashMap::new(),
        };

        assert_eq!(info.capabilities.len(), 4);
        assert!(info.capabilities.contains(&ProviderCapability::ToolCalling));
    }
}
