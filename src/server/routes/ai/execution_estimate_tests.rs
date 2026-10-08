use super::*;
use crate::config::models::storage::RedisConfig;
use crate::core::providers::{Provider, ProviderError, openai::OpenAIProvider};
use crate::core::router::{Deployment, DeploymentConfig, RouterConfig, UnifiedRouter};
use crate::core::types::model::ProviderCapability;
use crate::storage::redis::RedisPool;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

fn request() -> ChatCompletionRequest {
    serde_json::from_value(serde_json::json!({
        "model": "gateway-admission-alias",
        "messages": [{"role": "user", "content": "a long prompt ".repeat(100)}],
        "max_tokens": 128
    }))
    .unwrap()
}

#[test]
fn gateway_admission_estimate_preserves_explicit_output_bound_and_choices() {
    let mut input = request();
    input.max_tokens = Some(1);
    let minimum = chat(&input, 0).unwrap();
    input.max_tokens = Some(100_000);
    assert_eq!(chat(&input, 0).unwrap(), minimum + 99_999);
    input.n = Some(2);
    assert_eq!(chat(&input, 0).unwrap(), minimum + 199_999);
    assert_eq!(chat(&input, 40).unwrap(), minimum + 200_039);
    let original = chat(&input, 0).unwrap();
    input.tools = Some(serde_json::from_value(serde_json::json!([
        {"type":"function", "function":{"name":"large_schema", "description":"schema ".repeat(1000), "parameters":{"type":"object"}}}
    ])).unwrap());
    assert!(chat(&input, 0).unwrap() > original + 1_000);
}

#[test]
fn gateway_admission_estimate_uses_largest_output_bound_for_unary_and_stream() {
    for stream in [false, true] {
        let mut input = request();
        input.stream = Some(stream);
        input.max_tokens = Some(1);
        input.max_completion_tokens = Some(1);
        input.n = Some(2);
        let minimum = chat(&input, 0).unwrap();
        for (max_tokens, max_completion_tokens) in [(100_000, 1), (1, 100_000)] {
            input.max_tokens = Some(max_tokens);
            input.max_completion_tokens = Some(max_completion_tokens);
            assert_eq!(chat(&input, 0).unwrap(), minimum + 199_998);
        }
    }
}

#[test]
fn gateway_admission_estimate_includes_visible_and_opaque_continuation_payloads() {
    use crate::core::types::anthropic_continuation::{
        AnthropicRedactedData, AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent,
    };

    let mut input = request();
    input.messages[0].role = crate::core::models::openai::MessageRole::Assistant;
    let baseline = chat(&input, 0).unwrap();
    assert_eq!(
        chat_with_continuation(&input, &[ChatMessageContinuation::new()], 0).unwrap(),
        baseline,
    );
    let continuation = |thinking: String, signature: String, data: String| {
        ChatMessageContinuation::new().with_anthropic_thinking(AnthropicThinkingContent::new(vec![
            AnthropicThinkingBlock::Thinking {
                thinking,
                signature: AnthropicSignature::try_from(signature).unwrap(),
            },
            AnthropicThinkingBlock::RedactedThinking {
                data: AnthropicRedactedData::try_from(data).unwrap(),
            },
        ]))
    };
    let small = continuation("visible".into(), "signature".into(), "encrypted".into());
    let small_estimate = chat_with_continuation(&input, &[small], 0).unwrap();
    assert!(small_estimate > baseline);
    // Each transmitted component independently increases admission, including
    // the two opaque components deliberately excluded from guardrail text.
    for large in [
        continuation(
            "visible ".repeat(2_000),
            "signature".into(),
            "encrypted".into(),
        ),
        continuation(
            "visible".into(),
            "signature".repeat(2_000),
            "encrypted".into(),
        ),
        continuation(
            "visible".into(),
            "signature".into(),
            "encrypted".repeat(2_000),
        ),
    ] {
        let tokens = chat_with_continuation(&input, std::slice::from_ref(&large), 0).unwrap();
        assert!(tokens > small_estimate + 1_000);
        assert_eq!(
            chat_with_continuation(&input, &[large], 40).unwrap(),
            tokens + 40,
        );
    }
}

async fn assert_second_dispatch_blocked(router: &Arc<UnifiedRouter>, tokens: u64) {
    let calls = Arc::new(AtomicUsize::new(0));
    let attempted = calls.clone();
    let unary = super::super::execute_with_selected_deployment(
        router,
        "gateway-admission-alias",
        ProviderCapability::ChatCompletion,
        tokens,
        move |_, _, _| {
            attempted.fetch_add(1, Ordering::SeqCst);
            async { Ok::<_, ProviderError>(((), 0)) }
        },
    )
    .await;
    assert!(unary.is_err(), "in-flight tokens must block unary dispatch");
    let attempted = calls.clone();
    let stream = super::super::execute_stream_with_selected_deployment(
        router.clone(),
        "gateway-admission-alias",
        ProviderCapability::ChatCompletionStream,
        tokens,
        move |_, _, _| {
            attempted.fetch_add(1, Ordering::SeqCst);
            async { Ok::<_, ProviderError>(()) }
        },
    )
    .await;
    assert!(
        stream.is_err(),
        "in-flight tokens must block streaming dispatch"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

async fn assert_shared_admission(
    pool: &RedisPool,
    deployment: &Deployment,
    expected: (i64, i64, i64),
) {
    assert!(!pool.is_noop(), "the shared branch must use actual Redis");
    let mut connection = pool.open_live_connection().await.unwrap();
    let counts: (i64, i64, i64) = redis::cmd("HMGET")
        .arg(RedisPool::admission_key(&deployment.shared_state_id()))
        .arg(&["p", "r", "t"])
        .query_async(&mut connection)
        .await
        .unwrap();
    assert_eq!(counts, expected);
}

#[tokio::test]
async fn gateway_request_estimates_block_concurrent_unary_and_stream_dispatch() {
    for shared in [false, true] {
        let pool = if shared {
            let Ok(url) = std::env::var("REDIS_URL") else {
                assert!(std::env::var("CI").is_err(), "REDIS_URL is required in CI");
                eprintln!("Skipping shared admission case: REDIS_URL is not configured");
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
        for first_is_stream in [false, true] {
            let router = UnifiedRouter::new(RouterConfig {
                num_retries: 0,
                ..Default::default()
            });
            let router = Arc::new(match &pool {
                Some(pool) => router.with_admission_redis(pool.clone()),
                None => router,
            });
            let tokens = chat(&request(), 0).unwrap();
            assert!(tokens > 128);
            let id = uuid::Uuid::new_v4().to_string();
            router.add_deployment(
                Deployment::new(
                    id.clone(),
                    Provider::OpenAI(
                        OpenAIProvider::with_api_key("sk-admission-fixture")
                            .await
                            .unwrap(),
                    ),
                    "gpt-4o-mini".into(),
                    "gateway-admission-alias".into(),
                )
                .with_config(DeploymentConfig {
                    rpm_limit: Some(100),
                    max_parallel_requests: Some(100),
                    tpm_limit: Some(tokens.saturating_add(tokens / 2)),
                    ..Default::default()
                }),
            );
            let deployment = router.get_deployment(&id).unwrap();
            if let Some(pool) = &pool {
                let mut connection = pool.open_live_connection().await.unwrap();
                let (seconds, _): (u64, u64) = redis::cmd("TIME")
                    .query_async(&mut connection)
                    .await
                    .unwrap();
                if seconds % 60 >= 55 {
                    tokio::time::sleep(Duration::from_secs(61 - seconds % 60)).await;
                }
            }
            if first_is_stream {
                let (_, lease) = super::super::execute_stream_with_selected_deployment(
                    router.clone(),
                    "gateway-admission-alias",
                    ProviderCapability::ChatCompletionStream,
                    tokens,
                    |_, _, _| async { Ok::<_, ProviderError>(()) },
                )
                .await
                .unwrap();
                assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
                if let Some(pool) = &pool {
                    assert_shared_admission(
                        pool,
                        &deployment,
                        (1, 1, i64::try_from(tokens).unwrap()),
                    )
                    .await;
                }
                assert_second_dispatch_blocked(&router, tokens).await;
                lease.finish_success(0).await;
            } else {
                let entered = Arc::new(tokio::sync::Notify::new());
                let release = Arc::new(tokio::sync::Notify::new());
                let entered_call = entered.clone();
                let release_call = release.clone();
                let mut first = Box::pin(super::super::execute_with_selected_deployment(
                    &router,
                    "gateway-admission-alias",
                    ProviderCapability::ChatCompletion,
                    tokens,
                    move |_, _, _| {
                        let entered = entered_call.clone();
                        let release = release_call.clone();
                        async move {
                            entered.notify_one();
                            release.notified().await;
                            Ok::<_, ProviderError>(((), 0))
                        }
                    },
                ));
                tokio::select! {
                    result = &mut first => panic!("first request finished before release: {result:?}"),
                    _ = entered.notified() => {}
                }
                assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
                if let Some(pool) = &pool {
                    assert_shared_admission(
                        pool,
                        &deployment,
                        (1, 1, i64::try_from(tokens).unwrap()),
                    )
                    .await;
                }
                assert_second_dispatch_blocked(&router, tokens).await;
                release.notify_one();
                first.await.unwrap();
            }
            assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
            assert_eq!(deployment.state.tpm_current.load(Ordering::Relaxed), 0);
            if let Some(pool) = &pool {
                assert_shared_admission(pool, &deployment, (0, 1, 0)).await;
            }
            let (_, positive) = super::super::execute_stream_with_selected_deployment(
                router.clone(),
                "gateway-admission-alias",
                ProviderCapability::ChatCompletionStream,
                tokens,
                |_, _, _| async { Ok::<_, ProviderError>(()) },
            )
            .await
            .expect("known-zero settlement must permit the next reservation");
            positive.finish_success(0).await;
            if let Some(pool) = &pool {
                assert_shared_admission(pool, &deployment, (0, 2, 0)).await;
                let mut connection = pool.open_live_connection().await.unwrap();
                let key = RedisPool::admission_key(&deployment.shared_state_id());
                let _: i64 = redis::cmd("DEL")
                    .arg(key)
                    .query_async(&mut connection)
                    .await
                    .unwrap();
            }
        }
    }
}

#[test]
fn gateway_admission_respects_the_effective_key_cap_without_changing_explicit_bounds() {
    let mut input = request();
    input.max_tokens = Some(0);
    let prompt = chat(&input, 0).unwrap();
    input.max_tokens = None;
    assert_eq!(
        chat_with_key_limit(&input, &[], 0, Some(16)).unwrap(),
        prompt + 16
    );
    assert!(chat(&input, 0).unwrap() > prompt + 16);
    input.n = Some(2);
    assert_eq!(
        chat_with_key_limit(&input, &[], 40, Some(16)).unwrap(),
        prompt + 72
    );
    input.max_tokens = Some(8);
    assert_eq!(
        chat_with_key_limit(&input, &[], 0, Some(16)).unwrap(),
        prompt + 16
    );
    input.max_completion_tokens = Some(4);
    assert_eq!(
        chat_with_key_limit(&input, &[], 0, Some(16)).unwrap(),
        prompt + 16
    );
}

#[test]
fn native_media_reserves_existing_floors_and_encoded_payloads_once() {
    let small_images = [
        serde_json::json!({"type":"image", "source":{"type":"base64", "media_type":"image/png", "data":"iVBORw=="}}),
        serde_json::json!({"type":"image", "source":{"type":"url", "url":"https://example.test/image.png"}}),
        serde_json::json!({"inlineData":{"mimeType":"image/png", "data":"iVBORw=="}}),
        serde_json::json!({"fileData":{"mimeType":"image/png", "fileUri":"gs://bucket/image.png"}}),
    ];
    for part in small_images {
        let body = serde_json::json!({"messages":[{"role":"user", "content":[part]}]});
        let original = body.clone();
        let tokens = json_completion(&body, "gateway-alias", Some(0)).unwrap();
        assert!(
            tokens >= 1_105,
            "{tokens}: media cannot be admitted as a short URL or base64 string"
        );
        assert_eq!(body, original);
    }
    let encoded = "A".repeat(16_000);
    for part in [
        serde_json::json!({"type":"image", "source":{"type":"base64", "media_type":"image/png", "data":encoded}}),
        serde_json::json!({"type":"document", "source":{"type":"base64", "media_type":"application/pdf", "data":encoded}}),
        serde_json::json!({"inlineData":{"mimeType":"audio/wav", "data":encoded}}),
    ] {
        let body = serde_json::json!({"contents":[{"parts":[part]}]});
        let original = body.clone();
        let input = json_completion(&body, "gateway-alias", Some(0)).unwrap();
        assert!(
            (4_000..5_000).contains(&input),
            "{input}: encoded payload uses the existing media policy exactly once"
        );
        assert_eq!(
            json_completion_with_candidates(&body, "gateway-alias", Some(3_000), 2).unwrap(),
            input + 6_000
        );
        assert_eq!(body, original);
    }
}
