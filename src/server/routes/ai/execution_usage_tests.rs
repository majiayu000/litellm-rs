//! Unknown usage must survive the real gateway completion/spend/admission chain.
use super::{completion, execute_with_selected_deployment};
use crate::config::models::storage::RedisConfig;
use crate::core::budget::UnifiedBudgetLimits;
use crate::core::keys::{InMemoryKeyRepository, KeyManager};
use crate::core::pricing_service::{PricingService, PricingUsage};
use crate::core::providers::{Provider, ProviderError, openai::OpenAIProvider};
use crate::core::router::{Deployment, DeploymentConfig, RouterConfig, UnifiedRouter};
use crate::core::types::{model::ProviderCapability, responses::Usage};
use crate::server::routes::ai::{budgeted::BudgetedCall, spend};
use crate::storage::redis::RedisPool;
use crate::utils::error::gateway_error::GatewayError;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wait {
    Complete,
    Budget,
    Postprocess,
}

async fn request(
    router: &UnifiedRouter,
    usage: Option<u64>,
    wait: Wait,
    entered: Arc<tokio::sync::Notify>,
    calls: Arc<AtomicUsize>,
) -> Result<(), GatewayError> {
    execute_with_selected_deployment(
        router,
        "usage-fixture",
        ProviderCapability::ChatCompletion,
        24,
        move |_, _, _| {
            let entered = entered.clone();
            let calls = calls.clone();
            async move {
                BudgetedCall::new(Arc::new(UnifiedBudgetLimits::new()), "openai", "gpt-4o")
                    .reserve_call_settle(
                        async |_| Ok(None),
                        || async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            Ok::<_, ProviderError>(usage.map(|tokens| Usage::new(tokens as u32, 0)))
                        },
                        |observed, reservations, budget| async move {
                            let keys = KeyManager::new(InMemoryKeyRepository::new());
                            let (provider, key) = reservations.into_parts();
                            // This is the same missing/known usage spend entry
                            // used by chat and by embeddings without usage.
                            spend::record_completion_spend_with_reservation(
                                spend::usage_spend_settlement(
                                    (budget.budget_limits(), &keys, None),
                                    (budget.provider(), budget.model(), observed.as_ref()),
                                    provider,
                                    key,
                                ),
                            )
                            .await;
                            if wait != Wait::Complete {
                                // Two overlapping accounting guards must have
                                // exactly one owner when the request is dropped.
                                let _first = (wait == Wait::Budget)
                                    .then(completion::BudgetSettlementWait::new);
                                let _second = (wait == Wait::Budget)
                                    .then(completion::BudgetSettlementWait::new);
                                entered.notify_one();
                                std::future::pending::<()>().await;
                            }
                            (observed, usage.unwrap_or(0))
                        },
                    )
                    .await
                    .map(|(_, tokens)| ((), tokens))
            }
        },
    )
    .await
}

#[tokio::test]
async fn gateway_missing_usage_retains_estimate_through_success_and_cancelled_accounting() {
    for shared in [false, true] {
        let pool = if shared {
            let Ok(url) = std::env::var("REDIS_URL") else {
                assert!(
                    std::env::var_os("CI").is_none(),
                    "CI requires actual REDIS_URL"
                );
                eprintln!("Skipping shared gateway usage cases: REDIS_URL unset");
                continue;
            };
            Some(Arc::new(
                RedisPool::new(&RedisConfig {
                    url,
                    enabled: true,
                    allow_degraded: false,
                    ..Default::default()
                })
                .await
                .unwrap(),
            ))
        } else {
            None
        };
        for usage in [None, Some(0), Some(3)] {
            for wait in [Wait::Complete, Wait::Budget, Wait::Postprocess] {
                let router = UnifiedRouter::new(RouterConfig {
                    num_retries: 0,
                    max_fallbacks: 0,
                    ..Default::default()
                });
                let router = match &pool {
                    Some(pool) => router.with_admission_redis(pool.clone()),
                    None => router,
                };
                let id = format!("gateway-usage-{}", uuid::Uuid::new_v4());
                router.add_deployment(
                    Deployment::new(
                        id.clone(),
                        Provider::OpenAI(
                            OpenAIProvider::with_api_key("sk-usage-fixture")
                                .await
                                .unwrap(),
                        ),
                        "gpt-4o".into(),
                        "usage-fixture".into(),
                    )
                    .with_config(DeploymentConfig {
                        tpm_limit: Some(32),
                        rpm_limit: Some(100),
                        max_parallel_requests: Some(100),
                        ..Default::default()
                    }),
                );
                let deployment = router.get_deployment(&id).unwrap();
                if let Some(pool) = &pool {
                    let mut conn = pool.open_live_connection().await.unwrap();
                    let (seconds, _): (u64, u64) =
                        redis::cmd("TIME").query_async(&mut conn).await.unwrap();
                    if seconds % 60 >= 40 {
                        tokio::time::sleep(Duration::from_secs(61 - seconds % 60)).await;
                    }
                }
                deployment.state.reset_minute();
                if usage.is_none() && wait == Wait::Complete {
                    // No completed provider response and no usage observation:
                    // cancellation must refund the reservation, so the first
                    // real completion below can still dispatch its full estimate.
                    let entered = Arc::new(tokio::sync::Notify::new());
                    let signal = entered.clone();
                    let mut pending = Box::pin(execute_with_selected_deployment(
                        &router,
                        "usage-fixture",
                        ProviderCapability::ChatCompletion,
                        24,
                        move |_, _, _| {
                            let signal = signal.clone();
                            async move {
                                signal.notify_one();
                                std::future::pending::<Result<((), u64), ProviderError>>().await
                            }
                        },
                    ));
                    tokio::select! {
                        result = pending.as_mut() => panic!("provider must wait: {result:?}"),
                        () = entered.notified() => {}
                    }
                    assert!(futures::poll!(pending.as_mut()).is_pending());
                    drop(pending);
                    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                    assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 0);
                    assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
                    assert_eq!(deployment.state.total_requests.load(Ordering::Relaxed), 0);
                    assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
                    if let Some(pool) = &pool {
                        assert_admission(pool, &deployment, 0, 0).await;
                    }
                }
                let calls = Arc::new(AtomicUsize::new(0));
                let entered = Arc::new(tokio::sync::Notify::new());
                let mut first = Box::pin(request(
                    &router,
                    usage,
                    wait,
                    entered.clone(),
                    calls.clone(),
                ));
                if wait == Wait::Complete {
                    first.await.unwrap();
                } else {
                    tokio::select! {
                        result = first.as_mut() => panic!("completion must wait: {result:?}"),
                        () = entered.notified() => {}
                    }
                    assert!(futures::poll!(first.as_mut()).is_pending());
                    drop(first);
                }
                assert_eq!(calls.load(Ordering::SeqCst), 1);
                assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
                assert_eq!(
                    deployment.state.tpm_current.load(Ordering::Relaxed),
                    usage.unwrap_or(0)
                );
                let first_successes = u64::from(wait != Wait::Postprocess);
                assert_eq!(
                    deployment.state.success_requests.load(Ordering::Relaxed),
                    first_successes
                );
                assert_eq!(
                    deployment.state.total_requests.load(Ordering::Relaxed),
                    first_successes
                );
                assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
                if let Some(pool) = &pool {
                    assert_admission(pool, &deployment, 1, usage.unwrap_or(24)).await;
                }
                let second = request(
                    &router,
                    usage,
                    Wait::Complete,
                    Arc::new(tokio::sync::Notify::new()),
                    calls.clone(),
                )
                .await;
                let completed = if usage.is_none() {
                    assert!(second.is_err(), "unknown usage must retain estimated TPM");
                    1
                } else {
                    second.unwrap();
                    2
                };
                assert_eq!(calls.load(Ordering::SeqCst), completed as usize);
                assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                assert_eq!(
                    deployment.state.rpm_current.load(Ordering::Relaxed),
                    completed
                );
                assert_eq!(
                    deployment.state.tpm_current.load(Ordering::Relaxed),
                    usage.unwrap_or(0) * completed
                );
                assert_eq!(
                    deployment.state.total_requests.load(Ordering::Relaxed),
                    first_successes + completed - 1
                );
                assert_eq!(
                    deployment.state.success_requests.load(Ordering::Relaxed),
                    first_successes + completed - 1
                );
                assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
                if let Some(pool) = &pool {
                    assert_admission(
                        pool,
                        &deployment,
                        completed,
                        usage.map_or(24, |tokens| tokens * completed),
                    )
                    .await;
                    pool.delete(&RedisPool::admission_key(&deployment.shared_state_id()))
                        .await
                        .unwrap();
                }
            }
        }
    }
}

async fn assert_admission(pool: &RedisPool, deployment: &Deployment, requests: u64, tokens: u64) {
    let key = RedisPool::admission_key(&deployment.shared_state_id());
    let mut conn = pool.open_live_connection().await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let counts: (i64, i64, i64) = redis::cmd("HMGET")
                .arg(&key)
                .arg(&["p", "r", "t"])
                .query_async(&mut conn)
                .await
                .unwrap();
            if counts == (0, requests as i64, tokens as i64) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("owned admission cleanup must retain unknown or settle known usage once");
    let fields: Vec<String> = redis::cmd("HKEYS")
        .arg(&key)
        .query_async(&mut conn)
        .await
        .unwrap();
    assert!(!fields.iter().any(|field| field.starts_with("l:")));
}

fn audio_admission_pricing() -> PricingService {
    let pricing = PricingService::with_embedded_default().unwrap();
    let snapshot = pricing.snapshot();
    let (_, info) = snapshot
        .get_model_info_for_provider("openai", "gpt-4o")
        .unwrap();
    let mut info = info.clone();
    info.extra.insert(
        "input_cost_per_audio_token".into(),
        serde_json::json!(0.001),
    );
    info.extra.insert(
        "output_cost_per_image_token".into(),
        serde_json::json!(0.002),
    );
    pricing.add_custom_model("gpt-4o".into(), info);
    pricing
}

async fn priced_media_request(
    router: &UnifiedRouter,
    priced: bool,
    image: bool,
    wait: Wait,
    entered: Arc<tokio::sync::Notify>,
    calls: Arc<AtomicUsize>,
) -> Result<(), GatewayError> {
    execute_with_selected_deployment(
        router,
        "audio-usage-fixture",
        ProviderCapability::ChatCompletion,
        66,
        move |_, _, _| {
            let entered = entered.clone();
            let calls = calls.clone();
            async move {
                BudgetedCall::new(Arc::new(UnifiedBudgetLimits::new()), "openai", "gpt-4o")
                    .reserve_call_settle(
                        async |_| Ok(None),
                        || async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            let mut usage = PricingUsage::new(2, 0);
                            if image {
                                usage.image_tokens = Some(64);
                                usage.output_image_count = Some(1);
                            } else {
                                usage.audio_tokens = Some(64);
                            }
                            Ok::<_, ProviderError>(usage)
                        },
                        |usage, reservations, budget| async move {
                            completion::observe_unknown_usage();
                            let pricing = audio_admission_pricing();
                            let request_pricing = spend::RequestPricing::from_exact(
                                &pricing,
                                "openai",
                                if priced {
                                    "gpt-4o"
                                } else {
                                    "audio-unpriced-fixture"
                                },
                            );
                            assert_eq!(
                                request_pricing.calculate_settlement(&usage).is_ok(),
                                priced
                            );
                            let keys = KeyManager::new(InMemoryKeyRepository::new());
                            let (provider, key) = reservations.into_parts();
                            spend::record_pricing_usage_spend_with_admission(
                                &request_pricing,
                                &crate::config::models::gateway::GatewayPricingConfig::default(),
                                budget.budget_limits(),
                                &keys,
                                None,
                                budget.provider(),
                                budget.model(),
                                &usage,
                                provider,
                                key,
                                super::estimate::usage(&usage),
                            )
                            .await;
                            if wait == Wait::Budget {
                                let _first = completion::BudgetSettlementWait::new();
                                let _second = completion::BudgetSettlementWait::new();
                                entered.notify_one();
                                std::future::pending::<()>().await;
                            }
                            (usage, 0)
                        },
                    )
                    .await
            }
        },
    )
    .await
    .map(|_| ())
}

#[tokio::test]
async fn audio_pricing_observation_preserves_combined_admission_through_cancelled_accounting() {
    assert_media_pricing_admission(false).await;
}

#[tokio::test]
async fn image_pricing_observation_preserves_combined_admission_through_cancelled_accounting() {
    assert_media_pricing_admission(true).await;
}

async fn assert_media_pricing_admission(image: bool) {
    for shared in [false, true] {
        let pool = if shared {
            let Ok(url) = std::env::var("REDIS_URL") else {
                assert!(
                    std::env::var_os("CI").is_none(),
                    "CI requires actual REDIS_URL"
                );
                eprintln!("Skipping shared audio admission cases: REDIS_URL unset");
                continue;
            };
            Some(Arc::new(
                RedisPool::new(&RedisConfig {
                    url,
                    enabled: true,
                    allow_degraded: false,
                    ..Default::default()
                })
                .await
                .unwrap(),
            ))
        } else {
            None
        };
        for priced in [false, true] {
            for wait in [Wait::Complete, Wait::Budget] {
                let router = UnifiedRouter::new(RouterConfig {
                    num_retries: 0,
                    max_fallbacks: 0,
                    ..Default::default()
                });
                let router = match &pool {
                    Some(pool) => router.with_admission_redis(pool.clone()),
                    None => router,
                };
                let id = format!("audio-usage-{}", uuid::Uuid::new_v4());
                router.add_deployment(
                    Deployment::new(
                        id.clone(),
                        Provider::OpenAI(
                            OpenAIProvider::with_api_key("sk-audio-usage-fixture")
                                .await
                                .unwrap(),
                        ),
                        "gpt-4o".into(),
                        "audio-usage-fixture".into(),
                    )
                    .with_config(DeploymentConfig {
                        tpm_limit: Some(100),
                        rpm_limit: Some(100),
                        max_parallel_requests: Some(100),
                        ..Default::default()
                    }),
                );
                let deployment = router.get_deployment(&id).unwrap();
                loop {
                    let local = crate::core::router::deployment::current_timestamp() % 60;
                    let shared = if let Some(pool) = &pool {
                        let mut conn = pool.open_live_connection().await.unwrap();
                        let (seconds, _): (u64, u64) =
                            redis::cmd("TIME").query_async(&mut conn).await.unwrap();
                        seconds % 60
                    } else {
                        local
                    };
                    let delay = [local, shared]
                        .into_iter()
                        .filter(|second| *second >= 40)
                        .map(|second| 61 - second)
                        .max();
                    match delay {
                        Some(seconds) => tokio::time::sleep(Duration::from_secs(seconds)).await,
                        None => break,
                    }
                }
                deployment.state.reset_minute();
                let entered = Arc::new(tokio::sync::Notify::new());
                let calls = Arc::new(AtomicUsize::new(0));
                let mut first = Box::pin(priced_media_request(
                    &router,
                    priced,
                    image,
                    wait,
                    entered.clone(),
                    calls.clone(),
                ));
                if wait == Wait::Complete {
                    first.await.unwrap();
                } else {
                    tokio::select! {
                        result = first.as_mut() => panic!("accounting must wait: {result:?}"),
                        () = entered.notified() => {}
                    }
                    assert!(futures::poll!(first.as_mut()).is_pending());
                    drop(first);
                }
                assert_eq!(calls.load(Ordering::SeqCst), 1);
                assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
                // Shared reservations live in Redis; the local ledger only
                // owns reservations for the in-process admission backend.
                assert_eq!(
                    deployment
                        .state
                        .admission_tpm(crate::core::router::deployment::current_timestamp()),
                    if shared { 0 } else { 66 },
                    "local admission ownership: shared={shared}, priced={priced}, wait={wait:?}"
                );
                assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
                assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 1);
                assert_eq!(deployment.state.fail_requests.load(Ordering::Relaxed), 0);
                assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                if let Some(pool) = &pool {
                    assert_admission(pool, &deployment, 1, 66).await;
                }
                assert!(
                    priced_media_request(
                        &router,
                        priced,
                        image,
                        Wait::Complete,
                        Arc::new(tokio::sync::Notify::new()),
                        calls.clone(),
                    )
                    .await
                    .is_err()
                );
                assert_eq!(calls.load(Ordering::SeqCst), 1);
                assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
                assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
                assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
                if let Some(pool) = &pool {
                    assert_admission(pool, &deployment, 1, 66).await;
                    pool.delete(&RedisPool::admission_key(&deployment.shared_state_id()))
                        .await
                        .unwrap();
                }
            }
        }
    }
}

#[tokio::test]
async fn generic_pricing_observation_keeps_authoritative_total_with_audio_details() {
    for priced in [false, true] {
        let router = UnifiedRouter::new(RouterConfig {
            num_retries: 0,
            max_fallbacks: 0,
            ..Default::default()
        });
        router.add_deployment(
            Deployment::new(
                "generic-audio-details".into(),
                Provider::OpenAI(
                    OpenAIProvider::with_api_key("sk-generic-audio-details")
                        .await
                        .unwrap(),
                ),
                "gpt-4o".into(),
                "audio-details-fixture".into(),
            )
            .with_config(DeploymentConfig {
                tpm_limit: Some(100),
                ..Default::default()
            }),
        );
        execute_with_selected_deployment(
            &router,
            "audio-details-fixture",
            ProviderCapability::ChatCompletion,
            24,
            move |_, _, _| async move {
                let pricing = audio_admission_pricing();
                let request_pricing = spend::RequestPricing::from_exact(
                    &pricing,
                    "openai",
                    if priced {
                        "gpt-4o"
                    } else {
                        "unpriced-audio-details"
                    },
                );
                let limits = UnifiedBudgetLimits::new();
                let keys = KeyManager::new(InMemoryKeyRepository::new());
                let mut usage = PricingUsage::new(10, 2);
                // This is an input detail already included in the total of 12.
                usage.audio_tokens = Some(4);
                // Observe the authoritative provider total at the route boundary.
                completion::observe_usage(u64::from(usage.total_tokens));
                assert_eq!(request_pricing.calculate_settlement(&usage).is_ok(), priced);
                spend::record_pricing_usage_spend_with_request_pricing(
                    &request_pricing,
                    &crate::config::models::gateway::GatewayPricingConfig::default(),
                    &limits,
                    &keys,
                    None,
                    "openai",
                    "gpt-4o",
                    &usage,
                    None,
                    None,
                )
                .await;
                Ok::<_, ProviderError>(((), 16))
            },
        )
        .await
        .unwrap();
        let deployment = router.get_deployment("generic-audio-details").unwrap();
        assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 12);
        assert_eq!(deployment.state.rpm_current.load(Ordering::Relaxed), 1);
        assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 1);
        assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
    }
}
