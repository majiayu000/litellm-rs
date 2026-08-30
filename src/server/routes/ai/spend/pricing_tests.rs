use super::*;

#[cfg(test)]
mod mapped_identity_tests {
    use super::*;
    use crate::config::models::provider::ProviderConfig;
    use crate::core::pricing_service::LiteLLMModelInfo;
    use crate::core::providers::openai_like::{OpenAILikeConfig, OpenAILikeProvider};
    use crate::core::router::unified::Router;
    use std::collections::HashMap;

    fn pricing_info(provider: &str, input: f64, output: f64) -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: Some(4_096),
            max_input_tokens: Some(4_096),
            max_output_tokens: Some(1_024),
            input_cost_per_token: Some(input),
            output_cost_per_token: Some(output),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: provider.to_string(),
            mode: "chat".to_string(),
            supports_function_calling: None,
            supports_vision: None,
            supports_streaming: Some(true),
            supports_parallel_function_calling: None,
            supports_system_message: None,
            extra: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn request_pricing_keeps_the_snapshot_pinned_across_a_refresh() {
        let pricing = Arc::new(PricingService::new(None));
        pricing.add_custom_model(
            "race-model".to_string(),
            pricing_info("race-provider", 0.01, 0.02),
        );
        let provider = Provider::OpenAILike(
            OpenAILikeProvider::new_openai_compatible(
                OpenAILikeConfig::new("https://example.com")
                    .with_skip_api_key(true)
                    .with_provider_name("race-provider"),
            )
            .await
            .expect("test provider should build"),
        );

        let pinned = request_pricing_for_provider_with_snapshot_hook(
            &pricing,
            &provider,
            "race-model",
            ProviderCapability::ChatCompletion,
            || {
                pricing
                    .add_custom_model("race-model".to_string(), pricing_info("openai", 1.0, 2.0));
            },
        )
        .expect("the attempt should retain its original pricing generation");

        assert_eq!(pinned.priced_parts(), Some(("race-provider", "race-model")));
        let cost = pinned
            .calculate_usage(&PricingUsage::new(10, 5))
            .expect("the pinned generation should remain priceable");
        assert!((cost.total_cost - 0.2).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn gateway_bound_legacy_mapping_only_changes_chat_pricing_identity() {
        let pricing = Arc::new(PricingService::new(None));
        pricing.add_custom_model(
            "review-wire-alias".to_string(),
            pricing_info("openai", 0.01, 0.02),
        );
        pricing.add_custom_model("gpt-4".to_string(), pricing_info("openai", 1.0, 2.0));
        let mut config = ProviderConfig {
            name: "review-openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test".to_string(),
            models: vec!["review-wire-alias".to_string()],
            ..Default::default()
        };
        config.settings.insert(
            "model_mappings".to_string(),
            serde_json::json!({"review-wire-alias": "gpt-4"}),
        );
        let router = Router::from_gateway_config_with_pricing(&[config], None, pricing.clone())
            .await
            .expect("legacy chat mapping should bind through the production gateway");
        let deployment = router
            .get_deployment("review-openai-review-wire-alias")
            .expect("configured deployment should be published");

        for surface in [
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
        ] {
            let request_pricing = request_pricing_for_provider(
                &pricing,
                &deployment.provider,
                &deployment.model,
                surface,
            )
            .expect("chat pricing should resolve");
            assert_eq!(request_pricing.priced_parts(), Some(("openai", "gpt-4")));
        }

        for surface in [
            ProviderCapability::ImageGeneration,
            ProviderCapability::ImageEdit,
            ProviderCapability::ImageVariation,
            ProviderCapability::Embeddings,
            ProviderCapability::AudioTranscription,
            ProviderCapability::AudioTranslation,
            ProviderCapability::TextToSpeech,
        ] {
            let request_pricing = request_pricing_for_provider(
                &pricing,
                &deployment.provider,
                &deployment.model,
                surface.clone(),
            )
            .expect("non-chat raw pricing should resolve");
            assert_eq!(
                request_pricing.priced_parts(),
                Some(("openai", "review-wire-alias")),
                "legacy chat mapping changed {surface:?} pricing"
            );
        }
    }

    #[test]
    fn unpriced_openai_mapping_retains_canonical_identity_only_for_real_mapping() {
        assert_eq!(
            unpriced_openai_mapping_identity(
                &ProviderType::OpenAICompatible,
                "openai",
                "public-alias",
                "canonical-model",
            ),
            Some(("openai".to_string(), "canonical-model".to_string()))
        );
        assert_eq!(
            unpriced_openai_mapping_identity(
                &ProviderType::OpenAICompatible,
                "openai",
                "same-model",
                "same-model",
            ),
            None
        );
        assert_eq!(
            unpriced_openai_mapping_identity(
                &ProviderType::Anthropic,
                "anthropic",
                "public-alias",
                "canonical-model",
            ),
            None
        );
    }

    #[test]
    fn embedding_budget_fails_for_selected_exact_identity_without_tokenizer() {
        let pricing = PricingService::new(None);
        pricing.add_custom_model(
            "gpt-audio-1.5".to_string(),
            pricing_info("openai", 0.01, 0.02),
        );
        let request_pricing = RequestPricing::from_exact(&pricing, "openai", "gpt-audio-1.5");
        let input = EmbeddingInput::Text("hello".to_string());

        let error = match reserve_embedding_budget_with_request_pricing(
            &request_pricing,
            &GatewayPricingConfig::default(),
            &UnifiedBudgetLimits::new(),
            "selected-openai",
            "wire-audio-deployment",
            &input,
        ) {
            Err(error) => error,
            Ok(_) => panic!("missing exact embedding tokenizer must fail closed"),
        };

        assert!(matches!(
            error,
            ProviderError::InvalidRequest {
                provider: "token_count",
                ..
            }
        ));
        assert!(error.to_string().contains("openai/gpt-audio-1.5"));
    }

    #[test]
    fn transport_binding_uses_capability_identity_for_tokenization() {
        use crate::core::providers::model_identity::{
            ModelIdentityMapping, validate_deployment_identity,
        };
        use crate::core::providers::registry::model_catalog_authority::CatalogAuthority;

        let catalog = CatalogAuthority::from_embedded().expect("embedded catalog");
        let pricing = PricingService::new(None);
        for (transport, wire_model, target, exact, expected_provider, expected_model) in [
            (
                "openai",
                "ft:tenant:custom-chat",
                "gpt-4",
                true,
                "openai",
                "gpt-4",
            ),
            (
                "azure",
                "wire-deployment",
                "openai/gpt-4",
                true,
                "openai",
                "gpt-4",
            ),
            (
                "azure_ai",
                "wire-deployment",
                "openai/gpt-4",
                true,
                "openai",
                "gpt-4",
            ),
            (
                "azure_ai",
                "wire-deployment",
                "azure_ai/Phi-4",
                false,
                "azure_ai",
                "Phi-4",
            ),
        ] {
            let mapping = ModelIdentityMapping::new(Some(target.to_string()), None);
            let binding = validate_deployment_identity(
                "selected",
                transport,
                wire_model,
                Some(&mapping),
                None,
                &catalog,
                &pricing.snapshot(),
            )
            .expect("typed capability mapping should validate");
            let token = token_identity_for_binding(&binding, transport, wire_model)
                .expect("validated capability must yield a token identity");

            assert_eq!(binding.wire_model(), wire_model);
            assert_eq!(token.provider(), expected_provider);
            assert_eq!(token.model(), expected_model);
            assert_eq!(matches!(token, TokenizerIdentity::ExactOpenAi(_)), exact);
        }
    }

    #[test]
    fn pricing_only_binding_has_no_token_identity() {
        use crate::core::providers::model_identity::{
            ModelIdentityMapping, validate_deployment_identity,
        };
        use crate::core::providers::registry::model_catalog_authority::CatalogAuthority;

        let catalog = CatalogAuthority::from_embedded().expect("embedded catalog");
        let pricing = PricingService::new(None);
        pricing.add_custom_model("price-only".to_string(), pricing_info("openai", 0.01, 0.02));
        let mapping = ModelIdentityMapping::new(None, Some("price-only".to_string()));
        let binding = validate_deployment_identity(
            "selected",
            "openai",
            "wire-deployment",
            Some(&mapping),
            None,
            &catalog,
            &pricing.snapshot(),
        )
        .expect("pricing-only startup mapping remains representable");

        let error = token_identity_for_binding(&binding, "selected", "wire-deployment")
            .expect_err("tokenization must fail closed without capability identity");
        assert!(
            error
                .to_string()
                .contains("no validated capability token identity")
        );
    }
}

#[cfg(test)]
mod pricing_identity_tests {
    use std::collections::HashMap;

    use super::*;
    use crate::core::pricing_service::LiteLLMModelInfo;

    fn priced_model_info(provider: &str) -> LiteLLMModelInfo {
        LiteLLMModelInfo {
            max_tokens: Some(4096),
            max_input_tokens: Some(4096),
            max_output_tokens: Some(4096),
            input_cost_per_token: Some(0.001),
            output_cost_per_token: Some(0.002),
            input_cost_per_character: None,
            output_cost_per_character: None,
            cost_per_second: None,
            litellm_provider: provider.to_string(),
            mode: "chat".to_string(),
            supports_function_calling: Some(true),
            supports_vision: Some(false),
            supports_streaming: Some(true),
            supports_parallel_function_calling: Some(true),
            supports_system_message: Some(true),
            extra: HashMap::new(),
        }
    }

    async fn legacy_mapped_provider(provider_name: &str) -> Provider {
        let mut config = crate::core::providers::openai::OpenAIConfig {
            provider_name: provider_name.to_string(),
            ..Default::default()
        };
        config.base.api_key = Some("sk-test".to_string());
        config.model_mappings.insert(
            "review-public-alias".to_string(),
            "review-canonical-unpriced".to_string(),
        );
        Provider::OpenAI(
            crate::core::providers::openai::OpenAIProvider::new(config)
                .await
                .expect("test provider should build"),
        )
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
    async fn legacy_mapping_preserves_explicit_unpriced_target_and_provider() {
        let pricing = PricingService::new(None);
        pricing.add_custom_model(
            "review-public-alias".to_string(),
            priced_model_info("openai"),
        );
        for provider_name in ["openai", "review-custom-openai"] {
            let provider = legacy_mapped_provider(provider_name).await;
            assert_eq!(
                pricing_identity_for_provider(
                    &pricing.snapshot(),
                    &provider,
                    "review-public-alias",
                    ProviderCapability::ChatCompletion,
                ),
                (
                    provider_name.to_string(),
                    "review-canonical-unpriced".to_string(),
                )
            );
        }
    }

    #[tokio::test]
    async fn chat_only_mapping_does_not_change_non_chat_pricing_identity() {
        let pricing = PricingService::new(None);
        pricing.add_custom_model(
            "review-public-alias".to_string(),
            priced_model_info("review-custom-openai"),
        );
        let provider = legacy_mapped_provider("review-custom-openai").await;
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
                    "review-public-alias".to_string(),
                )
            );
        }
    }
}
