//! Ollama Provider Tests
//!
//! Comprehensive unit tests for the Ollama provider implementation.

use super::*;
use crate::core::providers::ollama::config::OllamaConfig;
use crate::core::providers::ollama::model_info::{
    get_model_info, OllamaModelEntry, OllamaModelInfo, OllamaShowResponse, OllamaTagsResponse,
};
use crate::core::providers::ollama::streaming::{OllamaStreamChunk, OllamaToolCall};
use crate::core::types::model::ProviderCapability;

// ==================== Config Tests ====================

#[test]
fn test_config_default_values() {
    let config = OllamaConfig::default();
    assert!(config.api_key.is_none());
    assert!(config.api_base.is_none());
    assert_eq!(config.timeout, 120);
    assert_eq!(config.max_retries, 3);
    assert!(!config.debug);
    assert!(config.mirostat.is_none());
    assert!(config.num_ctx.is_none());
}

#[test]
fn test_config_get_api_base_default() {
    let config = OllamaConfig::default();
    assert_eq!(config.get_api_base(), "http://localhost:11434");
}

#[test]
fn test_config_get_api_base_custom() {
    let config = OllamaConfig {
        api_base: Some("http://192.168.1.100:11434".to_string()),
        ..Default::default()
    };
    assert_eq!(config.get_api_base(), "http://192.168.1.100:11434");
}

#[test]
fn test_config_endpoints() {
    let config = OllamaConfig {
        api_base: Some("http://test:11434".to_string()),
        ..Default::default()
    };
    assert_eq!(config.get_chat_endpoint(), "http://test:11434/api/chat");
    assert_eq!(
        config.get_generate_endpoint(),
        "http://test:11434/api/generate"
    );
    assert_eq!(
        config.get_embeddings_endpoint(),
        "http://test:11434/api/embed"
    );
    assert_eq!(config.get_tags_endpoint(), "http://test:11434/api/tags");
    assert_eq!(config.get_show_endpoint(), "http://test:11434/api/show");
}

#[test]
fn test_config_validation_ok() {
    let config = OllamaConfig::default();
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_validation_zero_timeout() {
    let config = OllamaConfig {
        timeout: 0,
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Timeout"));
}

#[test]
fn test_config_validation_invalid_mirostat() {
    let config = OllamaConfig {
        mirostat: Some(5),
        ..Default::default()
    };
    let result = config.validate();
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Mirostat"));
}

#[test]
fn test_config_build_options() {
    let config = OllamaConfig {
        mirostat: Some(1),
        mirostat_eta: Some(0.1),
        mirostat_tau: Some(5.0),
        num_ctx: Some(4096),
        num_gpu: Some(-1),
        num_thread: Some(8),
        repeat_penalty: Some(1.1),
        ..Default::default()
    };

    let options = config.build_options();
    assert_eq!(options["mirostat"], 1);
    assert_eq!(options["mirostat_eta"], 0.1);
    assert_eq!(options["mirostat_tau"], 5.0);
    assert_eq!(options["num_ctx"], 4096);
    assert_eq!(options["num_gpu"], -1);
    assert_eq!(options["num_thread"], 8);
    assert_eq!(options["repeat_penalty"], 1.1);
}

#[test]
fn test_config_serialization() {
    let config = OllamaConfig {
        api_base: Some("http://custom:11434".to_string()),
        timeout: 60,
        num_ctx: Some(8192),
        ..Default::default()
    };

    let json = serde_json::to_value(&config).unwrap();
    assert_eq!(json["api_base"], "http://custom:11434");
    assert_eq!(json["timeout"], 60);
    assert_eq!(json["num_ctx"], 8192);
}

#[test]
fn test_config_deserialization() {
    let json = r#"{
        "api_base": "http://192.168.1.100:11434",
        "timeout": 60,
        "num_ctx": 4096,
        "mirostat": 1
    }"#;

    let config: OllamaConfig = serde_json::from_str(json).unwrap();
    assert_eq!(
        config.api_base,
        Some("http://192.168.1.100:11434".to_string())
    );
    assert_eq!(config.timeout, 60);
    assert_eq!(config.num_ctx, Some(4096));
    assert_eq!(config.mirostat, Some(1));
}

// ==================== Model Info Tests ====================

#[test]
fn test_model_info_new() {
    let info = OllamaModelInfo::new("llama3:8b");
    assert_eq!(info.name, "llama3:8b");
    assert_eq!(info.display_name, "llama3:8b");
    assert!(!info.supports_tools);
    assert!(!info.supports_multimodal);
}

#[test]
fn test_model_info_infer_llama() {
    let info = get_model_info("llama3:8b");
    assert_eq!(info.family, Some("llama".to_string()));
    assert!(info.supports_tools);
    assert!(!info.supports_multimodal);
    assert_eq!(info.parameter_size, Some("8B".to_string()));
}

#[test]
fn test_model_info_infer_vision() {
    let info = get_model_info("llava:13b");
    assert!(info.supports_multimodal);

    let info = get_model_info("llama3-vision:11b");
    assert!(info.supports_multimodal);

    let info = get_model_info("moondream:1.8b");
    assert!(info.supports_multimodal);

    let info = get_model_info("bakllava:7b");
    assert!(info.supports_multimodal);
}

#[test]
fn test_model_info_infer_mistral() {
    let info = get_model_info("mistral:7b");
    assert_eq!(info.family, Some("mistral".to_string()));
    assert!(info.supports_tools);
    assert_eq!(info.max_context_length, Some(32768));
}

#[test]
fn test_model_info_infer_mixtral() {
    let info = get_model_info("mixtral:8x7b");
    assert_eq!(info.family, Some("mixtral".to_string()));
    assert!(info.supports_tools);
}

#[test]
fn test_model_info_infer_qwen() {
    let info = get_model_info("qwen2:7b");
    assert_eq!(info.family, Some("qwen".to_string()));
    assert!(info.supports_tools);
}

#[test]
fn test_model_info_infer_gemma() {
    let info = get_model_info("gemma:7b");
    assert_eq!(info.family, Some("gemma".to_string()));
    assert_eq!(info.max_context_length, Some(8192));
}

#[test]
fn test_model_info_infer_deepseek() {
    let info = get_model_info("deepseek-coder:6.7b");
    assert_eq!(info.family, Some("deepseek".to_string()));
    assert!(info.supports_tools);
}

#[test]
fn test_model_info_infer_phi() {
    let info = get_model_info("phi:3b");
    assert_eq!(info.family, Some("phi".to_string()));
    assert_eq!(info.max_context_length, Some(4096));
}

#[test]
fn test_show_response_supports_tools() {
    let response = OllamaShowResponse {
        modelfile: None,
        parameters: None,
        template: Some("{{ .Tools }}".to_string()),
        details: None,
        model_info: None,
    };
    assert!(response.supports_tools());

    let response = OllamaShowResponse {
        modelfile: None,
        parameters: None,
        template: Some("{{ .System }}".to_string()),
        details: None,
        model_info: None,
    };
    assert!(!response.supports_tools());
}

#[test]
fn test_show_response_get_context_length() {
    let response = OllamaShowResponse {
        modelfile: None,
        parameters: None,
        template: None,
        details: None,
        model_info: Some(serde_json::json!({
            "context_length": 8192
        })),
    };
    assert_eq!(response.get_context_length(), Some(8192));

    let response = OllamaShowResponse {
        modelfile: None,
        parameters: None,
        template: None,
        details: None,
        model_info: Some(serde_json::json!({
            "num_ctx": 4096
        })),
    };
    assert_eq!(response.get_context_length(), Some(4096));
}

#[test]
fn test_tags_response_deserialization() {
    let json = r#"{
        "models": [
            {
                "name": "llama3:8b",
                "modified_at": "2024-01-01T00:00:00Z",
                "size": 4000000000,
                "details": {
                    "family": "llama",
                    "parameter_size": "8B"
                }
            },
            {
                "name": "mistral:7b",
                "modified_at": "2024-01-02T00:00:00Z",
                "size": 3500000000
            }
        ]
    }"#;

    let response: OllamaTagsResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.models.len(), 2);
    assert_eq!(response.models[0].name, "llama3:8b");
    assert_eq!(response.models[1].name, "mistral:7b");
}

#[test]
fn test_model_entry_to_model_info() {
    let entry = OllamaModelEntry {
        name: "llama3:8b".to_string(),
        model: Some("llama3:8b".to_string()),
        modified_at: Some("2024-01-01T00:00:00Z".to_string()),
        size: Some(4_000_000_000),
        digest: None,
        details: Some(super::model_info::OllamaModelDetails {
            parent_model: None,
            format: Some("gguf".to_string()),
            family: Some("llama".to_string()),
            families: None,
            parameter_size: Some("8B".to_string()),
            quantization_level: Some("Q4_0".to_string()),
        }),
    };

    let info: OllamaModelInfo = entry.into();
    assert_eq!(info.name, "llama3:8b");
    assert_eq!(info.family, Some("llama".to_string()));
    assert_eq!(info.parameter_size, Some("8B".to_string()));
    assert_eq!(info.quantization, Some("Q4_0".to_string()));
    assert!(info.supports_tools);
}

// ==================== Streaming Tests ====================

#[test]
fn test_stream_chunk_deserialization_basic() {
    let json = r#"{
        "model": "llama3:8b",
        "created_at": "2024-01-01T00:00:00Z",
        "message": {
            "role": "assistant",
            "content": "Hello"
        },
        "done": false
    }"#;

    let chunk: OllamaStreamChunk = serde_json::from_str(json).unwrap();
    assert_eq!(chunk.model, "llama3:8b");
    assert!(!chunk.done);
    assert!(chunk.message.is_some());
    assert_eq!(chunk.message.unwrap().content, Some("Hello".to_string()));
}

#[test]
fn test_stream_chunk_deserialization_done() {
    let json = r#"{
        "model": "llama3:8b",
        "message": {
            "role": "assistant",
            "content": ""
        },
        "done": true,
        "done_reason": "stop",
        "prompt_eval_count": 10,
        "eval_count": 50,
        "total_duration": 1000000000
    }"#;

    let chunk: OllamaStreamChunk = serde_json::from_str(json).unwrap();
    assert!(chunk.done);
    assert_eq!(chunk.done_reason, Some("stop".to_string()));
    assert_eq!(chunk.prompt_eval_count, Some(10));
    assert_eq!(chunk.eval_count, Some(50));
}

#[test]
fn test_stream_chunk_deserialization_tool_calls() {
    let json = r#"{
        "model": "llama3:8b",
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [
                {
                    "function": {
                        "name": "get_weather",
                        "arguments": {"location": "NYC"}
                    }
                }
            ]
        },
        "done": true,
        "done_reason": "tool_calls"
    }"#;

    let chunk: OllamaStreamChunk = serde_json::from_str(json).unwrap();
    let tool_calls = chunk.message.as_ref().unwrap().tool_calls.as_ref().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].function.name, "get_weather");
}

#[test]
fn test_stream_chunk_deserialization_thinking() {
    let json = r#"{
        "model": "deepseek-r1",
        "message": {
            "role": "assistant",
            "content": "",
            "thinking": "Let me think about this..."
        },
        "done": false
    }"#;

    let chunk: OllamaStreamChunk = serde_json::from_str(json).unwrap();
    let message = chunk.message.unwrap();
    assert_eq!(
        message.thinking,
        Some("Let me think about this...".to_string())
    );
}

#[test]
fn test_stream_chunk_deserialization_error() {
    let json = r#"{
        "model": "llama3:8b",
        "error": "model not found",
        "done": true
    }"#;

    let chunk: OllamaStreamChunk = serde_json::from_str(json).unwrap();
    assert_eq!(chunk.error, Some("model not found".to_string()));
}

#[test]
fn test_tool_call_serialization() {
    let tool_call = OllamaToolCall {
        id: Some("call_123".to_string()),
        function: super::streaming::OllamaToolFunction {
            name: "get_weather".to_string(),
            arguments: serde_json::json!({"location": "NYC"}),
        },
    };

    let json = serde_json::to_string(&tool_call).unwrap();
    assert!(json.contains("get_weather"));
    assert!(json.contains("NYC"));
}

// ==================== Provider Tests ====================

#[tokio::test]
async fn test_provider_creation() {
    let provider = OllamaProvider::new(OllamaConfig::default()).await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(provider.name(), "ollama");
}

#[tokio::test]
async fn test_provider_with_base_url() {
    let provider = OllamaProvider::with_base_url("http://192.168.1.100:11434").await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_capabilities() {
    let provider = OllamaProvider::new(OllamaConfig::default()).await.unwrap();
    let capabilities = provider.capabilities();

    assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
    assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
    assert!(capabilities.contains(&ProviderCapability::Embeddings));
    assert!(capabilities.contains(&ProviderCapability::ToolCalling));
}

#[tokio::test]
async fn test_provider_supported_params() {
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    let provider = OllamaProvider::new(OllamaConfig::default()).await.unwrap();
    let params = provider.get_supported_openai_params("llama3:8b");

    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"top_p"));
    assert!(params.contains(&"max_tokens"));
    assert!(params.contains(&"stream"));
    assert!(params.contains(&"stop"));
    assert!(params.contains(&"tools"));
    assert!(params.contains(&"num_ctx"));
    assert!(params.contains(&"num_predict"));
    assert!(params.contains(&"repeat_penalty"));
    assert!(!params.contains(&"mirostat"));
}

#[tokio::test]
async fn test_provider_map_openai_params() {
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
    use std::collections::HashMap;

    let provider = OllamaProvider::new(OllamaConfig::default()).await.unwrap();

    let mut params = HashMap::new();
    params.insert("max_tokens".to_string(), serde_json::json!(100));
    params.insert("temperature".to_string(), serde_json::json!(0.7));

    let mapped = provider
        .map_openai_params(params, "llama3:8b")
        .await
        .unwrap();

    // max_tokens should be mapped to num_predict
    assert!(mapped.contains_key("num_predict"));
    assert!(!mapped.contains_key("max_tokens"));
    assert!(mapped.contains_key("temperature"));
}

#[tokio::test]
async fn test_provider_calculate_cost() {
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    let provider = OllamaProvider::new(OllamaConfig::default()).await.unwrap();

    // Ollama is free, so cost should always be 0
    let cost = provider
        .calculate_cost("llama3:8b", 1000, 500)
        .await
        .unwrap();
    assert_eq!(cost, 0.0);
}
