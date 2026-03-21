//! Performance benchmarks for litellm-rs
//!
//! This module contains comprehensive benchmarks to measure the performance
//! of various components in the litellm-rs system.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use litellm_rs::core::models::openai::*;
use litellm_rs::core::providers::Provider;
use litellm_rs::core::providers::openai::OpenAIProvider;
use litellm_rs::core::router::{
    Deployment, DeploymentConfig, RouterConfig, UnifiedRouter, UnifiedRoutingStrategy,
};
use std::hint::black_box;

use std::sync::Arc;
use tokio::runtime::Runtime;

/// Helper to create a test provider for benchmarks
fn create_test_provider(rt: &Runtime) -> Provider {
    rt.block_on(async {
        let openai = OpenAIProvider::with_api_key("sk-test-key-for-benchmarking")
            .await
            .expect("Failed to create OpenAI provider");
        Provider::OpenAI(openai)
    })
}

/// Helper to create test deployments for benchmarks
fn create_test_deployments(rt: &Runtime, count: usize, model_name: &str) -> Vec<Deployment> {
    // Create one provider and clone it for all deployments
    let provider = create_test_provider(rt);

    (0..count)
        .map(|i| {
            Deployment::new(
                format!("deployment-{}", i),
                provider.clone(),
                format!("{}-turbo", model_name), // model (actual)
                model_name.to_string(),          // model_name (user-facing)
            )
            .with_config(DeploymentConfig {
                tpm_limit: Some(100000),
                rpm_limit: Some(1000),
                priority: i as u32,
                weight: 1,
                ..Default::default()
            })
            .with_tags(vec![format!("tag-{}", i % 3)])
        })
        .collect()
}

/// Benchmark unified router operations - the KEY performance benchmark
fn bench_unified_router(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("unified_router");

    // Test 1: Router creation
    group.bench_function("router_creation", |b| {
        b.iter(|| black_box(UnifiedRouter::default()));
    });

    // Test 2: Add deployment
    group.bench_function("add_deployment", |b| {
        let router = UnifiedRouter::default();
        let provider = create_test_provider(&rt);
        let mut counter = 0;

        b.iter(|| {
            counter += 1;
            let deployment = Deployment::new(
                format!("bench-deployment-{}", counter),
                provider.clone(),
                "gpt-4-turbo".to_string(),
                "gpt-4".to_string(),
            )
            .with_config(DeploymentConfig {
                tpm_limit: Some(100000),
                rpm_limit: Some(1000),
                ..Default::default()
            });
            router.add_deployment(deployment);
            black_box(())
        });
    });

    // Test 3: Select deployment with different number of deployments
    for num_deployments in [1, 5, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("select_deployment", num_deployments),
            num_deployments,
            |b, &num| {
                let router = UnifiedRouter::default();
                let deployments = create_test_deployments(&rt, num, "gpt-4");
                for deployment in deployments {
                    router.add_deployment(deployment);
                }

                b.iter(|| black_box(router.select_deployment("gpt-4")));
            },
        );
    }

    // Test 4: Get healthy deployments
    for num_deployments in [1, 5, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("get_healthy_deployments", num_deployments),
            num_deployments,
            |b, &num| {
                let router = UnifiedRouter::default();
                let deployments = create_test_deployments(&rt, num, "gpt-4");
                for deployment in deployments {
                    router.add_deployment(deployment);
                }

                b.iter(|| black_box(router.get_healthy_deployments("gpt-4")));
            },
        );
    }

    // Test 5: Model alias resolution
    group.bench_function("alias_resolution", |b| {
        let router = UnifiedRouter::default();
        router.add_model_alias("gpt4", "gpt-4").unwrap();
        router.add_model_alias("claude3", "claude-3-opus").unwrap();
        router.add_model_alias("openai/gpt-4", "gpt-4").unwrap();

        // Add deployments
        let deployments = create_test_deployments(&rt, 5, "gpt-4");
        for deployment in deployments {
            router.add_deployment(deployment);
        }

        b.iter(|| black_box(router.resolve_model_name("gpt4")));
    });

    // Test 6: Different routing strategies
    for strategy in [
        UnifiedRoutingStrategy::SimpleShuffle,
        UnifiedRoutingStrategy::LeastBusy,
        UnifiedRoutingStrategy::RoundRobin,
        UnifiedRoutingStrategy::LatencyBased,
    ]
    .iter()
    {
        group.bench_with_input(
            BenchmarkId::new("routing_strategy", format!("{:?}", strategy)),
            strategy,
            |b, strategy| {
                let config = RouterConfig {
                    routing_strategy: *strategy,
                    ..Default::default()
                };
                let router = UnifiedRouter::new(config);
                let deployments = create_test_deployments(&rt, 10, "gpt-4");
                for deployment in deployments {
                    router.add_deployment(deployment);
                }

                b.iter(|| black_box(router.select_deployment("gpt-4")));
            },
        );
    }

    // Test 7: Record success/failure (lock-free atomic operations)
    group.bench_function("record_success", |b| {
        let router = UnifiedRouter::default();
        let deployments = create_test_deployments(&rt, 1, "gpt-4");
        for deployment in deployments {
            router.add_deployment(deployment);
        }
        let deployment_id = "deployment-0";

        b.iter(|| {
            router.record_success(deployment_id, 100, 50_000); // 50ms in microseconds
            black_box(())
        });
    });

    group.bench_function("record_failure", |b| {
        let router = UnifiedRouter::default();
        let deployments = create_test_deployments(&rt, 1, "gpt-4");
        for deployment in deployments {
            router.add_deployment(deployment);
        }
        let deployment_id = "deployment-0";

        b.iter(|| {
            router.record_failure(deployment_id);
            black_box(())
        });
    });

    group.finish();
}

/// Benchmark concurrent router operations
fn bench_concurrent_router(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("concurrent_router");

    // Test concurrent select operations
    for num_tasks in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_select", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                let router = Arc::new(UnifiedRouter::default());
                let deployments = create_test_deployments(&rt, 10, "gpt-4");
                for deployment in deployments {
                    router.add_deployment(deployment);
                }

                b.iter(|| {
                    let router = router.clone();
                    rt.block_on(async move {
                        let mut handles = Vec::new();

                        for _ in 0..num_tasks {
                            let router = router.clone();
                            let handle = tokio::spawn(async move {
                                let _ = router.select_deployment("gpt-4");
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            let _ = handle.await;
                        }
                        black_box(());
                    })
                });
            },
        );
    }

    // Test concurrent select + record operations (read/write mix)
    for num_tasks in [10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_select_and_record", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                let router = Arc::new(UnifiedRouter::default());
                let deployments = create_test_deployments(&rt, 10, "gpt-4");
                for deployment in deployments {
                    router.add_deployment(deployment);
                }

                b.iter(|| {
                    let router = router.clone();
                    rt.block_on(async move {
                        let mut handles = Vec::new();

                        for i in 0..num_tasks {
                            let router = router.clone();
                            let handle = tokio::spawn(async move {
                                // Mix of reads and writes
                                if i % 3 == 0 {
                                    router.record_success(
                                        &format!("deployment-{}", i % 10),
                                        100,
                                        50_000, // 50ms in microseconds
                                    );
                                } else if i % 3 == 1 {
                                    router.record_failure(&format!("deployment-{}", i % 10));
                                } else {
                                    let _ = router.select_deployment("gpt-4");
                                }
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            handle.await.unwrap();
                        }
                        black_box(());
                    })
                });
            },
        );
    }

    group.finish();
}

/// Benchmark serialization/deserialization
fn bench_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");
    group.throughput(Throughput::Elements(1));

    let request = ChatCompletionRequest {
        model: "gpt-4".to_string(),
        messages: vec![
            ChatMessage {
                role: MessageRole::System,
                content: Some(MessageContent::Text(
                    "You are a helpful assistant.".to_string(),
                )),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            },
            ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello, how are you?".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            },
        ],
        temperature: Some(0.7),
        max_tokens: Some(150),
        max_completion_tokens: None,
        top_p: Some(1.0),
        n: Some(1),
        stream: Some(false),
        stream_options: None,
        stop: None,
        presence_penalty: Some(0.0),
        frequency_penalty: Some(0.0),
        logit_bias: None,
        user: None,
        functions: None,
        function_call: None,
        tools: None,
        tool_choice: None,
        response_format: None,
        seed: None,
        logprobs: None,
        top_logprobs: None,
        modalities: None,
        audio: None,
        reasoning_effort: None,
    };

    group.bench_function("serialize_request", |b| {
        b.iter(|| black_box(serde_json::to_string(&request).unwrap()));
    });

    let json_str = serde_json::to_string(&request).unwrap();
    group.bench_function("deserialize_request", |b| {
        b.iter(|| black_box(serde_json::from_str::<ChatCompletionRequest>(&json_str).unwrap()));
    });

    group.finish();
}

/// Benchmark concurrent operations
fn bench_concurrent_operations(c: &mut Criterion) {
    let _rt = Runtime::new().unwrap();
    let group = c.benchmark_group("concurrent_operations");

    group.finish();
}

/// Benchmark memory usage patterns
fn bench_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_usage");

    // Test memory allocation patterns
    group.bench_function("string_allocations", |b| {
        b.iter(|| {
            let mut strings = Vec::new();
            for i in 0..1000 {
                strings.push(format!("test_string_{}", i));
            }
            black_box(strings)
        });
    });

    group.bench_function("arc_allocations", |b| {
        b.iter(|| {
            let mut arcs = Vec::new();
            for i in 0..1000 {
                arcs.push(Arc::new(format!("test_string_{}", i)));
            }
            black_box(arcs)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_unified_router,
    bench_concurrent_router,
    bench_serialization,
    bench_concurrent_operations,
    bench_memory_usage
);

criterion_main!(benches);
