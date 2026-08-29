use super::*;
use crate::config::models::gateway::GatewayPricingConfig;
use crate::core::budget::{
    ModelLimitConfig, ProviderLimitConfig, ResetPeriod, UnifiedBudgetLimits,
};
use crate::core::keys::{InMemoryKeyRepository, KeyManager};
use crate::core::pricing_service::PricingService;
use std::sync::Arc;

fn test_pricing_service() -> Arc<PricingService> {
    match PricingService::with_embedded_default() {
        Ok(service) => Arc::new(service),
        Err(error) => panic!("embedded pricing service should initialize for tests: {error}"),
    }
}

#[test]
fn test_sse_error_contains_done() {
    let b = sse_error("oops", "server_error", "internal_error");
    let s = String::from_utf8(b.to_vec()).unwrap();
    assert!(s.contains("data: {"));
    assert!(s.contains("[DONE]"));
    assert!(s.contains("oops"));
}

#[test]
fn test_classify_auth_error() {
    let e = ProviderError::Authentication {
        provider: "openai",
        message: "bad key".to_string(),
    };
    let (t, c) = classify(&e);
    assert_eq!(t, "invalid_request_error");
    assert_eq!(c, "authentication_error");
}

#[test]
fn test_classify_timeout() {
    let e = ProviderError::Timeout {
        provider: "openai",
        message: "timed out".to_string(),
    };
    let (t, c) = classify(&e);
    assert_eq!(t, "server_error");
    assert_eq!(c, "timeout");
}

#[test]
fn test_completed_response_output_contains_reasoning_item() {
    let added = serde_json::to_value(ResponseStreamEvent::ResponseOutputItemAdded {
        output_index: 0,
        item: in_progress_reasoning_item("rs_test".to_string()),
    })
    .unwrap();
    let reasoning = completed_reasoning_item(
        "rs_test".to_string(),
        "completed",
        "checked constraints".to_string(),
    );
    let message = ResponseOutputItem::Message(ResponseOutputMessage {
        id: "msg_test".to_string(),
        role: "assistant".to_string(),
        status: "completed".to_string(),
        content: vec![ResponseOutputContent::OutputText {
            text: "final answer".to_string(),
            annotations: None,
            logprobs: None,
        }],
    });
    let output = output_items_in_stream_order(vec![(1, message), (0, reasoning)]);
    let completed = ResponsesApiResponse {
        id: "resp_test".to_string(),
        object: "response".to_string(),
        created_at: 1,
        status: "completed".to_string(),
        model: "gpt-test".to_string(),
        output,
        usage: None,
        error: None,
        previous_response_id: None,
        metadata: None,
    };

    let event = serde_json::to_value(ResponseStreamEvent::ResponseCompleted {
        response: Box::new(completed),
    })
    .unwrap();

    assert_eq!(added["type"], "response.output_item.added");
    assert_eq!(added["output_index"], 0);
    assert_eq!(added["item"]["type"], "reasoning");
    assert_eq!(added["item"]["id"], "rs_test");
    assert_eq!(added["item"]["status"], "in_progress");

    assert_eq!(event["type"], "response.completed");
    assert_eq!(event["response"]["output"][0]["type"], "reasoning");
    assert_eq!(event["response"]["output"][0]["id"], "rs_test");
    assert_eq!(
        event["response"]["output"][0]["summary"][0]["type"],
        "summary_text"
    );
    assert_eq!(
        event["response"]["output"][0]["summary"][0]["text"],
        "checked constraints"
    );
    assert_eq!(event["response"]["output"][1]["type"], "message");
}

#[test]
fn codex_custom_tool_stream_events_are_ordered_and_lossless() {
    let mut state = ToolCallAccum::new("ct_1".into(), "call_1".into(), "shell".into(), 2, true);
    state.arguments = r#"{"input":"echo hello"}"#.into();
    let added = serde_json::to_value(ResponseStreamEvent::ResponseOutputItemAdded {
        output_index: 2,
        item: state.output_item("in_progress"),
    })
    .unwrap();
    let events = state
        .done_events()
        .into_iter()
        .map(|event| serde_json::to_value(event).unwrap())
        .collect::<Vec<_>>();
    let done = serde_json::to_value(ResponseStreamEvent::ResponseOutputItemDone {
        output_index: 2,
        item: state.output_item("completed"),
    })
    .unwrap();

    assert_eq!(added["item"]["type"], "custom_tool_call");
    assert_eq!(events[0]["type"], "response.custom_tool_call_input.delta");
    assert_eq!(events[0]["item_id"], "ct_1");
    assert_eq!(events[0]["delta"], "echo hello");
    assert_eq!(events[1]["type"], "response.custom_tool_call_input.done");
    assert_eq!(events[1]["input"], "echo hello");
    assert_eq!(done["item"]["call_id"], "call_1");
    assert_eq!(done["item"]["input"], "echo hello");
}

#[test]
fn function_call_stream_events_use_response_item_id() {
    let mut state = ToolCallAccum::new("fc_1".into(), "call_1".into(), "lookup".into(), 0, false);
    state.arguments = "{}".into();
    let delta = serde_json::to_value(state.delta_event("{".into()).unwrap()).unwrap();
    let done = serde_json::to_value(state.done_events().remove(0)).unwrap();
    assert_eq!(delta["item_id"], "fc_1");
    assert!(delta.get("call_id").is_none());
    assert_eq!(done["item_id"], "fc_1");
    assert_eq!(done["name"], "lookup");
}

#[test]
fn test_response_usage_from_chat_usage_preserves_details() {
    let usage = ChatUsage {
        prompt_tokens: 100,
        completion_tokens: 40,
        total_tokens: 140,
        prompt_tokens_details: Some(crate::core::types::responses::PromptTokensDetails {
            cached_tokens: Some(25),
            cache_creation_tokens: Some(5),
            cache_read_tokens: Some(20),
            audio_tokens: Some(7),
        }),
        completion_tokens_details: Some(crate::core::types::responses::CompletionTokensDetails {
            reasoning_tokens: Some(11),
            audio_tokens: Some(3),
        }),
        thinking_usage: None,
    };

    let response_usage = response_usage_from_chat_usage(&usage);

    assert_eq!(response_usage.input_tokens, 100);
    assert_eq!(response_usage.output_tokens, 40);
    assert_eq!(response_usage.total_tokens, 140);
    assert_eq!(
        response_usage.input_tokens_details.unwrap().cached_tokens,
        25
    );
    assert_eq!(
        response_usage
            .output_tokens_details
            .unwrap()
            .reasoning_tokens,
        11
    );
}

#[test]
fn response_stream_total_tokens_preserves_provider_saturation() {
    let usage = ChatUsage {
        prompt_tokens: u32::MAX,
        completion_tokens: 1,
        total_tokens: u32::MAX,
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: None,
    };

    assert_eq!(
        response_stream_total_tokens(Some(&usage), usage.prompt_tokens, usage.completion_tokens),
        u32::MAX
    );
    assert_eq!(response_stream_total_tokens(None, u32::MAX, 1), u32::MAX);
}

#[tokio::test]
async fn disconnect_after_upstream_output_settles_reserved_budget() {
    let budget = Arc::new(UnifiedBudgetLimits::new());
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "gpt-4o",
        ModelLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let reservation =
        spend::reserve_completion_budget(budget.as_ref(), "openai", "gpt-4o", 0, Some(100))
            .unwrap()
            .unwrap();
    let reserved = reservation.reserved_amount();
    let mut settlement = StreamBudgetSettlement {
        pricing_service: test_pricing_service(),
        pricing_config: GatewayPricingConfig::default(),
        budget_limits: Arc::clone(&budget),
        key_manager: KeyManager::new(InMemoryKeyRepository::new()),
        api_key_id: None,
        provider: "openai".to_string(),
        model: "gpt-4o".to_string(),
        pricing_provider: "openai".to_string(),
        pricing_model: "gpt-4o".to_string(),
        budget_reservation: Some(reservation),
        key_budget_reservation: None,
    };

    settlement.record_disconnect(None).await;

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

#[tokio::test]
async fn completed_stream_without_usage_after_output_settles_reserved_budget() {
    let budget = Arc::new(UnifiedBudgetLimits::new());
    budget.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    budget.models.set_model_limit(
        "gpt-4o",
        ModelLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let reservation =
        spend::reserve_completion_budget(budget.as_ref(), "openai", "gpt-4o", 0, Some(100))
            .unwrap()
            .unwrap();
    let reserved = reservation.reserved_amount();
    let settlement = StreamBudgetSettlement {
        pricing_service: test_pricing_service(),
        pricing_config: GatewayPricingConfig::default(),
        budget_limits: Arc::clone(&budget),
        key_manager: KeyManager::new(InMemoryKeyRepository::new()),
        api_key_id: None,
        provider: "openai".to_string(),
        model: "gpt-4o".to_string(),
        pricing_provider: "openai".to_string(),
        pricing_model: "gpt-4o".to_string(),
        budget_reservation: Some(reservation),
        key_budget_reservation: None,
    };

    settlement.record_completion(None, true).await;

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
