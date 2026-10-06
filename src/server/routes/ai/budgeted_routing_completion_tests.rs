//! A real dollar-budget Redis wait must not erase the selected route outcome.

use super::*;
use crate::core::keys::{InMemoryKeyRepository, KeyManager};
use crate::core::providers::Provider;
use crate::core::providers::openai::OpenAIProvider;
use crate::core::router::{Deployment, DeploymentConfig, UnifiedRouter};
use crate::core::types::model::ProviderCapability;
use crate::core::types::responses::Usage;
use crate::server::routes::ai::{execution, spend};

#[derive(Clone, Copy, PartialEq, Eq)]
enum CompletionCase {
    Cancel,
    CancelUnpriced,
    Complete,
    ReplaceThenCancel,
    LocalConversionError,
    CancelLocalConversionError,
}

async fn route_budget_completion(case: CompletionCase) {
    let _serial = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    // This connection bypasses the dollar-budget proxy. We deliberately hold
    // only the new budget wait, not #1465's routing-admission I/O.
    let route_pool = Arc::new(
        RedisPool::new(&RedisConfig {
            url: std::env::var("REDIS_URL").unwrap(),
            enabled: true,
            allow_degraded: false,
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let route_pool = if case == CompletionCase::CancelLocalConversionError {
        fixture.pool.clone()
    } else {
        route_pool
    };
    let router = UnifiedRouter::default().with_admission_redis(route_pool.clone());
    let id = uuid::Uuid::new_v4().to_string();
    let provider = Provider::OpenAI(OpenAIProvider::with_api_key("sk-test-key").await.unwrap());
    let deployment = || {
        Deployment::new(
            id.clone(),
            provider.clone(),
            "gpt-4o-mini".into(),
            "joint-route".into(),
        )
        .with_config(DeploymentConfig {
            max_parallel_requests: Some(1),
            ..Default::default()
        })
    };
    router.add_deployment(deployment());
    let original = router.get_deployment(&id).unwrap();
    let limits = fixture.limits.clone();
    let budget_provider = fixture.provider.clone();
    let budget_model = fixture.model.clone();
    let gate = fixture.gate.clone();

    let mut request = Box::pin(execution::execute_with_selected_deployment(
        &router,
        "joint-route",
        ProviderCapability::ChatCompletion,
        move |_, _, _| {
            let call = BudgetedCall::new(
                limits.clone(),
                budget_provider.clone(),
                budget_model.clone(),
            );
            let gate = gate.clone();
            async move {
                let settlement_gate = gate.clone();
                let result = call
                    .reserve_call_settle(
                        async |budget| budget.reserve_spend(1.0).await.map(Some),
                        || async move {
                            // The provider contract has returned a complete, valid
                            // response. The next Redis reply is its dollar settlement.
                            assert!(!gate.armed.swap(true, Ordering::SeqCst));
                            Ok(Usage::new(6, 36))
                        },
                        |usage, reservations, budget| async move {
                            let keys = KeyManager::new(InMemoryKeyRepository::new());
                            let (provider, key) = reservations.into_parts();
                            if case == CompletionCase::CancelUnpriced {
                                let pricing = crate::config::models::gateway::GatewayPricingConfig {
                                    unpriced_model_policy: crate::config::models::gateway::UnpricedModelPolicy::AllowUnpriced,
                                    unpriced_fallback_cost_per_1k_tokens: Some(1.0),
                                    ..Default::default()
                                };
                                spend::settle_unpriced_usage(&pricing, budget.budget_limits(), &keys, None,
                                    budget.provider(), budget.model(), &crate::core::pricing_service::PricingUsage::from(&usage), provider, key,
                                    "allowed unpriced completion").await;
                            } else {
                            spend::record_completion_spend_with_reservation(
                                spend::usage_spend_settlement_with_pricing(
                                    (budget.budget_limits(), &keys, None),
                                    (budget.provider(), budget.model(), Some(&usage)),
                                    ("openai", "gpt-4o-mini"),
                                    provider,
                                    key,
                                ),
                            )
                            .await;
                            }
                            let tokens = u64::from(usage.total_tokens);
                            (usage, tokens)
                        },
                    )
                    .await?;
                if matches!(
                    case,
                    CompletionCase::LocalConversionError
                        | CompletionCase::CancelLocalConversionError
                ) {
                    if case == CompletionCase::CancelLocalConversionError {
                        assert!(!settlement_gate.armed.swap(true, Ordering::SeqCst));
                    }
                    return Err(crate::core::providers::ProviderError::response_parsing(
                        "openai",
                        "local response conversion rejected the value",
                    ));
                }
                Ok(((), result.1))
            }
        },
    ));

    tokio::select! {
        _ = fixture.reply_held() => {},
        _ = &mut request => panic!("route completion must wait for the held budget reply"),
    }
    assert_eq!(original.state.active_requests.load(Ordering::Relaxed), 1);
    let cost = fixture.state("provider").await.0;
    assert!(
        cost > 0,
        "the real Redis dollar settlement has been applied"
    );

    let replacement = if case == CompletionCase::ReplaceThenCancel {
        router.remove_deployment(&id);
        router.add_deployment(deployment());
        let replacement = router.get_deployment(&id).unwrap();
        assert_eq!(replacement.state.active_requests.load(Ordering::Relaxed), 0);
        Some(replacement)
    } else {
        None
    };
    if matches!(
        case,
        CompletionCase::Cancel | CompletionCase::CancelUnpriced | CompletionCase::ReplaceThenCancel
    ) {
        drop(request);
        fixture.gate.release.notify_one();
    } else if case == CompletionCase::CancelLocalConversionError {
        fixture.gate.release.notify_one();
        tokio::select! {
            _ = fixture.reply_held() => {},
            _ = &mut request => panic!("error outcome must reach held routing settlement"),
        }
        assert_eq!(original.state.fail_requests.load(Ordering::Relaxed), 1);
        drop(request);
        fixture.gate.release.notify_one();
    } else {
        fixture.gate.release.notify_one();
        let result = tokio::time::timeout(Duration::from_secs(5), request)
            .await
            .unwrap();
        if case == CompletionCase::LocalConversionError {
            assert!(result.is_err());
        } else {
            result.unwrap();
        }
    }
    assert_eq!(original.state.active_requests.load(Ordering::Relaxed), 0);
    let success = u64::from(!matches!(
        case,
        CompletionCase::LocalConversionError | CompletionCase::CancelLocalConversionError
    ));
    assert_eq!(
        original.state.success_requests.load(Ordering::Relaxed),
        success
    );
    assert_eq!(original.state.total_requests.load(Ordering::Relaxed), 1);
    assert_eq!(original.state.rpm_current.load(Ordering::Relaxed), 1);
    assert_eq!(original.state.tpm_current.load(Ordering::Relaxed), 42);
    if let Some(replacement) = replacement {
        assert_eq!(
            replacement.state.success_requests.load(Ordering::Relaxed),
            0
        );
        assert_eq!(replacement.state.total_requests.load(Ordering::Relaxed), 0);
        assert_eq!(replacement.state.rpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(replacement.state.tpm_current.load(Ordering::Relaxed), 0);
    }

    fixture.wait_state(cost, 0).await;
    let mut control = fixture.control.clone();
    let route_key = RedisPool::admission_key(&id);
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let state: (i64, i64) = redis::cmd("HMGET")
                .arg(&route_key)
                .arg(&["p", "t"])
                .query_async(&mut control)
                .await
                .unwrap();
            if state == (0, 42) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("routing admission must settle actual tokens exactly once");
    fixture.finish().await;
    route_pool.delete(&route_key).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_budget_wait_retains_routing_success_and_actual_admission() {
    route_budget_completion(CompletionCase::Cancel).await;
}

#[tokio::test(flavor = "current_thread")]
async fn completed_budget_wait_records_routing_success_exactly_once() {
    route_budget_completion(CompletionCase::Complete).await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_budget_wait_keeps_original_lease_after_deployment_id_reuse() {
    route_budget_completion(CompletionCase::ReplaceThenCancel).await;
}

#[tokio::test(flavor = "current_thread")]
async fn local_conversion_failure_after_budget_wait_does_not_claim_success() {
    route_budget_completion(CompletionCase::LocalConversionError).await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_error_routing_settlement_retains_failure_and_usage() {
    route_budget_completion(CompletionCase::CancelLocalConversionError).await;
}

#[tokio::test(flavor = "current_thread")]
async fn allowed_unpriced_completion_is_still_success_when_budget_wait_is_cancelled() {
    route_budget_completion(CompletionCase::CancelUnpriced).await;
}
