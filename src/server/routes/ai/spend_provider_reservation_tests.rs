use super::*;
use crate::core::budget::{ProviderLimitConfig, ResetPeriod};
use crate::core::cost::calculator::estimate_cost;
use crate::core::models::openai::requests::ChatCompletionRequest;
use crate::core::models::openai::{ChatMessage, ContentPart, MessageContent, MessageRole};
use crate::utils::ai::counter::token_counter::TokenizerIdentity;

fn reserve_with_provider_limit(
    provider: &str,
    model: &str,
    max_output_tokens: u32,
) -> UnifiedBudgetReservation {
    let budget = UnifiedBudgetLimits::new();
    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text("hello".to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }];
    let token_identity = if provider == "openai" {
        TokenizerIdentity::exact_openai(model)
    } else {
        TokenizerIdentity::approximate(provider, model)
    };
    let prompt_tokens =
        estimate_chat_prompt_tokens(&token_identity, &messages, None, None, None, None);
    let estimate = estimate_cost(model, provider, prompt_tokens, Some(max_output_tokens)).unwrap();
    budget.providers.set_provider_limit(
        provider,
        ProviderLimitConfig::new(estimate.max_cost * 2.0, ResetPeriod::Monthly),
    );

    let mut request = ChatCompletionRequest {
        model: model.to_string(),
        messages,
        ..Default::default()
    };
    request.max_completion_tokens = Some(max_output_tokens);
    if provider == "bedrock" {
        request.max_tokens = Some(max_output_tokens);
    }

    let reservation = reserve_chat_completion_budget(&budget, provider, model, &request)
        .unwrap()
        .unwrap();
    assert!((reservation.reserved_amount() - estimate.max_cost).abs() < f64::EPSILON);
    reservation
}

#[test]
fn bedrock_chat_reservation_uses_bedrock_cost_pricing() {
    let reservation = reserve_with_provider_limit("bedrock", "amazon.titan-text-express-v1", 100);
    reservation.cancel();
}

#[test]
fn amazon_nova_chat_reservation_uses_provider_pricing() {
    let reservation = reserve_with_provider_limit("amazon_nova", "amazon.nova-2-lite-v1:0", 10);
    reservation.cancel();
}

#[test]
fn openai_like_prefixed_chat_reservation_uses_provider_pricing() {
    let reservation =
        reserve_with_provider_limit("openai_like", "groq/llama-3.3-70b-versatile", 100);
    reservation.cancel();
}

#[test]
fn openai_chat_reservation_uses_exact_tiktoken_prompt_count() {
    let budget = UnifiedBudgetLimits::new();
    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text("Hello, how are you?".to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }];
    let prompt_tokens = estimate_chat_prompt_tokens(
        &TokenizerIdentity::exact_openai("gpt-3.5-turbo"),
        &messages,
        None,
        None,
        None,
        None,
    );
    assert_eq!(prompt_tokens, 13);

    let expected = estimate_cost("gpt-3.5-turbo", "openai", prompt_tokens, Some(10))
        .unwrap()
        .max_cost;
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(expected * 2.0, ResetPeriod::Monthly),
    );
    let mut request = ChatCompletionRequest {
        model: "gpt-3.5-turbo".to_string(),
        messages,
        ..Default::default()
    };
    request.max_tokens = Some(10);

    let reservation = reserve_chat_completion_budget(&budget, "openai", "gpt-3.5-turbo", &request)
        .unwrap()
        .unwrap();

    assert!((reservation.reserved_amount() - expected).abs() < f64::EPSILON);
    reservation.cancel();
}

#[test]
fn chat_prompt_estimate_accounts_for_serialized_tool_parts() {
    let payload = "x".repeat(4_000);
    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Parts(vec![
            ContentPart::ToolResult {
                tool_use_id: "call_1".to_string(),
                content: serde_json::json!({ "payload": payload }),
                is_error: None,
            },
            ContentPart::ToolUse {
                id: "call_2".to_string(),
                name: "lookup".to_string(),
                input: serde_json::json!({ "payload": "y".repeat(4_000) }),
            },
        ])),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }];

    let prompt_tokens = estimate_chat_prompt_tokens(
        &TokenizerIdentity::exact_openai("gpt-4o"),
        &messages,
        None,
        None,
        None,
        None,
    );

    assert!(
        prompt_tokens > 1_800,
        "large tool payloads should add a serialized prompt floor"
    );
}
