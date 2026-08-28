use super::*;
use crate::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
use crate::core::cost::calculator::{estimate_cost, generic_cost_per_token};
use crate::core::cost::types::UsageTokens;
use crate::core::keys::InMemoryKeyRepository;
use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::models::openai::{
    AudioContent, ChatMessage, ContentPart, Function, ImageUrl, MessageContent, MessageRole,
    ResponseFormat,
};
use crate::core::types::responses::Usage;

fn usage(prompt: u32, completion: u32) -> Usage {
    Usage {
        prompt_tokens: prompt,
        completion_tokens: completion,
        total_tokens: prompt + completion,
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: None,
    }
}

fn user_message(content: &str) -> ChatMessage {
    ChatMessage {
        thinking: None,
        role: MessageRole::User,
        content: Some(MessageContent::Text(content.to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }
}

fn chat_request(model: &str, messages: Vec<ChatMessage>) -> ChatCompletionRequest {
    ChatCompletionRequest {
        model: model.to_string(),
        messages,
        ..Default::default()
    }
}

#[tokio::test]
async fn records_provider_spend_for_priced_model() {
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let keys = KeyManager::new(InMemoryKeyRepository::new());

    record_completion_spend_with_reservation(usage_spend_settlement(
        (&budget, &keys, None),
        ("openai", "gpt-4o", Some(&usage(1000, 1000))),
        None,
        None,
    ))
    .await;

    let spent = budget
        .providers
        .get_provider_usage("openai")
        .map(|u| u.current_spend)
        .unwrap_or(0.0);
    assert!(spent > 0.0, "priced completion must record provider spend");
}

#[tokio::test]
async fn reserved_completion_settles_actual_spend_and_refunds_estimate() {
    let budget = UnifiedBudgetLimits::new();
    let estimate = estimate_cost("gpt-4o", "openai", 0, Some(100)).unwrap();
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(estimate.max_cost * 2.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "gpt-4o",
        ModelLimitConfig::new(estimate.max_cost * 2.0, ResetPeriod::Monthly),
    );
    let keys = KeyManager::new(InMemoryKeyRepository::new());
    let reservation = reserve_completion_budget(&budget, "openai", "gpt-4o", 0, Some(100))
        .unwrap()
        .unwrap();

    assert_eq!(
        budget
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        estimate.max_cost
    );

    record_completion_spend_with_reservation(usage_spend_settlement(
        (&budget, &keys, None),
        ("openai", "gpt-4o", Some(&usage(0, 50))),
        Some(reservation),
        None,
    ))
    .await;

    let expected = generic_cost_per_token("gpt-4o", &UsageTokens::new(0, 50), "openai")
        .unwrap()
        .total_cost;
    assert_eq!(
        budget
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        expected
    );
    assert_eq!(
        budget
            .models
            .get_model_usage("gpt-4o")
            .unwrap()
            .current_spend,
        expected
    );
}

#[tokio::test]
async fn reserved_completion_records_actual_when_usage_exceeds_estimate() {
    let budget = UnifiedBudgetLimits::new();
    let actual_cost = generic_cost_per_token("gpt-4o", &UsageTokens::new(0, 100), "openai")
        .unwrap()
        .total_cost;
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(actual_cost * 2.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "gpt-4o",
        ModelLimitConfig::new(actual_cost * 2.0, ResetPeriod::Monthly),
    );
    let keys = KeyManager::new(InMemoryKeyRepository::new());
    let reservation = reserve_completion_budget(&budget, "openai", "gpt-4o", 0, Some(1))
        .unwrap()
        .unwrap();

    record_completion_spend_with_reservation(usage_spend_settlement(
        (&budget, &keys, None),
        ("openai", "gpt-4o", Some(&usage(0, 100))),
        Some(reservation),
        None,
    ))
    .await;

    assert_eq!(
        budget
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        actual_cost
    );
    assert_eq!(
        budget
            .models
            .get_model_usage("gpt-4o")
            .unwrap()
            .current_spend,
        actual_cost
    );
}

#[test]
fn chat_reservation_without_max_tokens_uses_conservative_output_bound() {
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "gpt-4o",
        ModelLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let messages = vec![user_message("hello")];
    let prompt_tokens = estimate_chat_prompt_tokens("gpt-4o", &messages, None, None, None, None);
    let default_estimate = estimate_cost("gpt-4o", "openai", prompt_tokens, Some(100))
        .unwrap()
        .max_cost;

    let request = chat_request("gpt-4o", messages.clone());
    let reservation = reserve_chat_completion_budget(&budget, "openai", "gpt-4o", &request)
        .unwrap()
        .unwrap();
    let reserved = reservation.reserved_amount();

    assert!(reserved > default_estimate);
    reservation.cancel();
}

#[test]
fn chat_reservation_without_max_tokens_uses_catalog_output_limit() {
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let messages = vec![user_message(&"x".repeat(40_000))];
    let prompt_tokens = estimate_chat_prompt_tokens("gpt-4o", &messages, None, None, None, None);
    let input_only = estimate_cost("gpt-4o", "openai", prompt_tokens, Some(0))
        .unwrap()
        .max_cost;

    let request = chat_request("gpt-4o", messages.clone());
    let reservation = reserve_chat_completion_budget(&budget, "openai", "gpt-4o", &request)
        .unwrap()
        .unwrap();
    let reserved = reservation.reserved_amount();

    assert!(reserved > input_only);
    reservation.cancel();
}

#[test]
fn chat_reservation_with_explicit_max_tokens_reserves_requested_output() {
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let messages = vec![user_message(&"x".repeat(40_000))];
    let prompt_tokens = estimate_chat_prompt_tokens("gpt-4o", &messages, None, None, None, None);
    let input_only = estimate_cost("gpt-4o", "openai", prompt_tokens, Some(0))
        .unwrap()
        .max_cost;

    let mut request = chat_request("gpt-4o", messages.clone());
    request.max_tokens = Some(100);
    let reservation = reserve_chat_completion_budget(&budget, "openai", "gpt-4o", &request)
        .unwrap()
        .unwrap();
    let reserved = reservation.reserved_amount();

    assert!(reserved > input_only);
    reservation.cancel();
}

#[test]
fn chat_reservation_includes_legacy_functions() {
    let without_functions = UnifiedBudgetLimits::new();
    without_functions.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let with_functions = UnifiedBudgetLimits::new();
    with_functions.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let messages = vec![user_message("hello")];
    let functions = vec![Function {
        name: "lookup_customer_profile".to_string(),
        description: Some("Return a complete customer profile".to_string()),
        parameters: Some(serde_json::json!({
            "type": "object",
            "properties": {
                "customer_id": {
                    "type": "string",
                    "description": "A long-form external customer identifier"
                },
                "include_history": {
                    "type": "boolean",
                    "description": "Whether to include historical transactions"
                }
            },
            "required": ["customer_id"]
        })),
    }];

    let mut baseline_request = chat_request("gpt-4o", messages.clone());
    baseline_request.max_tokens = Some(100);
    let mut function_request = chat_request("gpt-4o", messages.clone());
    function_request.functions = Some(functions);
    function_request.max_tokens = Some(100);
    let baseline =
        reserve_chat_completion_budget(&without_functions, "openai", "gpt-4o", &baseline_request)
            .unwrap()
            .unwrap();
    let function_reserved =
        reserve_chat_completion_budget(&with_functions, "openai", "gpt-4o", &function_request)
            .unwrap()
            .unwrap();

    assert!(function_reserved.reserved_amount() > baseline.reserved_amount());
    baseline.cancel();
    function_reserved.cancel();
}

#[test]
fn chat_reservation_includes_response_format_schema() {
    let without_schema = UnifiedBudgetLimits::new();
    without_schema.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let with_schema = UnifiedBudgetLimits::new();
    with_schema.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let messages = vec![user_message("return structured JSON")];
    let response_format = ResponseFormat {
        format_type: "json_schema".to_string(),
        json_schema: Some(serde_json::json!({
            "name": "customer_profile",
            "schema": {
                "type": "object",
                "properties": {
                    "customer_id": {
                        "type": "string",
                        "description": "A long-form external customer identifier"
                    },
                    "risk_notes": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "description": "Detailed compliance and billing annotations"
                        }
                    }
                },
                "required": ["customer_id", "risk_notes"]
            }
        })),
        response_type: None,
    };

    let mut baseline_request = chat_request("gpt-4o", messages.clone());
    baseline_request.max_tokens = Some(100);
    let mut schema_request = chat_request("gpt-4o", messages.clone());
    schema_request.response_format = Some(response_format);
    schema_request.max_tokens = Some(100);
    let baseline =
        reserve_chat_completion_budget(&without_schema, "openai", "gpt-4o", &baseline_request)
            .unwrap()
            .unwrap();
    let schema_reserved =
        reserve_chat_completion_budget(&with_schema, "openai", "gpt-4o", &schema_request)
            .unwrap()
            .unwrap();

    assert!(schema_reserved.reserved_amount() > baseline.reserved_amount());
    baseline.cancel();
    schema_reserved.cancel();
}

#[test]
fn chat_reservation_multiplies_output_budget_by_choice_count() {
    let single_budget = UnifiedBudgetLimits::new();
    single_budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let multi_budget = UnifiedBudgetLimits::new();
    multi_budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let messages = vec![user_message("hello")];

    let mut single_request = chat_request("gpt-4o", messages.clone());
    single_request.max_tokens = Some(100);
    let mut multi_request = chat_request("gpt-4o", messages);
    multi_request.max_tokens = Some(100);
    multi_request.n = Some(3);
    let single =
        reserve_chat_completion_budget(&single_budget, "openai", "gpt-4o", &single_request)
            .unwrap()
            .unwrap();
    let multi = reserve_chat_completion_budget(&multi_budget, "openai", "gpt-4o", &multi_request)
        .unwrap()
        .unwrap();

    assert!(multi.reserved_amount() > single.reserved_amount());
    single.cancel();
    multi.cancel();
}

#[test]
fn chat_reservation_uses_conservative_multimodal_prompt_floor() {
    let messages = vec![ChatMessage {
        thinking: None,
        role: MessageRole::User,
        content: Some(MessageContent::Parts(vec![ContentPart::ImageUrl {
            image_url: ImageUrl {
                url: "https://example.test/image.png".to_string(),
                detail: Some("high".to_string()),
            },
        }])),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }];

    let prompt_tokens = estimate_chat_prompt_tokens("gpt-4o", &messages, None, None, None, None);
    assert!(prompt_tokens >= IMAGE_HIGH_DETAIL_PROMPT_TOKENS);
}

#[test]
fn chat_reservation_uses_encoded_audio_prompt_floor() {
    let encoded_audio = "a".repeat(8_000);
    let messages = vec![ChatMessage {
        thinking: None,
        role: MessageRole::User,
        content: Some(MessageContent::Parts(vec![ContentPart::Audio {
            audio: AudioContent {
                data: encoded_audio,
                format: "wav".to_string(),
            },
        }])),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }];

    let prompt_tokens = estimate_chat_prompt_tokens("gpt-4o", &messages, None, None, None, None);
    assert!(prompt_tokens >= 2_000);
}

#[test]
fn catalog_output_limit_matches_dated_model_alias() {
    let base = catalog_max_output_tokens("openai", "gpt-4o");
    assert!(
        base.is_some(),
        "base catalog model should have output limit"
    );
    assert_eq!(
        catalog_max_output_tokens("openai", "gpt-4o-2024-08-06"),
        base
    );
}

#[test]
fn catalog_output_limit_matches_pricing_provider_alias() {
    let vertex_ai = catalog_max_output_tokens("vertex_ai", "gemini-3.1-flash-lite");
    assert!(
        vertex_ai.is_some(),
        "shared Gemini catalog model should have output limit"
    );
    assert_eq!(
        catalog_max_output_tokens("gemini", "gemini-3.1-flash-lite"),
        vertex_ai
    );
}

#[test]
fn catalog_output_limit_matches_anthropic_compatible_mimo_alias() {
    assert_eq!(
        catalog_max_output_tokens("anthropic", "mimo-v2.5-pro"),
        Some(131_072)
    );
}

#[test]
fn catalog_output_limit_matches_zhipu_provider_alias() {
    assert_eq!(catalog_max_output_tokens("zhipuai", "glm-4"), Some(4096));
}

#[test]
fn gemini_max_completion_tokens_only_uses_catalog_output_bound() {
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "gemini",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let messages = vec![user_message("hello")];
    let prompt_tokens =
        estimate_chat_prompt_tokens("gemini-3.1-flash-lite", &messages, None, None, None, None);
    let ten_output_tokens =
        estimate_cost("gemini-3.1-flash-lite", "gemini", prompt_tokens, Some(10))
            .unwrap()
            .max_cost;

    let mut request = chat_request("gemini-3.1-flash-lite", messages);
    request.max_completion_tokens = Some(10);
    let reservation =
        reserve_chat_completion_budget(&budget, "gemini", "gemini-3.1-flash-lite", &request)
            .unwrap()
            .unwrap();

    assert!(reservation.reserved_amount() > ten_output_tokens);
    reservation.cancel();
}

#[test]
fn cohere_max_completion_tokens_reserves_provider_effective_output() {
    let budget = UnifiedBudgetLimits::new();
    let messages = vec![user_message("hello")];
    let prompt_tokens =
        estimate_chat_prompt_tokens("command-r-plus", &messages, None, None, None, None);
    let ten_output_tokens = estimate_cost("command-r-plus", "cohere", prompt_tokens, Some(10))
        .unwrap()
        .max_cost;
    let catalog_output_tokens =
        estimate_cost("command-r-plus", "cohere", prompt_tokens, Some(4096))
            .unwrap()
            .max_cost;
    budget.providers.set_provider_limit(
        "cohere",
        ProviderLimitConfig::new(ten_output_tokens * 1.1, ResetPeriod::Monthly),
    );

    let mut request = chat_request("command-r-plus", messages);
    request.max_completion_tokens = Some(10);
    let reservation = reserve_chat_completion_budget(&budget, "cohere", "command-r-plus", &request)
        .unwrap()
        .unwrap();

    assert!(catalog_output_tokens > ten_output_tokens * 100.0);
    assert!((reservation.reserved_amount() - ten_output_tokens).abs() < f64::EPSILON);
    reservation.cancel();
}

#[test]
fn provider_effective_max_output_tokens_tracks_adapter_precedence() {
    let mut request = chat_request("amazon.nova-2-lite-v1:0", vec![user_message("hello")]);
    request.max_tokens = Some(100);
    request.max_completion_tokens = Some(10);

    assert_eq!(
        provider_effective_max_output_tokens("amazon_nova", "amazon.nova-2-lite-v1:0", &request),
        Some(10)
    );
    assert_eq!(
        provider_effective_max_output_tokens("bedrock", "amazon.nova-2-lite-v1:0", &request),
        Some(10)
    );
    assert_eq!(
        provider_effective_max_output_tokens("cohere", "command-r-plus", &request),
        Some(100)
    );
    assert_eq!(
        provider_effective_max_output_tokens("gemini", "gemini-3.1-flash-lite", &request),
        Some(100)
    );
}

#[test]
fn bedrock_invoke_models_ignore_max_completion_tokens_for_reservation_cap() {
    let mut request = chat_request("amazon.titan-text-express-v1", vec![user_message("hello")]);
    request.max_completion_tokens = Some(10);

    assert_eq!(
        provider_effective_max_output_tokens("bedrock", "amazon.titan-text-express-v1", &request),
        None
    );

    request.max_tokens = Some(100);
    assert_eq!(
        provider_effective_max_output_tokens("bedrock", "amazon.titan-text-express-v1", &request),
        Some(100)
    );
}

#[tokio::test]
async fn reservation_settlement_after_reset_records_actual_spend() {
    let budget = UnifiedBudgetLimits::new();
    let estimate = estimate_cost("gpt-4o", "openai", 0, Some(100)).unwrap();
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(estimate.max_cost * 2.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "gpt-4o",
        ModelLimitConfig::new(estimate.max_cost * 2.0, ResetPeriod::Monthly),
    );
    let keys = KeyManager::new(InMemoryKeyRepository::new());
    let reservation = reserve_completion_budget(&budget, "openai", "gpt-4o", 0, Some(100))
        .unwrap()
        .unwrap();
    assert!(budget.providers.reset_provider_budget("openai"));

    record_completion_spend_with_reservation(usage_spend_settlement(
        (&budget, &keys, None),
        ("openai", "gpt-4o", Some(&usage(0, 50))),
        Some(reservation),
        None,
    ))
    .await;

    let expected = generic_cost_per_token("gpt-4o", &UsageTokens::new(0, 50), "openai")
        .unwrap()
        .total_cost;
    assert_eq!(
        budget
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        expected
    );
    assert_eq!(
        budget
            .models
            .get_model_usage("gpt-4o")
            .unwrap()
            .current_spend,
        expected
    );
}

#[tokio::test]
async fn stream_disconnect_without_usage_settles_reserved_budget() {
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "gpt-4o",
        ModelLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let keys = KeyManager::new(InMemoryKeyRepository::new());
    let reservation = reserve_completion_budget(&budget, "openai", "gpt-4o", 0, Some(100))
        .unwrap()
        .unwrap();
    let reserved = reservation.reserved_amount();

    record_stream_disconnect_spend_with_reservation(usage_spend_settlement(
        (&budget, &keys, None),
        ("openai", "gpt-4o", None),
        Some(reservation),
        None,
    ))
    .await;

    assert_eq!(
        budget
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        reserved
    );
    assert_eq!(
        budget
            .models
            .get_model_usage("gpt-4o")
            .unwrap()
            .current_spend,
        reserved
    );
}

#[test]
fn concurrent_completion_reservations_allow_one_last_budget_winner() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;

    let estimate = estimate_cost("gpt-4o", "openai", 0, Some(100)).unwrap();
    let budget = Arc::new(UnifiedBudgetLimits::new());
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(estimate.max_cost, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "gpt-4o",
        ModelLimitConfig::new(estimate.max_cost, ResetPeriod::Monthly),
    );
    let barrier = Arc::new(Barrier::new(8));
    let winners = Arc::new(AtomicUsize::new(0));
    let reservations = Arc::new(Mutex::new(Vec::new()));

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let budget = Arc::clone(&budget);
            let barrier = Arc::clone(&barrier);
            let winners = Arc::clone(&winners);
            let reservations = Arc::clone(&reservations);
            thread::spawn(move || {
                barrier.wait();
                if let Ok(Some(reservation)) =
                    reserve_completion_budget(&budget, "openai", "gpt-4o", 0, Some(100))
                {
                    winners.fetch_add(1, Ordering::SeqCst);
                    reservations.lock().unwrap().push(reservation);
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(winners.load(Ordering::SeqCst), 1);
    assert_eq!(
        budget
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        estimate.max_cost
    );
    reservations.lock().unwrap().clear();
    assert_eq!(
        budget
            .providers
            .get_provider_usage("openai")
            .unwrap()
            .current_spend,
        0.0
    );
}

#[tokio::test]
async fn unpriced_model_records_no_budget_spend() {
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let keys = KeyManager::new(InMemoryKeyRepository::new());

    // Unknown model has no pricing: budget spend must stay at 0 rather than
    // being booked at a fabricated $0 cost silently.
    record_completion_spend_with_reservation(usage_spend_settlement(
        (&budget, &keys, None),
        (
            "openai",
            "definitely-not-a-real-model-xyz",
            Some(&usage(1000, 1000)),
        ),
        None,
        None,
    ))
    .await;

    let spent = budget
        .providers
        .get_provider_usage("openai")
        .map(|u| u.current_spend)
        .unwrap_or(0.0);
    assert_eq!(spent, 0.0);
}

#[test]
fn budget_available_when_unconfigured() {
    // No limits set: precheck must allow the request through.
    let budget = UnifiedBudgetLimits::new();
    assert!(ensure_budget_available(&budget, "openai", "gpt-4o").is_ok());
}

#[test]
fn budget_rejects_when_provider_exhausted() {
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    // Drive the provider over its limit.
    budget.providers.record_provider_spend("openai", 2.0);

    let err = ensure_budget_available(&budget, "openai", "gpt-4o")
        .expect_err("exhausted provider budget must be rejected");
    assert!(matches!(err, ProviderError::QuotaExceeded { .. }));
}
