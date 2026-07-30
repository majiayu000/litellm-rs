use super::*;
use crate::core::budget::{ModelLimitConfig, ProviderLimitConfig, ResetPeriod};
use crate::core::keys::{CreateKeyConfig, InMemoryKeyRepository};
use crate::core::types::responses::{ChatDelta, ChatStreamChoice, FinishReason};

#[test]
fn empty_internal_stream_sentinel_is_not_candidate_output() {
    let mut chunk = ChatChunk {
        id: "stream-sentinel".to_string(),
        object: "chat.completion.chunk".to_string(),
        created: 0,
        model: "gemini-test".to_string(),
        choices: vec![],
        usage: None,
        system_fingerprint: None,
    };
    assert!(!stream_chunk_has_candidate_output(&chunk));

    let finish_only = ChatStreamChoice {
        index: 0,
        delta: ChatDelta::default(),
        finish_reason: Some(FinishReason::Stop),
        logprobs: None,
    };
    chunk.choices.push(finish_only);
    assert!(stream_chunk_has_candidate_output(&chunk));

    chunk.choices[0].finish_reason = None;
    chunk.choices[0].delta.content = Some("candidate output".to_string());
    assert!(stream_chunk_has_candidate_output(&chunk));
}

#[tokio::test]
async fn stream_disconnect_without_usage_records_reserved_key_cost() {
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
    let (key_id, _) = keys
        .generate_key(CreateKeyConfig {
            name: "stream key".to_string(),
            ..Default::default()
        })
        .await
        .expect("test key should be created");
    let reservation = reserve_completion_budget(&budget, "openai", "gpt-4o", 0, Some(100))
        .expect("reservation should succeed")
        .expect("priced model should reserve budget");
    let reserved = reservation.reserved_amount();

    record_stream_disconnect_spend_with_reservation(usage_spend_settlement(
        (&budget, &keys, Some(key_id)),
        ("openai", "gpt-4o", None),
        Some(reservation),
        None,
    ))
    .await;

    let stats = keys
        .get_usage_stats(key_id)
        .await
        .expect("usage stats should be readable");
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.total_tokens, 0);
    assert_eq!(stats.total_cost, reserved);
}

#[tokio::test]
async fn finished_stream_without_usage_records_reserved_key_cost_after_output() {
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
    let (key_id, _) = keys
        .generate_key(CreateKeyConfig {
            name: "finished stream key".to_string(),
            ..Default::default()
        })
        .await
        .expect("test key should be created");
    let reservation = reserve_completion_budget(&budget, "openai", "gpt-4o", 0, Some(100))
        .expect("reservation should succeed")
        .expect("priced model should reserve budget");
    let reserved = reservation.reserved_amount();

    record_finished_stream_spend_with_reservation(StreamSpendSettlement {
        budget_limits: &budget,
        key_manager: &keys,
        api_key_id: Some(key_id),
        provider: "openai",
        model: "gpt-4o",
        pricing_provider: "openai",
        pricing_model: "gpt-4o",
        usage: None,
        saw_upstream_output: true,
        budget_reservation: Some(reservation),
        key_budget_reservation: None,
    })
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
    let stats = keys
        .get_usage_stats(key_id)
        .await
        .expect("usage stats should be readable");
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.total_tokens, 0);
    assert_eq!(stats.total_cost, reserved);
}
