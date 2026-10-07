//! Opt-in construction measurements through the real runtime apply path.

use super::AppState;
use crate::server::HttpServer;
use std::collections::HashMap;
use std::time::Instant;

const MODEL: &str = "gpt-4o-mini";
const WARMUP_APPLIES: u32 = 3;
const MEASURED_APPLIES: u32 = 30;

async fn profile_state(count: usize) -> AppState {
    let mut config = crate::server::valid_test_config();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config.gateway.cache.enabled = false;
    config.gateway.router.load_balancer.health_check_enabled = false;
    let mut template = config.gateway.providers[0].clone();
    template.models = vec![MODEL.into()];
    template.api_key = "sk-runtime-profile-synthetic".into();
    template.base_url = Some("https://api.openai.com/v1".into());
    template.weight = 1.0;
    template.rpm = 1000;
    template.timeout = 30;
    config.gateway.providers = (0..count)
        .map(|index| {
            let mut provider = template.clone();
            provider.name = format!("profile-{index}");
            provider
        })
        .collect();
    let state = HttpServer::new(&config).await.unwrap().state().clone();
    assert!(state.response_cache().is_none());
    assert!(state.config_sync.is_none());
    state
}

fn latency(values: &[f64]) -> serde_json::Value {
    let mut ordered = values.to_vec();
    ordered.sort_by(f64::total_cmp);
    let percentile = |value: f64| ordered[(value * ordered.len() as f64).ceil() as usize - 1];
    serde_json::json!({
        "count": values.len(),
        "p50_ms": percentile(0.50),
        "p95_ms": percentile(0.95),
        "max_ms": ordered.last().unwrap(),
        "samples_ms": values,
    })
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "opt-in runtime construction profile; no upstream requests or performance threshold"]
async fn profile_runtime_reload_construction() {
    for count in [1, 10, 100] {
        for scenario in ["routing_policy", "credential", "endpoint", "timeout"] {
            let state = profile_state(count).await;
            let mut samples = Vec::new();
            let mut constructed = Vec::new();
            for iteration in 0..WARMUP_APPLIES + MEASURED_APPLIES {
                let revision = iteration + 1;
                // Keep the preceding revision pinned during apply, as an
                // in-flight request can. Its eventual Drop is outside timing.
                let before = state.pin_runtime();
                let mut identities = HashMap::new();
                for index in 0..count {
                    let id = format!("profile-{index}-{MODEL}");
                    let deployment = before.unified_router.get_deployment(&id).unwrap();
                    identities.insert(id, deployment.state.provider_instance_identity());
                }
                let mut candidate = (*state.config()).clone();
                for provider in &mut candidate.gateway.providers {
                    match scenario {
                        "routing_policy" => {
                            provider.weight = 1.0 + (revision % 2) as f32;
                            provider.rpm = 1000 + revision;
                        }
                        "credential" => {
                            provider.api_key = format!("sk-runtime-profile-synthetic-{revision}");
                        }
                        "endpoint" => {
                            provider.base_url = Some(format!(
                                "https://api.openai.com/v1/profile-{}",
                                revision % 2
                            ));
                        }
                        "timeout" => provider.timeout = 30 + u64::from(revision),
                        _ => unreachable!(),
                    }
                }
                if scenario == "routing_policy" {
                    // Toggle one alias; growing the alias map every sample
                    // would mix configuration growth into reload latency.
                    candidate.gateway.model_aliases.clear();
                    if revision % 2 == 1 {
                        candidate
                            .gateway
                            .model_aliases
                            .insert("profile-alias".into(), MODEL.into());
                    }
                }
                let start = Instant::now();
                state.apply_runtime(candidate).await.unwrap();
                let milliseconds = start.elapsed().as_secs_f64() * 1000.0;
                let after = state.pin_runtime();
                assert_eq!(after.generation, before.generation + 1);
                let new_instances = identities
                    .iter()
                    .filter(|(id, previous)| {
                        let deployment = after.unified_router.get_deployment(id).unwrap();
                        deployment.state.provider_instance_identity() != **previous
                    })
                    .count();
                assert_eq!(identities.len(), count);
                if scenario != "routing_policy" {
                    assert_eq!(new_instances, count, "connection inputs must reconstruct");
                }
                if iteration >= WARMUP_APPLIES {
                    samples.push(milliseconds);
                    constructed.push(new_instances);
                }
            }
            println!(
                "RUNTIME_RELOAD_PROFILE {}",
                serde_json::json!({
                    "providers": count,
                    "models_per_provider": 1,
                    "scenario": scenario,
                    "warmup_applies": WARMUP_APPLIES,
                    "apply_latency": latency(&samples),
                    "new_logical_provider_instances": constructed,
                    "debug_assertions": cfg!(debug_assertions),
                    "os": std::env::consts::OS,
                    "arch": std::env::consts::ARCH,
                    "tokio_runtime": "current_thread",
                    "scope": "real AppState::apply_runtime with the previous revision pinned; synthetic OpenAI credentials, no upstream call, database/Redis/response cache/provider probes disabled; startup, candidate cloning, identity inspection and old revision Drop are outside timing; logical instances do not prove new TCP/TLS connections",
                })
            );
        }
    }
}
