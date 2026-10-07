//! Completed calls retain their actual database usage write after cancellation.

use super::*;
use crate::core::budget::{ProviderLimitConfig, ResetPeriod};
use crate::core::keys::{CreateKeyConfig, DatabaseKeyRepository};
use crate::storage::StorageLayer;
use sea_orm::TransactionTrait;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Copy)]
enum Case {
    Priced,
    RequestPricing,
    Unpriced,
    NoUsage,
}

async fn cancel_while_database_connection_is_busy(case: Case) {
    let directory = tempfile::tempdir().unwrap();
    let mut config = crate::config::models::storage::StorageConfig::default();
    // Disabled persistence selects an isolated in-memory SQLite pool with one
    // connection; holding its transaction makes the real usage write wait.
    config.database.enabled = false;
    config.redis.enabled = false;
    config.files.local_path = Some(directory.path().to_string_lossy().into_owned());
    let storage = Arc::new(StorageLayer::new(&config).await.unwrap());
    storage.migrate().await.unwrap();
    let keys = KeyManager::new(DatabaseKeyRepository::new(storage.clone()));
    let (key_id, _) = keys
        .generate_key(CreateKeyConfig {
            name: "cancelled-usage-fixture".into(),
            ..Default::default()
        })
        .await
        .unwrap();
    let limits = UnifiedBudgetLimits::new();
    limits.providers.set_provider_limit(
        "openai",
        ProviderLimitConfig::new(1.0, ResetPeriod::Monthly),
    );
    let reservation = limits
        .reserve_spend_async("openai", "gpt-4o", 0.25)
        .await
        .unwrap();
    let usage = Usage::new(6, 36);
    let pricing_usage = PricingUsage::new(6, 36);
    let pricing_config = serde_json::from_value(serde_json::json!({
        "unpriced_model_policy": "allow_unpriced",
        "unpriced_fallback_cost_per_1k_tokens": 1.0
    }))
    .unwrap();
    let request_pricing =
        RequestPricing::from_exact(default_spend_pricing_service(), "openai", "gpt-4o");
    let transaction = storage.db().connection().begin().await.unwrap();
    let mut completion = Box::pin(async {
        match case {
            Case::Priced => {
                record_completion_spend_with_reservation(usage_spend_settlement(
                    (&limits, &keys, Some(key_id)),
                    ("openai", "gpt-4o", Some(&usage)),
                    Some(reservation),
                    None,
                ))
                .await
            }
            Case::RequestPricing => {
                record_pricing_usage_spend_with_request_pricing(
                    &request_pricing,
                    &pricing_config,
                    &limits,
                    &keys,
                    Some(key_id),
                    "openai",
                    "gpt-4o",
                    &pricing_usage,
                    Some(reservation),
                    None,
                )
                .await
            }
            Case::Unpriced => {
                settle_unpriced_usage(
                    &pricing_config,
                    &limits,
                    &keys,
                    Some(key_id),
                    "openai",
                    "gpt-4o",
                    &pricing_usage,
                    Some(reservation),
                    None,
                    "cancelled fixture",
                )
                .await
            }
            Case::NoUsage => {
                record_reserved_spend_without_usage(
                    &keys,
                    Some(key_id),
                    "openai",
                    "gpt-4o",
                    Some(reservation),
                    None,
                    "cancelled fixture without usage",
                )
                .await
            }
        }
    });
    assert!(futures::poll!(completion.as_mut()).is_pending());
    tokio::task::yield_now().await;
    // Cancelling this waiter must not cancel the pending database write.
    drop(completion);
    transaction.commit().await.unwrap();
    let stats = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let stats = keys.get_usage_stats(key_id).await.unwrap();
            if stats.total_requests == 1 {
                break stats;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the real database usage write must outlive the cancelled request");
    assert_eq!(stats.total_requests, 1);
    assert_eq!(
        stats.total_tokens,
        if matches!(case, Case::NoUsage) { 0 } else { 42 }
    );
    assert!(stats.total_cost > 0.0);
    assert_eq!(
        stats.unpriced_requests,
        u64::from(matches!(case, Case::Unpriced))
    );
    if matches!(case, Case::Unpriced) {
        assert!((stats.total_cost - 0.042).abs() < f64::EPSILON);
        assert_eq!(stats.unpriced_tokens, 42);
    }
    if matches!(case, Case::NoUsage) {
        assert_eq!(stats.total_cost, 0.25);
    }
}

#[tokio::test]
async fn priced_usage_persists_after_cancellation() {
    cancel_while_database_connection_is_busy(Case::Priced).await;
}

#[tokio::test]
async fn request_pricing_usage_persists_after_cancellation() {
    cancel_while_database_connection_is_busy(Case::RequestPricing).await;
}

#[tokio::test]
async fn unpriced_usage_persists_after_cancellation() {
    cancel_while_database_connection_is_busy(Case::Unpriced).await;
}

#[tokio::test]
async fn reserved_usage_persists_after_cancellation() {
    cancel_while_database_connection_is_busy(Case::NoUsage).await;
}
