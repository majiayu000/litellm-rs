use super::*;
use crate::core::pricing_service::PricingUsage;

fn create_test_model_info(provider: &str) -> LiteLLMModelInfo {
    LiteLLMModelInfo {
        max_tokens: Some(4096),
        max_input_tokens: Some(4096),
        max_output_tokens: Some(4096),
        input_cost_per_token: Some(0.00001),
        output_cost_per_token: Some(0.00003),
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

fn create_character_based_model_info() -> LiteLLMModelInfo {
    LiteLLMModelInfo {
        max_tokens: Some(8192),
        max_input_tokens: Some(8192),
        max_output_tokens: Some(8192),
        input_cost_per_token: None,
        output_cost_per_token: None,
        input_cost_per_character: Some(0.000001),
        output_cost_per_character: Some(0.000002),
        cost_per_second: None,
        litellm_provider: "google".to_string(),
        mode: "chat".to_string(),
        supports_function_calling: Some(true),
        supports_vision: Some(true),
        supports_streaming: Some(true),
        supports_parallel_function_calling: Some(false),
        supports_system_message: Some(true),
        extra: HashMap::new(),
    }
}

fn create_time_based_model_info(provider: &str) -> LiteLLMModelInfo {
    let mut model_info = create_test_model_info(provider);
    model_info.input_cost_per_token = None;
    model_info.output_cost_per_token = None;
    model_info.cost_per_second = Some(0.001);
    model_info
}

// ==================== Provider and Model Listing Tests ====================

#[test]
fn test_get_models_by_provider_empty() {
    let service = PricingService::new(None);
    let models = service.get_models_by_provider("openai");
    assert!(models.is_empty());
}

#[test]
fn test_get_models_by_provider_with_models() {
    let service = PricingService::new(None);
    service.add_custom_model("gpt-4".to_string(), create_test_model_info("openai"));
    service.add_custom_model("gpt-3.5".to_string(), create_test_model_info("openai"));
    service.add_custom_model("claude-3".to_string(), create_test_model_info("anthropic"));

    let openai_models = service.get_models_by_provider("openai");
    assert_eq!(openai_models.len(), 2);
    assert!(openai_models.contains(&"gpt-4".to_string()));
    assert!(openai_models.contains(&"gpt-3.5".to_string()));

    let anthropic_models = service.get_models_by_provider("anthropic");
    assert_eq!(anthropic_models.len(), 1);
    assert!(anthropic_models.contains(&"claude-3".to_string()));
}

#[test]
fn test_get_providers_empty() {
    let service = PricingService::new(None);
    let providers = service.get_providers();
    assert!(providers.is_empty());
}

#[test]
fn test_get_providers_with_models() {
    let service = PricingService::new(None);
    service.add_custom_model("gpt-4".to_string(), create_test_model_info("openai"));
    service.add_custom_model("claude-3".to_string(), create_test_model_info("anthropic"));
    service.add_custom_model("gemini-pro".to_string(), create_test_model_info("google"));

    let providers = service.get_providers();
    assert_eq!(providers.len(), 3);
    // Sorted alphabetically
    assert_eq!(providers[0], "anthropic");
    assert_eq!(providers[1], "google");
    assert_eq!(providers[2], "openai");
}

#[test]
fn test_get_providers_deduplication() {
    let service = PricingService::new(None);
    service.add_custom_model("gpt-4".to_string(), create_test_model_info("openai"));
    service.add_custom_model("gpt-3.5".to_string(), create_test_model_info("openai"));
    service.add_custom_model("gpt-4-turbo".to_string(), create_test_model_info("openai"));

    let providers = service.get_providers();
    assert_eq!(providers.len(), 1);
    assert_eq!(providers[0], "openai");
}

#[test]
fn issue_760_pricing_service_keeps_zai_separate_from_zhipu() {
    let service = match PricingService::with_embedded_default() {
        Ok(service) => service,
        Err(error) => panic!("embedded pricing service should initialize for tests: {error}"),
    };

    let Some((resolved_model, info)) = service.get_model_info_for_provider("zai", "zai/glm-5")
    else {
        panic!("ZAI provider should resolve ZAI pricing rows");
    };

    assert_eq!(resolved_model, "zai/glm-5");
    assert_eq!(info.litellm_provider, "zai");
    assert!(
        service
            .get_model_info_for_provider("zhipu", "zai/glm-5")
            .is_none(),
        "Zhipu must not borrow distinct ZAI pricing rows"
    );
    assert!(
        service
            .get_model_info_for_provider("zhipuai", "zai/glm-5")
            .is_none(),
        "ZhipuAI aliases must not borrow distinct ZAI pricing rows"
    );
}

// ==================== Add Custom Model Tests ====================

#[test]
fn test_add_custom_model() {
    let service = PricingService::new(None);
    let model_info = create_test_model_info("custom");

    service.add_custom_model("my-custom-model".to_string(), model_info.clone());

    let result = service.get_model_info("my-custom-model");
    assert!(result.is_some());
    assert_eq!(result.unwrap().litellm_provider, "custom");
}

#[test]
fn test_add_custom_model_overwrites() {
    let service = PricingService::new(None);
    let model_info1 = create_test_model_info("provider1");
    let model_info2 = create_test_model_info("provider2");

    service.add_custom_model("model".to_string(), model_info1);
    service.add_custom_model("model".to_string(), model_info2);

    let result = service.get_model_info("model");
    assert!(result.is_some());
    assert_eq!(result.unwrap().litellm_provider, "provider2");
}

// ==================== Statistics Tests ====================

#[test]
fn test_get_statistics_empty() {
    let service = PricingService::new(None);
    let stats = service.get_statistics();

    assert_eq!(stats.total_models, 0);
    assert!(stats.provider_stats.is_empty());
    assert!(stats.cost_ranges.is_empty());
}

#[test]
fn test_get_statistics_with_models() {
    let service = PricingService::new(None);
    service.add_custom_model("gpt-4".to_string(), create_test_model_info("openai"));
    service.add_custom_model("gpt-3.5".to_string(), create_test_model_info("openai"));
    service.add_custom_model("claude-3".to_string(), create_test_model_info("anthropic"));

    let stats = service.get_statistics();

    assert_eq!(stats.total_models, 3);
    assert_eq!(*stats.provider_stats.get("openai").unwrap(), 2);
    assert_eq!(*stats.provider_stats.get("anthropic").unwrap(), 1);
}

#[test]
fn test_get_statistics_cost_ranges() {
    let service = PricingService::new(None);

    let mut cheap_model = create_test_model_info("openai");
    cheap_model.input_cost_per_token = Some(0.000001);
    cheap_model.output_cost_per_token = Some(0.000002);

    let mut expensive_model = create_test_model_info("openai");
    expensive_model.input_cost_per_token = Some(0.00006);
    expensive_model.output_cost_per_token = Some(0.00012);

    service.add_custom_model("gpt-3.5".to_string(), cheap_model);
    service.add_custom_model("gpt-4".to_string(), expensive_model);

    let stats = service.get_statistics();

    let range = stats.cost_ranges.get("openai").unwrap();
    assert_eq!(range.input_min, 0.000001);
    assert_eq!(range.input_max, 0.00006);
    assert_eq!(range.output_min, 0.000002);
    assert_eq!(range.output_max, 0.00012);
}

// ==================== Google/Character-Based Cost Tests ====================

#[test]
fn test_calculate_google_cost_character_based() {
    let service = PricingService::new(None);
    let model_info = create_character_based_model_info();

    let prompt = "Hello, world!"; // 13 chars
    let completion = "Hi there!"; // 9 chars

    let result = service
        .calculate_google_cost(
            "gemini-pro",
            &model_info,
            10,
            5,
            Some(prompt),
            Some(completion),
        )
        .unwrap();

    assert_eq!(result.cost_type, CostType::CharacterBased);
    assert_eq!(result.input_cost, 13.0 * 0.000001);
    assert_eq!(result.output_cost, 9.0 * 0.000002);
}

#[test]
fn test_calculate_google_cost_fallback_to_token() {
    let service = PricingService::new(None);
    let model_info = create_test_model_info("google");

    let result = service
        .calculate_google_cost(
            "gemini-pro",
            &model_info,
            1000,
            500,
            Some("prompt"),
            Some("completion"),
        )
        .unwrap();

    // Should fall back to token-based
    assert_eq!(result.cost_type, CostType::TokenBased);
}

#[test]
fn test_calculate_google_cost_no_text() {
    let service = PricingService::new(None);
    let model_info = create_character_based_model_info();

    let result = service
        .calculate_google_cost("gemini-pro", &model_info, 10, 5, None, None)
        .unwrap();

    // Should still calculate based on 0 characters
    assert_eq!(result.input_cost, 0.0);
    assert_eq!(result.output_cost, 0.0);
}

#[test]
fn test_calculate_google_cost_partial_character_pricing() {
    let service = PricingService::new(None);
    let mut model_info = create_character_based_model_info();
    model_info.output_cost_per_character = None;

    let result =
        service.calculate_google_cost("gemini-pro", &model_info, 10, 5, Some("p"), Some("c"));

    assert!(matches!(
        result,
        Err(GatewayError::Config(message))
            if message.contains("gemini-pro") && message.contains("character pricing")
    ));
}

#[test]
fn test_calculate_token_based_cost_partial_token_pricing() {
    let service = PricingService::new(None);
    let mut model_info = create_test_model_info("openai");
    model_info.input_cost_per_token = None;

    let result = service.calculate_token_based_cost("gpt-partial", &model_info, 1000, 500);

    assert!(matches!(
        result,
        Err(GatewayError::Config(message))
            if message.contains("gpt-partial") && message.contains("input_cost_per_token")
    ));
}

#[tokio::test]
async fn test_calculate_completion_cost_propagates_missing_token_pricing() {
    let service = PricingService::new(None);
    let mut model_info = create_test_model_info("openai");
    model_info.output_cost_per_token = None;
    service.add_custom_model("gpt-public-missing-price".to_string(), model_info);

    let result = service
        .calculate_completion_cost("gpt-public-missing-price", 1000, 500, None, None, None)
        .await;

    assert!(matches!(
        result,
        Err(GatewayError::Config(message))
            if message.contains("gpt-public-missing-price")
                && message.contains("output_cost_per_token")
    ));
}

#[tokio::test]
async fn test_calculate_completion_cost_requires_time_for_time_based_pricing() {
    let service = PricingService::new(None);
    let model_info = create_time_based_model_info("replicate");
    service.add_custom_model("replicate/timed".to_string(), model_info);

    let result = service
        .calculate_completion_cost("replicate/timed", 0, 0, None, None, None)
        .await;

    assert!(matches!(
        result,
        Err(GatewayError::Validation(message))
            if message.contains("replicate/timed") && message.contains("total_time_seconds")
    ));
}

#[test]
fn pricing_review_rejects_negative_token_pricing_field() {
    let service = PricingService::new(None);
    let mut model_info = create_test_model_info("openai");
    model_info.input_cost_per_token = Some(-0.00001);

    let result = service.calculate_token_based_cost("gpt-negative-token", &model_info, 1000, 500);

    assert!(matches!(
        result,
        Err(GatewayError::Config(message))
            if message.contains("gpt-negative-token")
                && message.contains("input_cost_per_token")
                && message.contains("Invalid token pricing")
    ));
}

#[test]
fn pricing_review_rejects_nan_token_pricing_field() {
    let service = PricingService::new(None);
    let mut model_info = create_test_model_info("openai");
    model_info.output_cost_per_token = Some(f64::NAN);

    let result = service.calculate_token_based_cost("gpt-nan-token", &model_info, 1000, 500);

    assert!(matches!(
        result,
        Err(GatewayError::Config(message))
            if message.contains("gpt-nan-token")
                && message.contains("output_cost_per_token")
                && message.contains("Invalid token pricing")
    ));
}

#[test]
fn pricing_review_rejects_negative_character_pricing_field() {
    let service = PricingService::new(None);
    let mut model_info = create_character_based_model_info();
    model_info.output_cost_per_character = Some(-0.000002);

    let result = service.calculate_google_cost(
        "gemini-negative-char",
        &model_info,
        10,
        5,
        Some("p"),
        Some("c"),
    );

    assert!(matches!(
        result,
        Err(GatewayError::Config(message))
            if message.contains("gemini-negative-char")
                && message.contains("output_cost_per_character")
                && message.contains("Invalid character pricing")
    ));
}

#[test]
fn pricing_review_rejects_negative_time_pricing_field() {
    let service = PricingService::new(None);
    let mut model_info = create_time_based_model_info("replicate");
    model_info.cost_per_second = Some(-0.001);

    let result = service.calculate_time_based_cost("replicate/negative-time", &model_info, 10.0);

    assert!(matches!(
        result,
        Err(GatewayError::Config(message))
            if message.contains("replicate/negative-time")
                && message.contains("cost_per_second")
                && message.contains("Invalid time pricing")
    ));
}

#[tokio::test]
async fn pricing_review_rejects_negative_total_time_seconds() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "replicate/negative-duration".to_string(),
        create_time_based_model_info("replicate"),
    );

    let result = service
        .calculate_completion_cost("replicate/negative-duration", 0, 0, None, None, Some(-1.0))
        .await;

    assert!(matches!(
        result,
        Err(GatewayError::Validation(message))
            if message.contains("replicate/negative-duration")
                && message.contains("Invalid total_time_seconds")
    ));
}

#[tokio::test]
async fn pricing_review_rejects_nan_total_time_seconds() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "replicate/nan-duration".to_string(),
        create_time_based_model_info("replicate"),
    );

    let result = service
        .calculate_completion_cost("replicate/nan-duration", 0, 0, None, None, Some(f64::NAN))
        .await;

    assert!(matches!(
        result,
        Err(GatewayError::Validation(message))
            if message.contains("replicate/nan-duration")
                && message.contains("Invalid total_time_seconds")
    ));
}

#[tokio::test]
async fn pricing_review_uses_token_pricing_for_token_priced_together_ai_model() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "together/token-priced".to_string(),
        create_test_model_info("together_ai"),
    );

    let result = service
        .calculate_completion_cost("together/token-priced", 1000, 500, None, None, None)
        .await;

    let result = match result {
        Ok(result) => result,
        Err(error) => panic!("expected token pricing success, got {error:?}"),
    };

    assert_eq!(result.cost_type, CostType::TokenBased);
    assert_eq!(result.input_cost, 1000.0 * 0.00001);
    assert_eq!(result.output_cost, 500.0 * 0.00003);
}

#[tokio::test]
async fn pricing_review_uses_token_pricing_for_token_priced_baseten_model() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "baseten/token-priced".to_string(),
        create_test_model_info("baseten"),
    );

    let result = service
        .calculate_completion_cost("baseten/token-priced", 2000, 750, None, None, None)
        .await;

    let result = match result {
        Ok(result) => result,
        Err(error) => panic!("expected token pricing success, got {error:?}"),
    };

    assert_eq!(result.cost_type, CostType::TokenBased);
    assert_eq!(result.input_cost, 2000.0 * 0.00001);
    assert_eq!(result.output_cost, 750.0 * 0.00003);
}

// ==================== Clone Tests ====================

#[test]
fn test_pricing_service_clone() {
    let service = PricingService::new(None);
    service.add_custom_model("gpt-4".to_string(), create_test_model_info("openai"));

    let cloned = service.clone();

    // Both should see the same data
    assert!(cloned.get_model_info("gpt-4").is_some());

    // Adding to original should be visible in clone (same Arc)
    service.add_custom_model("gpt-3.5".to_string(), create_test_model_info("openai"));
    assert!(cloned.get_model_info("gpt-3.5").is_some());
}

#[test]
fn custom_model_overwrite_moves_provider_index_atomically() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "shared-model".to_string(),
        create_test_model_info("provider_one"),
    );
    assert!(
        service
            .get_model_info_for_provider("provider_one", "shared-model")
            .is_some()
    );

    service.add_custom_model(
        "shared-model".to_string(),
        create_test_model_info("provider_two"),
    );

    assert!(
        service
            .get_model_info_for_provider("provider_one", "shared-model")
            .is_none(),
        "overwriting a key must remove its old provider ownership"
    );
    assert!(
        service
            .get_model_info_for_provider("provider_two", "shared-model")
            .is_some(),
        "overwriting a key must publish its new provider ownership"
    );
}

#[test]
fn pinned_pricing_snapshot_is_immutable_across_service_updates() {
    let service = PricingService::new(None);
    let mut first = create_test_model_info("openai");
    first.input_cost_per_token = Some(0.01);
    first.output_cost_per_token = Some(0.02);
    service.add_custom_model("snapshot-model".to_string(), first);

    let snapshot_a = service.snapshot();
    let usage = PricingUsage::new(10, 5);
    let cost_a = snapshot_a
        .calculate_loaded_usage_cost_for_provider("openai", "snapshot-model", &usage)
        .expect("snapshot A should price the model")
        .total_cost;

    let mut second = create_test_model_info("openai");
    second.input_cost_per_token = Some(0.10);
    second.output_cost_per_token = Some(0.20);
    service.add_custom_model("snapshot-model".to_string(), second);
    let snapshot_b = service.snapshot();

    assert_eq!(
        snapshot_a
            .calculate_loaded_usage_cost_for_provider("openai", "snapshot-model", &usage)
            .expect("snapshot A remains valid")
            .total_cost,
        cost_a,
    );
    assert_ne!(
        snapshot_b
            .calculate_loaded_usage_cost_for_provider("openai", "snapshot-model", &usage)
            .expect("snapshot B should see the replacement")
            .total_cost,
        cost_a,
    );
}

#[test]
fn failed_refresh_preserves_existing_snapshot_and_future_readers() {
    let service = PricingService::new(None);
    service.add_custom_model(
        "stable-snapshot-model".to_string(),
        create_test_model_info("stable_provider"),
    );
    let snapshot = service.snapshot();

    let replacement = create_test_model_info("replacement_provider");
    service.add_custom_model("replacement-model".to_string(), replacement);

    assert!(
        snapshot
            .get_model_info_for_provider("stable_provider", "stable-snapshot-model")
            .is_some()
    );
    assert!(
        snapshot
            .get_model_info_for_provider("replacement_provider", "replacement-model")
            .is_none(),
        "an existing reader must not observe a later published snapshot"
    );
}

#[tokio::test]
async fn failed_refresh_preserves_models_indexes_and_timestamp() {
    let path = std::env::temp_dir().join(format!(
        "litellm-pricing-invalid-{}-{}.json",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    std::fs::write(&path, "{ invalid json")
        .unwrap_or_else(|error| panic!("invalid pricing fixture should write: {error}"));

    let service = PricingService::new(Some(path.to_string_lossy().into_owned()));
    service.add_custom_model(
        "stable-model".to_string(),
        create_test_model_info("stable_provider"),
    );
    let before = service.get_statistics().last_updated;

    let error = service
        .refresh_pricing_data()
        .await
        .expect_err("invalid refresh must fail explicitly");
    assert!(error.to_string().contains("Failed to parse pricing JSON"));
    assert_eq!(service.get_statistics().last_updated, before);
    assert!(
        service
            .get_model_info_for_provider("stable_provider", "stable-model")
            .is_some(),
        "failed refresh must preserve both model and provider index"
    );

    let _ = std::fs::remove_file(path);
}
