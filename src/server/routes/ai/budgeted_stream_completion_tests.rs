//! Exercise the production stream lease across a real dollar-budget wait.

use super::*;
use crate::core::keys::{InMemoryKeyRepository, KeyManager};
use crate::core::providers::{Provider, ProviderError, openai::OpenAIProvider};
use crate::core::router::{Deployment, DeploymentConfig, UnifiedRouter};
use crate::core::types::{model::ProviderCapability, responses::Usage};
use crate::server::routes::ai::{execution, spend};
use futures::StreamExt;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Case {
    CancelEof,
    CompleteEof,
    ReplaceEof,
    CancelError,
    CompleteError,
    CancelErrorRouting,
}

// Admission RPM is a Redis-minute counter. Avoid beginning this bounded case
// just before its legitimate rollover; use the server clock, not the test host.
async fn wait_for_admission_window(control: &mut redis::aio::MultiplexedConnection) {
    tokio::time::timeout(Duration::from_secs(25), async {
        loop {
            let (seconds, micros): (u64, u64) =
                redis::cmd("TIME").query_async(control).await.unwrap();
            let remaining_ms = 60_000 - ((seconds % 60) * 1_000 + micros / 1_000);
            if remaining_ms >= 20_000 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(remaining_ms + 20)).await;
        }
    })
    .await
    .expect("Redis must reach a stable admission-minute window");
}

async fn stream_budget_completion(case: Case, interrupted: bool, tokens: u32) {
    let _serial = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    wait_for_admission_window(&mut fixture.control.clone()).await;
    let route_pool = if case == Case::CancelErrorRouting {
        fixture.pool.clone()
    } else {
        Arc::new(
            RedisPool::new(&RedisConfig {
                url: std::env::var("REDIS_URL").unwrap(),
                enabled: true,
                allow_degraded: false,
                ..Default::default()
            })
            .await
            .unwrap(),
        )
    };
    let router = Arc::new(UnifiedRouter::default().with_admission_redis(route_pool.clone()));
    let id = uuid::Uuid::new_v4().to_string();
    let provider = Provider::OpenAI(OpenAIProvider::with_api_key("sk-test-key").await.unwrap());
    let deployment = || {
        Deployment::new(
            id.clone(),
            provider.clone(),
            "gpt-4o-mini".into(),
            "joint-stream".into(),
        )
        .with_config(DeploymentConfig {
            max_parallel_requests: Some(1),
            rpm_limit: Some(1_000),
            tpm_limit: Some(10_000),
            ..Default::default()
        })
    };
    router.add_deployment(deployment());
    let original = router.get_deployment(&id).unwrap();
    let is_error = matches!(
        case,
        Case::CancelError | Case::CompleteError | Case::CancelErrorRouting
    );
    let limits = fixture.limits.clone();
    let budget_provider = fixture.provider.clone();
    let budget_model = fixture.model.clone();
    let ((mut upstream, reservations), mut lease) =
        execution::execute_stream_with_selected_deployment(
            router.clone(),
            "joint-stream",
            ProviderCapability::ChatCompletionStream,
            0,
            move |_, _, _| {
                let call = BudgetedCall::new(
                    limits.clone(),
                    budget_provider.clone(),
                    budget_model.clone(),
                );
                async move {
                    call.reserve_call(
                        async |budget| budget.reserve_spend(1.0).await.map(Some),
                        || async move {
                            let mut events = vec![Ok(Usage::new(0, tokens))];
                            if is_error {
                                events.push(Err(ProviderError::rate_limit("openai", Some(60))));
                            }
                            Ok(futures::stream::iter(events))
                        },
                    )
                    .await
                }
            },
        )
        .await
        .unwrap();
    let usage = upstream.next().await.unwrap().unwrap();
    // Usage is observable while the provider is still open, never success.
    assert_eq!(original.state.success_requests.load(Ordering::Relaxed), 0);
    assert_eq!(original.state.active_requests.load(Ordering::Relaxed), 1);
    let terminal_error = if is_error {
        let error = upstream.next().await.unwrap().unwrap_err();
        assert!(matches!(
            error,
            ProviderError::RateLimit {
                retry_after: Some(60),
                ..
            }
        ));
        Some(error)
    } else {
        assert!(
            upstream.next().await.is_none(),
            "success requires actual EOF"
        );
        None
    };
    let (provider_reservation, key_reservation) = reservations.into_parts();
    let gate = fixture.gate.clone();
    let limits = fixture.limits.clone();
    let budget_provider = fixture.provider.clone();
    let budget_model = fixture.model.clone();
    fixture.pause_reply();
    let mut terminal = Box::pin(async move {
        let keys = KeyManager::new(InMemoryKeyRepository::new());
        let settlement = spend::record_completion_spend_with_reservation(
            spend::usage_spend_settlement_with_pricing(
                (&limits, &keys, None),
                (&budget_provider, &budget_model, Some(&usage)),
                ("openai", "gpt-4o-mini"),
                provider_reservation,
                key_reservation,
            ),
        );
        if interrupted {
            lease
                .settle_interrupted(u64::from(tokens), terminal_error.as_ref(), settlement)
                .await;
        } else {
            lease
                .settle_terminal(u64::from(tokens), terminal_error.as_ref(), settlement)
                .await;
        }
        if case == Case::CancelErrorRouting {
            assert!(!gate.armed.swap(true, Ordering::SeqCst));
        }
        #[cfg(feature = "websockets")]
        if interrupted {
            lease
                .finish_interrupted(u64::from(tokens), terminal_error.as_ref())
                .await;
            return terminal_error;
        }
        if let Some(error) = &terminal_error {
            lease
                .finish_failure_with_tokens(error, u64::from(tokens))
                .await;
        } else {
            lease.finish_success(u64::from(tokens)).await;
        }
        terminal_error
    });
    tokio::select! {
        _ = fixture.reply_held() => {},
        _ = &mut terminal => panic!("terminal routing completion must wait for the budget reply"),
    }
    let cost = fixture.state("provider").await.0;
    if tokens == 0 {
        assert_eq!(
            fixture.state("provider").await,
            (0, 0),
            "zero-cost settlement must have released the real outstanding reservation"
        );
    } else {
        assert!(cost > 0);
    }
    let replacement = if case == Case::ReplaceEof {
        router.remove_deployment(&id);
        router.add_deployment(deployment());
        Some(router.get_deployment(&id).unwrap())
    } else {
        None
    };
    match case {
        Case::CancelEof | Case::ReplaceEof | Case::CancelError => {
            drop(terminal);
            fixture.gate.release.notify_one();
        }
        Case::CancelErrorRouting => {
            fixture.gate.release.notify_one();
            tokio::select! {
                _ = fixture.reply_held() => {},
                _ = &mut terminal => panic!("typed failure must wait for the routing reply"),
            }
            assert_eq!(original.state.fail_requests.load(Ordering::Relaxed), 1);
            assert_eq!(
                original.state.tpm_current.load(Ordering::Relaxed),
                u64::from(tokens)
            );
            assert_eq!(original.state.active_requests.load(Ordering::Relaxed), 0);
            drop(terminal);
            fixture.gate.release.notify_one();
        }
        Case::CompleteEof | Case::CompleteError => {
            fixture.gate.release.notify_one();
            let error = tokio::time::timeout(Duration::from_secs(5), terminal)
                .await
                .unwrap();
            if is_error {
                assert!(matches!(
                    error,
                    Some(ProviderError::RateLimit {
                        retry_after: Some(60),
                        ..
                    })
                ));
            } else {
                assert!(error.is_none());
            }
        }
    }
    assert_eq!(original.state.active_requests.load(Ordering::Relaxed), 0);
    assert_eq!(original.state.total_requests.load(Ordering::Relaxed), 1);
    assert_eq!(
        original.state.success_requests.load(Ordering::Relaxed),
        u64::from(!is_error)
    );
    assert_eq!(
        original.state.fail_requests.load(Ordering::Relaxed),
        u64::from(is_error)
    );
    assert_eq!(
        original.state.rpm_current.load(Ordering::Relaxed),
        u64::from(!is_error || interrupted)
    );
    assert_eq!(
        original.state.tpm_current.load(Ordering::Relaxed),
        u64::from(tokens)
    );
    if let Some(replacement) = replacement {
        assert_eq!(replacement.state.total_requests.load(Ordering::Relaxed), 0);
        assert_eq!(replacement.state.rpm_current.load(Ordering::Relaxed), 0);
        assert_eq!(replacement.state.tpm_current.load(Ordering::Relaxed), 0);
    }
    fixture.wait_state(cost, 0).await;
    let route_key = RedisPool::admission_key(&id);
    let mut control = fixture.control.clone();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let state: (i64, i64, i64) = redis::cmd("HMGET")
                .arg(&route_key)
                .arg(&["p", "r", "t"])
                .query_async(&mut control)
                .await
                .unwrap();
            let expected_rpm = i64::from(!is_error || interrupted || tokens > 0);
            if state == (0, expected_rpm, i64::from(tokens)) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("terminal admission must retain actual usage exactly once");
    fixture.finish().await;
    route_pool.delete(&route_key).await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn eof_then_cancelled_budget_wait_retains_success() {
    stream_budget_completion(Case::CancelEof, false, 42).await;
}
#[tokio::test(flavor = "current_thread")]
async fn eof_budget_settlement_records_success_exactly_once() {
    stream_budget_completion(Case::CompleteEof, false, 42).await;
}
#[tokio::test(flavor = "current_thread")]
async fn eof_budget_cancellation_keeps_original_lease_after_id_reuse() {
    stream_budget_completion(Case::ReplaceEof, false, 42).await;
}
#[tokio::test(flavor = "current_thread")]
async fn typed_terminal_error_then_cancelled_budget_wait_retains_failure() {
    stream_budget_completion(Case::CancelError, false, 42).await;
}
#[tokio::test(flavor = "current_thread")]
async fn typed_terminal_error_budget_settlement_records_failure_once() {
    stream_budget_completion(Case::CompleteError, false, 42).await;
}
#[tokio::test(flavor = "current_thread")]
async fn typed_terminal_error_then_cancelled_routing_wait_retains_failure() {
    stream_budget_completion(Case::CancelErrorRouting, false, 42).await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_terminal_wait_finalizes_a_lease_retained_by_its_caller() {
    let _serial = TESTS.lock().await;
    let Some(fixture) = Fixture::new().await else {
        return;
    };
    let router = Arc::new(UnifiedRouter::default().with_admission_redis(fixture.pool.clone()));
    let id = uuid::Uuid::new_v4().to_string();
    router.add_deployment(
        Deployment::new(
            id.clone(),
            Provider::OpenAI(OpenAIProvider::with_api_key("sk-test-key").await.unwrap()),
            "gpt-4o-mini".into(),
            "retained-stream".into(),
        )
        .with_config(DeploymentConfig {
            max_parallel_requests: Some(1),
            ..Default::default()
        }),
    );
    let original = router.get_deployment(&id).unwrap();
    let (_, mut lease) = execution::execute_stream_with_selected_deployment(
        router.clone(),
        "retained-stream",
        ProviderCapability::ChatCompletionStream,
        0,
        |_, _, _| async { Ok(()) },
    )
    .await
    .unwrap();
    let reservation = fixture
        .limits
        .reserve_spend_async(&fixture.provider, &fixture.model, 1.0)
        .await
        .unwrap();
    fixture.pause_reply();
    let mut wait = Box::pin(lease.settle_terminal(
        42,
        None,
        execution::completion::settle_budget(reservation, 0.25),
    ));
    tokio::select! {
        _ = fixture.reply_held() => {},
        _ = &mut wait => panic!("must suspend in the real budget wait"),
    }
    drop(wait); // Keep lease alive, as a timeout/select caller does.
    assert_eq!(original.state.active_requests.load(Ordering::Relaxed), 0);
    assert_eq!(original.state.success_requests.load(Ordering::Relaxed), 1);
    assert_eq!(original.state.rpm_current.load(Ordering::Relaxed), 1);
    assert_eq!(original.state.tpm_current.load(Ordering::Relaxed), 42);
    fixture.gate.release.notify_one();
    lease.complete_response(42, None).await;
    lease.finish_success(42).await;
    assert_eq!(original.state.total_requests.load(Ordering::Relaxed), 1);
    assert_eq!(original.state.success_requests.load(Ordering::Relaxed), 1);
    assert_eq!(original.state.rpm_current.load(Ordering::Relaxed), 1);
    assert_eq!(original.state.tpm_current.load(Ordering::Relaxed), 42);
    fixture.wait_state(250_000_000, 0).await;
    let route_key = RedisPool::admission_key(&id);
    let mut control = fixture.control.clone();
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
    .expect("retained lease must still release shared admission");
    fixture.finish().await;
    fixture.pool.delete(&route_key).await.unwrap();
}

#[cfg(feature = "websockets")]
#[tokio::test(flavor = "current_thread")]
async fn realtime_failed_terminal_budget_cancellation_keeps_interrupted_usage() {
    stream_budget_completion(Case::CancelError, true, 42).await;
}

#[cfg(feature = "websockets")]
#[tokio::test(flavor = "current_thread")]
async fn realtime_failed_terminal_records_interrupted_usage_exactly_once() {
    stream_budget_completion(Case::CompleteError, true, 42).await;
}

#[tokio::test(flavor = "current_thread")]
async fn zero_token_failure_budget_cancellation_refunds_shared_rpm() {
    stream_budget_completion(Case::CancelError, false, 0).await;
}

#[tokio::test(flavor = "current_thread")]
async fn zero_token_failure_completion_refunds_shared_rpm() {
    stream_budget_completion(Case::CompleteError, false, 0).await;
}

#[cfg(feature = "websockets")]
#[tokio::test(flavor = "current_thread")]
async fn zero_token_realtime_failure_budget_cancellation_retains_shared_rpm() {
    stream_budget_completion(Case::CancelError, true, 0).await;
}

#[cfg(feature = "websockets")]
#[tokio::test(flavor = "current_thread")]
async fn zero_token_realtime_failure_completion_retains_shared_rpm() {
    stream_budget_completion(Case::CompleteError, true, 0).await;
}
