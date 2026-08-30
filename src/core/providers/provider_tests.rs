use super::*;

#[test]
fn test_provider_enum_is_send_sync() {
    assert!(matches!(ProviderType::from("openai"), ProviderType::OpenAI));
}

#[tokio::test]
async fn test_provider_capabilities_embeddings_error_names_real_provider() {
    let provider = Provider::Anthropic(
        anthropic::AnthropicProvider::new(anthropic::AnthropicConfig::new_test("test-key"))
            .unwrap(),
    );

    assert!(!provider.supports_capability(&ProviderCapability::Embeddings));

    let err = provider
        .create_embeddings(
            crate::core::types::embedding::EmbeddingRequest {
                model: "claude-3-opus-20240229".to_string(),
                input: crate::core::types::embedding::EmbeddingInput::Text("hello".to_string()),
                user: None,
                encoding_format: None,
                dimensions: None,
                task_type: None,
                truncation: None,
            },
            crate::core::types::context::RequestContext::default(),
        )
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            ProviderError::NotSupported {
                provider: "anthropic",
                ..
            }
        ),
        "expected provider-specific NotSupported, got {err}"
    );
}

#[tokio::test]
async fn test_provider_enum_calculate_cost_delegates_mistral_aliases() {
    let Ok(mistral_provider) = mistral::MistralProvider::new(mistral::MistralConfig {
        api_key: "sk-test".to_string(),
        ..mistral::MistralConfig::default()
    })
    .await
    else {
        panic!("Mistral provider should initialize with a test API key");
    };
    let provider = Provider::Mistral(mistral_provider);

    let Ok(alias_cost) = provider
        .calculate_cost("magistral-medium-1-2", 1000, 500)
        .await
    else {
        panic!("Mistral alias cost should calculate");
    };
    let Ok(canonical_cost) = provider
        .calculate_cost("magistral-medium-2509", 1000, 500)
        .await
    else {
        panic!("Mistral canonical cost should calculate");
    };
    let Ok(devstral_alias_cost) = provider.calculate_cost("devstral-2-2512", 1000, 500).await
    else {
        panic!("Devstral alias cost should calculate");
    };

    assert!((alias_cost - canonical_cost).abs() < 1e-12);
    assert!((alias_cost - 0.0045).abs() < 1e-12);
    assert!((devstral_alias_cost - 0.0014).abs() < 1e-12);
}

#[tokio::test]
async fn test_provider_enum_calculate_cost_strips_openai_prefix() {
    let mut config = openai::OpenAIConfig::default();
    config.base.api_key = Some("sk-test123456789012345678901234567890123456".to_string());
    let Ok(openai_provider) = openai::OpenAIProvider::new(config).await else {
        panic!("OpenAI provider should initialize with a test API key");
    };
    let provider = Provider::OpenAI(openai_provider);

    let Ok(cost) = provider
        .calculate_cost("openai/gpt-5.5-pro", 1000, 500)
        .await
    else {
        panic!("prefixed OpenAI cost should calculate");
    };

    assert!((cost - 0.12).abs() < 1e-12);
}

#[tokio::test]
async fn test_provider_capabilities_image_error_names_real_provider() {
    let provider = Provider::Anthropic(
        anthropic::AnthropicProvider::new(anthropic::AnthropicConfig::new_test("test-key"))
            .unwrap(),
    );

    assert!(!provider.supports_capability(&ProviderCapability::ImageGeneration));

    let err = provider
        .create_images(
            crate::core::types::image::ImageGenerationRequest {
                prompt: "a small test image".to_string(),
                model: Some("claude-3-opus-20240229".to_string()),
                n: None,
                size: None,
                quality: None,
                response_format: None,
                style: None,
                user: None,
            },
            crate::core::types::context::RequestContext::default(),
        )
        .await
        .unwrap_err();

    assert!(
        matches!(
            err,
            ProviderError::NotSupported {
                provider: "anthropic",
                ..
            }
        ),
        "expected provider-specific NotSupported, got {err}"
    );
}

#[tokio::test]
async fn test_provider_supports_capability_for_optional_provider() {
    let mut config = openai::OpenAIConfig::default();
    config.base.api_key = Some("sk-test123456789012345678901234567890123456".to_string());
    let Ok(openai_provider) = openai::OpenAIProvider::new(config).await else {
        panic!("OpenAI provider should initialize with a test API key");
    };
    let provider = Provider::OpenAI(openai_provider);

    assert!(provider.supports_capability(&ProviderCapability::ChatCompletion));
    assert!(provider.supports_capability(&ProviderCapability::ChatCompletionStream));
    assert!(provider.supports_capability(&ProviderCapability::Embeddings));
    assert!(provider.supports_capability(&ProviderCapability::TextToSpeech));
}
