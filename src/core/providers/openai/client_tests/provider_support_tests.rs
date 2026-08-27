use super::*;

// ==================== Provider Creation Tests ====================

#[tokio::test]
async fn test_provider_creation() {
    let mut config = OpenAIConfig::default();
    config.base.api_key = Some("sk-test123".to_string());

    let provider = OpenAIProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_creation_with_api_key() {
    let provider = OpenAIProvider::with_api_key("sk-testkey123").await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(provider.name(), "openai");
}

#[tokio::test]
async fn test_provider_creation_with_organization() {
    let mut config = OpenAIConfig::default();
    config.base.api_key = Some("sk-test123".to_string());
    config.organization = Some("org-test123".to_string());

    let provider = OpenAIProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_creation_with_project() {
    let mut config = OpenAIConfig::default();
    config.base.api_key = Some("sk-test123".to_string());
    config.project = Some("proj-test123".to_string());

    let provider = OpenAIProvider::new(config).await;
    assert!(provider.is_ok());
}

#[tokio::test]
async fn test_provider_creation_no_api_key() {
    let config = OpenAIConfig::default();
    let provider = OpenAIProvider::new(config).await;
    assert!(provider.is_err());
}

// ==================== Provider Properties Tests ====================

#[test]
fn test_provider_name() {
    let provider = create_test_provider();
    assert_eq!(provider.name(), "openai");
}

#[test]
fn test_provider_capabilities() {
    let provider = create_test_provider();
    let caps = provider.capabilities();

    assert!(caps.contains(&ProviderCapability::ChatCompletion));
    assert!(caps.contains(&ProviderCapability::ChatCompletionStream));
    assert!(caps.contains(&ProviderCapability::Embeddings));
    assert!(caps.contains(&ProviderCapability::ImageGeneration));
    assert!(caps.contains(&ProviderCapability::AudioTranscription));
    assert!(caps.contains(&ProviderCapability::ToolCalling));
    assert!(caps.contains(&ProviderCapability::FunctionCalling));
    assert!(caps.contains(&ProviderCapability::FineTuning));
    assert!(caps.contains(&ProviderCapability::ImageEdit));
    assert!(caps.contains(&ProviderCapability::ImageVariation));
    assert!(caps.contains(&ProviderCapability::RealtimeApi));
}

#[test]
fn test_provider_models_not_empty() {
    let provider = create_test_provider();
    assert!(!provider.models().is_empty());
}

// ==================== Model Support Tests ====================

#[test]
fn test_model_support_detection() {
    let provider = create_test_provider();

    // Test GPT-4 capabilities
    assert!(provider.model_supports_capability("gpt-4", &ProviderCapability::ChatCompletion));
    assert!(provider.model_supports_capability("gpt-4", &ProviderCapability::ToolCalling));

    // Test embedding model
    assert!(!provider.model_supports_capability(
        "text-embedding-ada-002",
        &ProviderCapability::ChatCompletion
    ));
}

#[test]
fn test_model_supports_capability_unknown_model() {
    let provider = create_test_provider();
    assert!(
        !provider.model_supports_capability("unknown-model", &ProviderCapability::ChatCompletion)
    );
}

#[test]
fn test_get_model_info() {
    let provider = create_test_provider();

    let model_info = provider.get_model_info("gpt-4");
    assert!(model_info.is_ok());

    let info = model_info.unwrap();
    assert_eq!(info.id, "gpt-4");
    assert_eq!(info.provider, "openai");
    assert!(info.supports_streaming);
    assert!(info.supports_tools);
}

#[test]
fn test_get_model_info_unknown_model() {
    let provider = create_test_provider();

    // Should return default info for unknown models (like Python LiteLLM)
    let model_info = provider.get_model_info("unknown-model-xyz");
    assert!(model_info.is_ok());

    let info = model_info.unwrap();
    assert_eq!(info.id, "unknown-model-xyz");
}

#[test]
fn test_get_model_config() {
    let provider = create_test_provider();

    let config = provider.get_model_config("gpt-4");
    // May or may not have config depending on registry
    let _ = config; // Just verify it doesn't panic
}

// ==================== Supported Params Tests ====================

#[test]
fn test_get_supported_openai_params_gpt4() {
    let provider = create_test_provider();
    let params = provider.get_supported_openai_params("gpt-4");

    assert!(params.contains(&"messages"));
    assert!(params.contains(&"model"));
    assert!(params.contains(&"temperature"));
    assert!(params.contains(&"max_tokens"));
    assert!(params.contains(&"tools"));
    assert!(params.contains(&"tool_choice"));
}

#[test]
fn test_get_supported_openai_params_gpt55() {
    let provider = create_test_provider();
    let params = provider.get_supported_openai_params("gpt-5.5");
    let prefixed_params = provider.get_supported_openai_params("openai/gpt-5.5");

    assert!(params.contains(&"tools"));
    assert!(params.contains(&"tool_choice"));
    assert!(params.contains(&"response_format"));
    assert!(params.contains(&"stream"));
    assert!(params.contains(&"reasoning_effort"));
    assert!(params.contains(&"store"));
    assert!(params.contains(&"metadata"));
    assert!(params.contains(&"service_tier"));
    assert_eq!(params, prefixed_params);
}

#[test]
fn test_get_supported_openai_params_gpt55_pro() {
    let provider = create_test_provider();
    let params = provider.get_supported_openai_params("gpt-5.5-pro");
    let prefixed_params = provider.get_supported_openai_params("openai/gpt-5.5-pro");

    assert!(params.contains(&"tools"));
    assert!(params.contains(&"tool_choice"));
    assert!(params.contains(&"response_format"));
    assert!(params.contains(&"reasoning_effort"));
    assert!(params.contains(&"store"));
    assert!(params.contains(&"metadata"));
    assert!(params.contains(&"service_tier"));
    assert!(!params.contains(&"stream"));
    assert_eq!(params, prefixed_params);
}

#[test]
fn test_get_supported_openai_params_gpt56_family() {
    let provider = create_test_provider();

    for model in [
        "gpt-5.6",
        "gpt-5.6-sol",
        "gpt-5.6-terra",
        "gpt-5.6-luna",
        "gpt-5.6-cyber",
    ] {
        let params = provider.get_supported_openai_params(model);
        let prefixed = provider.get_supported_openai_params(&format!("openai/{model}"));

        for expected in [
            "stream",
            "tools",
            "tool_choice",
            "response_format",
            "reasoning_effort",
            "store",
            "metadata",
            "service_tier",
        ] {
            assert!(
                params.contains(&expected),
                "{model} supported params should include {expected}"
            );
        }
        assert_eq!(params, prefixed);
    }
}

#[test]
fn test_get_supported_openai_params_advertises_forwarded_chat_fields() {
    let provider = create_test_provider();

    for model in [
        "gpt-4o",
        "gpt-4o-mini",
        "gpt-4o-audio-preview",
        "gpt-audio",
        "gpt-4.1",
        "gpt-4.1-mini",
        "gpt-4.1-nano",
        "gpt-5",
        "gpt-5-mini",
        "gpt-5-nano",
        "gpt-5.1",
        "gpt-5.1-thinking",
        "gpt-5.2",
        "gpt-5.2-pro",
        "gpt-5.2-codex",
        "gpt-5.4",
        "gpt-5.4-mini",
        "gpt-5.4-pro",
        "gpt-5.4-nano",
        "gpt-5.5",
        "gpt-5.5-pro",
        "gpt-5.6",
        "gpt-5.6-sol",
        "gpt-5.6-terra",
        "gpt-5.6-luna",
        "gpt-5.6-cyber",
        "o1-preview",
        "o1-pro",
        "o3",
        "o3-pro",
        "o3-mini",
        "o4-mini",
    ] {
        let params = provider.get_supported_openai_params(model);

        for forwarded_param in ["store", "metadata", "service_tier"] {
            assert!(
                params.contains(&forwarded_param),
                "{model} supported params should advertise forwarded field {forwarded_param}"
            );
        }
    }
}

#[test]
fn test_get_supported_openai_params_gpt35() {
    let provider = create_test_provider();
    let params = provider.get_supported_openai_params("gpt-3.5-turbo");

    assert!(params.contains(&"messages"));
    assert!(params.contains(&"temperature"));
}

#[test]
fn test_get_supported_openai_params_o1() {
    let provider = create_test_provider();
    let params = provider.get_supported_openai_params("o1-preview");

    // O1 models may or may not be in the registry - check basic params
    assert!(params.contains(&"messages"));
    assert!(params.contains(&"model"));
    // If not in registry, defaults to basic params without max_completion_tokens
}

#[test]
fn test_get_supported_openai_params_unknown() {
    let provider = create_test_provider();
    let params = provider.get_supported_openai_params("unknown-model");

    // Should return default params
    assert!(params.contains(&"messages"));
    assert!(params.contains(&"model"));
    assert!(params.contains(&"temperature"));
}
