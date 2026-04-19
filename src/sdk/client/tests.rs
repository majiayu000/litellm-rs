//! Tests for LLM client

#[cfg(test)]
use super::llm_client::LLMClient;
use crate::sdk::config::{ConfigBuilder, ProviderType, SdkProviderConfig};
use crate::sdk::errors::SDKError;
use crate::sdk::types::{
    ChatOptions, Content, ContentPart, ImageUrl, Message, Role, SdkChatRequest,
};
use std::collections::HashMap;

fn test_provider_config(id: &str, provider_type: ProviderType, model: &str) -> SdkProviderConfig {
    SdkProviderConfig {
        id: id.to_string(),
        provider_type,
        name: format!("{id} provider"),
        api_key: "test-key".to_string(),
        base_url: None,
        models: vec![model.to_string()],
        enabled: true,
        weight: 1.0,
        rate_limit_rpm: Some(1000),
        rate_limit_tpm: Some(10000),
        settings: HashMap::new(),
    }
}

#[tokio::test]
async fn test_llm_client_creation() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "test",
            ProviderType::OpenAI,
            "gpt-3.5-turbo",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    assert_eq!(client.list_providers().len(), 1);
}

#[tokio::test]
async fn test_provider_selection() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "anthropic",
            ProviderType::Anthropic,
            "claude-3-sonnet-20240229",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();

    let request = SdkChatRequest {
        model: "claude-3-sonnet-20240229".to_string(),
        messages: vec![],
        options: ChatOptions::default(),
    };

    let provider = client.select_provider(&request).await.unwrap();
    assert_eq!(provider.id, "anthropic");
}

#[tokio::test]
async fn test_stream_provider_selection_prefers_default_provider() {
    let config = ConfigBuilder::new()
        .default_provider("openai")
        .add_provider(test_provider_config(
            "anthropic",
            ProviderType::Anthropic,
            "claude-3-sonnet-20240229",
        ))
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-4o-mini",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.select_provider_for_stream(&[]).await.unwrap();

    assert_eq!(provider.id, "openai");
}

#[tokio::test]
async fn test_provider_selection_prefers_default_provider_when_model_unspecified() {
    let config = ConfigBuilder::new()
        .default_provider("openai")
        .add_provider(test_provider_config(
            "anthropic",
            ProviderType::Anthropic,
            "claude-3-sonnet-20240229",
        ))
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-4o-mini",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client
        .select_provider(&SdkChatRequest {
            model: String::new(),
            messages: Vec::new(),
            options: ChatOptions::default(),
        })
        .await
        .unwrap();

    assert_eq!(provider.id, "openai");
}

#[test]
fn test_provider_config_lookup() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-3.5-turbo",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(provider.id, "openai");
    assert_eq!(provider.models, vec!["gpt-3.5-turbo".to_string()]);
}

#[test]
fn test_provider_config_lookup_missing_provider() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-3.5-turbo",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let err = client.provider_config("missing").unwrap_err();

    assert!(matches!(err, SDKError::ProviderNotFound(id) if id == "missing"));
}

#[test]
fn test_provider_default_model_uses_first_configured_model() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-4o-mini",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(
        client.provider_default_model(provider, "fallback-model"),
        "gpt-4o-mini"
    );
}

#[test]
fn test_provider_default_model_falls_back_without_allocating_config_value() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            models: Vec::new(),
            ..test_provider_config("openai", ProviderType::OpenAI, "unused")
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(
        client.provider_default_model(provider, "fallback-model"),
        "fallback-model"
    );
}

#[test]
fn test_provider_base_url_uses_configured_value() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            base_url: Some("https://example.com/custom".to_string()),
            ..test_provider_config("openai", ProviderType::OpenAI, "gpt-4o-mini")
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(
        client.provider_base_url(provider, "https://fallback.example"),
        "https://example.com/custom"
    );
}

#[test]
fn test_provider_base_url_falls_back_to_default() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-4o-mini",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(
        client.provider_base_url(provider, "https://fallback.example"),
        "https://fallback.example"
    );
}

#[test]
fn test_provider_endpoint_uses_shared_url_joining() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-4o-mini",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(
        client.provider_endpoint(provider, "https://api.openai.com/", "/v1/chat/completions"),
        "https://api.openai.com/v1/chat/completions"
    );
}

#[tokio::test]
async fn test_execute_chat_request_anthropic_plain_url_image_returns_invalid_request() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "anthropic",
            ProviderType::Anthropic,
            "claude-sonnet-4-5",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let request = SdkChatRequest {
        model: String::new(),
        messages: vec![Message {
            role: Role::User,
            content: Some(Content::Multimodal(vec![ContentPart::Image {
                image_url: ImageUrl {
                    url: "https://example.com/photo.jpg".to_string(),
                    detail: None,
                },
            }])),
            name: None,
            tool_calls: None,
        }],
        options: ChatOptions::default(),
    };

    let err = client
        .execute_chat_request("anthropic", request)
        .await
        .unwrap_err();
    assert!(
        matches!(err, SDKError::InvalidRequest(_)),
        "expected InvalidRequest, got {err:?}"
    );
}

#[tokio::test]
async fn test_execute_chat_request_anthropic_malformed_data_uri_returns_invalid_request() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "anthropic",
            ProviderType::Anthropic,
            "claude-sonnet-4-5",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let request = SdkChatRequest {
        model: String::new(),
        messages: vec![Message {
            role: Role::User,
            content: Some(Content::Multimodal(vec![ContentPart::Image {
                image_url: ImageUrl {
                    url: "data:image/png;base64,!!!invalid!!!".to_string(),
                    detail: None,
                },
            }])),
            name: None,
            tool_calls: None,
        }],
        options: ChatOptions::default(),
    };

    let err = client
        .execute_chat_request("anthropic", request)
        .await
        .unwrap_err();
    assert!(
        matches!(err, SDKError::InvalidRequest(_)),
        "expected InvalidRequest, got {err:?}"
    );
}

#[test]
fn test_anthropic_messages_endpoint_avoids_duplicate_v1() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            base_url: Some("https://api.anthropic.com/v1".to_string()),
            ..test_provider_config("anthropic", ProviderType::Anthropic, "claude-sonnet-4-5")
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("anthropic").unwrap();

    assert_eq!(
        client.anthropic_messages_endpoint(provider),
        "https://api.anthropic.com/v1/messages"
    );
}

// ==================== Streaming Tests ====================

#[tokio::test]
async fn test_stream_azure_unsupported() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            id: "azure-test".to_string(),
            provider_type: ProviderType::Azure,
            name: "Azure".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some("https://my-resource.openai.azure.com".to_string()),
            models: vec!["gpt-4".to_string()],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(1000),
            rate_limit_tpm: Some(10000),
            settings: HashMap::new(),
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let result = client.execute_stream_request("azure-test", vec![]).await;
    assert!(matches!(result, Err(SDKError::NotSupported(_))));
}

#[tokio::test]
async fn test_stream_unsupported_provider() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            id: "google-test".to_string(),
            provider_type: ProviderType::Google,
            name: "Google".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            models: vec!["gemini-pro".to_string()],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(1000),
            rate_limit_tpm: Some(10000),
            settings: HashMap::new(),
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let result = client.execute_stream_request("google-test", vec![]).await;
    assert!(matches!(result, Err(SDKError::NotSupported(_))));
}

#[tokio::test]
async fn test_stream_provider_not_found() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            id: "openai-test".to_string(),
            provider_type: ProviderType::OpenAI,
            name: "OpenAI".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            models: vec!["gpt-4".to_string()],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(1000),
            rate_limit_tpm: Some(10000),
            settings: HashMap::new(),
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let result = client.execute_stream_request("nonexistent", vec![]).await;
    assert!(matches!(result, Err(SDKError::ProviderNotFound(_))));
}

#[test]
fn test_parse_openai_sse_line_content() {
    use super::completions::parse_openai_sse_line;

    let line = r#"data: {"id":"chatcmpl-abc","model":"gpt-4","choices":[{"index":0,"delta":{"role":null,"content":"Hello","tool_calls":null},"finish_reason":null}]}"#;
    let result = parse_openai_sse_line(line);
    assert!(result.is_some());
    let chunk = result.unwrap().unwrap();
    assert_eq!(chunk.id, "chatcmpl-abc");
    assert_eq!(chunk.model, "gpt-4");
    assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
}

#[test]
fn test_parse_openai_sse_line_done() {
    use super::completions::parse_openai_sse_line;

    let result = parse_openai_sse_line("data: [DONE]");
    assert!(result.is_none());
}

#[test]
fn test_parse_openai_sse_line_malformed() {
    use super::completions::parse_openai_sse_line;

    let result = parse_openai_sse_line("data: {not valid json}");
    assert!(result.is_some());
    assert!(matches!(result.unwrap(), Err(SDKError::ParseError(_))));
}

#[test]
fn test_parse_anthropic_sse_record_delta() {
    use super::completions::parse_anthropic_sse_record;

    let data =
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
    let result = parse_anthropic_sse_record("content_block_delta", data, None);
    assert!(result.is_some());
    let chunk = result.unwrap().unwrap();
    assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
}

#[test]
fn test_parse_anthropic_sse_record_stop() {
    use super::completions::parse_anthropic_sse_record;

    let result = parse_anthropic_sse_record("message_stop", r#"{"type":"message_stop"}"#, None);
    assert!(result.is_none());
}

#[test]
fn test_parse_anthropic_sse_record_message_delta_end_turn_maps_to_stop() {
    use super::completions::parse_anthropic_sse_record;

    let data = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":10}}"#;
    let chunk = parse_anthropic_sse_record("message_delta", data, None)
        .unwrap()
        .unwrap();
    assert_eq!(chunk.choices[0].finish_reason, Some("stop".to_string()));
}

#[test]
fn test_parse_anthropic_sse_record_message_delta_max_tokens_maps_to_length() {
    use super::completions::parse_anthropic_sse_record;

    let data = r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens","stop_sequence":null},"usage":{"output_tokens":100}}"#;
    let chunk = parse_anthropic_sse_record("message_delta", data, None)
        .unwrap()
        .unwrap();
    assert_eq!(chunk.choices[0].finish_reason, Some("length".to_string()));
}

#[test]
fn test_parse_anthropic_sse_record_message_delta_tool_use_maps_to_tool_calls() {
    use super::completions::parse_anthropic_sse_record;

    let data = r#"{"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":5}}"#;
    let chunk = parse_anthropic_sse_record("message_delta", data, None)
        .unwrap()
        .unwrap();
    assert_eq!(
        chunk.choices[0].finish_reason,
        Some("tool_calls".to_string())
    );
}

#[test]
fn test_parse_anthropic_sse_record_ignored_events() {
    use super::completions::parse_anthropic_sse_record;

    let result = parse_anthropic_sse_record(
        "message_start",
        r#"{"type":"message_start","message":{"id":"msg_1","model":"claude-3"}}"#,
        None,
    );
    assert!(result.is_none());

    let result = parse_anthropic_sse_record("ping", r#"{}"#, None);
    assert!(result.is_none());
}
