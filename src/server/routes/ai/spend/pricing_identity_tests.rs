use super::*;

fn priced_model_info() -> crate::core::pricing_service::LiteLLMModelInfo {
    crate::core::pricing_service::LiteLLMModelInfo {
        max_tokens: Some(4096),
        max_input_tokens: Some(4096),
        max_output_tokens: Some(4096),
        input_cost_per_token: Some(0.001),
        output_cost_per_token: Some(0.002),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: "openai".to_string(),
        mode: "chat".to_string(),
        supports_function_calling: Some(true),
        supports_vision: Some(false),
        supports_streaming: Some(true),
        supports_parallel_function_calling: Some(true),
        supports_system_message: Some(true),
        extra: std::collections::HashMap::new(),
    }
}

#[test]
fn retained_mapping_identity_does_not_price_non_image_requests() {
    let pricing = PricingService::new(None);
    let (provider, model) = unpriced_openai_mapping_identity(
        &ProviderType::OpenAICompatible,
        "openai",
        "public-alias",
        "canonical-model",
    )
    .expect("real mapping should retain canonical identity");

    let error = pricing
        .calculate_loaded_usage_cost_for_provider(&provider, &model, &PricingUsage::new(10, 5))
        .expect_err("identity retention must not invent a non-image price");
    assert!(error.to_string().contains("Model not found"));
}

#[tokio::test]
async fn production_resolver_preserves_explicit_unpriced_mapping_over_priced_raw_alias() {
    let pricing = PricingService::new(None);
    pricing.add_custom_model("review-public-alias".to_string(), priced_model_info());
    let mut config = crate::core::providers::openai::OpenAIConfig::default();
    config.base.api_key = Some("sk-test".to_string());
    config.model_mappings.insert(
        "review-public-alias".to_string(),
        "review-canonical-unpriced".to_string(),
    );
    let provider = Provider::OpenAI(
        crate::core::providers::openai::OpenAIProvider::new(config)
            .await
            .expect("test provider should build"),
    );

    let identity = pricing_identity_for_provider(
        &pricing.snapshot(),
        &provider,
        "review-public-alias",
        ProviderCapability::ChatCompletion,
    );

    assert_eq!(
        identity,
        (
            "openai".to_string(),
            "review-canonical-unpriced".to_string()
        )
    );
}

#[tokio::test]
async fn chat_mapping_preserves_configured_provider_when_target_is_unpriced() {
    let pricing = PricingService::new(None);
    let mut config = crate::core::providers::openai::OpenAIConfig {
        provider_name: "review-custom-openai".to_string(),
        ..Default::default()
    };
    config.base.api_key = Some("sk-test".to_string());
    config.model_mappings.insert(
        "review-public-alias".to_string(),
        "review-canonical-unpriced".to_string(),
    );
    let provider = Provider::OpenAI(
        crate::core::providers::openai::OpenAIProvider::new(config)
            .await
            .expect("test provider should build"),
    );

    let identity = pricing_identity_for_provider(
        &pricing.snapshot(),
        &provider,
        "review-public-alias",
        ProviderCapability::ChatCompletion,
    );

    assert_eq!(
        identity,
        (
            "review-custom-openai".to_string(),
            "review-canonical-unpriced".to_string()
        )
    );
}

#[tokio::test]
async fn chat_only_mapping_does_not_change_non_chat_pricing_identity() {
    let pricing = PricingService::new(None);
    let mut raw_info = priced_model_info();
    raw_info.litellm_provider = "review-custom-openai".to_string();
    pricing.add_custom_model("review-public-alias".to_string(), raw_info);
    let mut config = crate::core::providers::openai::OpenAIConfig {
        provider_name: "review-custom-openai".to_string(),
        ..Default::default()
    };
    config.base.api_key = Some("sk-test".to_string());
    config.model_mappings.insert(
        "review-public-alias".to_string(),
        "review-canonical-unpriced".to_string(),
    );
    let provider = Provider::OpenAI(
        crate::core::providers::openai::OpenAIProvider::new(config)
            .await
            .expect("test provider should build"),
    );

    for surface in [
        ProviderCapability::Embeddings,
        ProviderCapability::ImageGeneration,
        ProviderCapability::AudioTranscription,
    ] {
        assert_eq!(
            pricing_identity_for_provider(
                &pricing.snapshot(),
                &provider,
                "review-public-alias",
                surface,
            ),
            (
                "review-custom-openai".to_string(),
                "review-public-alias".to_string()
            )
        );
    }
}
