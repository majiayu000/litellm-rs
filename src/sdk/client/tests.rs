//! Tests for LLM client

#[cfg(test)]
use super::llm_client::LLMClient;
use crate::sdk::config::{ConfigBuilder, ProviderType, SdkProviderConfig};
use crate::sdk::errors::SDKError;
use crate::sdk::types::{ChatOptions, SdkChatRequest};
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
