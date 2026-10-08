//! Behavioral reloads retain quota/circuit state; resource rotations isolate it.

use super::GatewayRuntimeIdentity;
use crate::config::models::provider::ProviderConfig;
use serde_json::{Value, json};

fn config() -> ProviderConfig {
    ProviderConfig {
        name: "gemini-resource-settings".into(),
        provider_type: "gemini".into(),
        api_key: "resource-settings-test-key-1234567890".into(),
        models: vec!["gemini-2.5-flash".into()],
        rpm: 100,
        tpm: 1_000,
        max_concurrent_requests: 100,
        ..ProviderConfig::default()
    }
}

fn behavior_settings() -> [(&'static str, Value, Value); 8] {
    [
        ("timeout", json!(60), json!(90)),
        ("connect_timeout", json!(10), json!(20)),
        ("max_retries", json!(1), json!(2)),
        ("retry_delay_ms", json!(1_000), json!(2_000)),
        ("enable_caching", json!(true), json!(false)),
        ("cache_ttl_seconds", json!(300), json!(600)),
        ("enable_search_grounding", json!(false), json!(true)),
        ("debug", json!(false), json!(true)),
    ]
}

#[test]
fn gemini_behavior_settings_do_not_define_the_resource() {
    for selector in ["gemini", "google-gemini", "google_ai", "google-ai"] {
        let mut base = config();
        base.provider_type = selector.into();
        let original = GatewayRuntimeIdentity::for_provider(&base);
        for (key, before, after) in behavior_settings() {
            for value in [before, after] {
                let mut changed = base.clone();
                changed.settings.insert(key.into(), value);
                assert_eq!(
                    original,
                    GatewayRuntimeIdentity::for_provider(&changed),
                    "{selector}: adding or changing {key} preserves identity"
                );
            }
        }
    }
}

#[test]
fn known_factory_execution_options_do_not_define_resources() {
    // Both native enum dispatch and catalog-only selectors consume the common
    // options. Include aliases so policy follows the same factory resolution.
    for selector in [
        "openai",
        "anthropic",
        "aws-bedrock",
        "mistral",
        "azure",
        "azure_ai",
        "cloudflare",
        "bedrock",
        "vertex_ai",
        "github_copilot",
        "cohere",
        "fal_ai",
        "replicate",
        "ollama",
        "deepgram",
        "elevenlabs",
        "stability",
        "black_forest_labs",
        "databricks",
        "snowflake",
        "oci",
        "watsonx",
        "sagemaker",
        "openai_compatible",
        "groq",
        "aiml_api",
        "vllm",
    ] {
        let mut base = config();
        base.provider_type = selector.into();
        let original = GatewayRuntimeIdentity::for_provider(&base);
        for key in ["timeout", "max_retries"] {
            for value in [1, 2] {
                let mut changed = base.clone();
                changed.settings.insert(key.into(), json!(value));
                assert_eq!(
                    original,
                    GatewayRuntimeIdentity::for_provider(&changed),
                    "{selector}: {key}"
                );
            }
        }
    }
    for (selector, settings) in [
        (
            "anthropic",
            json!({"connect_timeout": 5, "retry_delay_base": 2,
            "enable_multimodal": false, "enable_cache_control": false,
            "enable_computer_use": true, "enable_experimental": true,
            "allow_unknown_models": true, "models": ["claude-fixture"],
            "multimodal_models": ["claude-fixture"]}),
        ),
        ("cloudflare", json!({"debug": true})),
        ("bedrock", json!({"timeout_seconds": 20})),
        ("vertex_ai", json!({"enable_experimental": true})),
        (
            "github_copilot",
            json!({"disable_system_to_assistant": true, "debug": true}),
        ),
        (
            "cohere",
            json!({"timeout_seconds": 20, "default_embedding_input_type": "search_query"}),
        ),
        (
            "fal_ai",
            json!({"output_format": "jpeg", "sync_mode": true}),
        ),
        (
            "replicate",
            json!({"timeout_seconds": 20, "polling_delay_seconds": 2,
            "polling_retries": 3, "use_streaming": true}),
        ),
        (
            "ollama",
            json!({"debug": true, "mirostat": 2, "mirostat_eta": 0.2,
            "mirostat_tau": 5.0, "num_ctx": 1024, "num_gqa": 1, "num_gpu": 0,
            "num_thread": 4, "repeat_last_n": 64, "repeat_penalty": 1.1,
            "tfs_z": 0.5, "system": "instruction", "template": "prompt", "keep_alive": "5m"}),
        ),
        ("openai_compatible", json!({"pass_through_params": true})),
        ("groq", json!({"pass_through_params": true})),
        ("aiml_api", json!({"pass_through_params": true})),
    ] {
        let mut base = config();
        base.provider_type = selector.into();
        let original = GatewayRuntimeIdentity::for_provider(&base);
        for (key, value) in settings.as_object().unwrap() {
            let mut changed = base.clone();
            changed.settings.insert(key.clone(), value.clone());
            assert_eq!(
                original,
                GatewayRuntimeIdentity::for_provider(&changed),
                "{selector}: {key}"
            );
        }
        // Known behavior names still belong to unknown/custom provider settings.
        base.provider_type = "custom-provider".into();
        let custom = GatewayRuntimeIdentity::for_provider(&base);
        base.settings = serde_json::from_value(settings).unwrap();
        assert_ne!(custom, GatewayRuntimeIdentity::for_provider(&base));
    }
    for (selector, key) in [
        ("openai", "debug"),
        ("vertex_ai", "retry_delay_ms"),
        ("github_copilot", "enable_caching"),
        ("groq", "connect_timeout"),
    ] {
        let mut base = config();
        base.provider_type = selector.into();
        let original = GatewayRuntimeIdentity::for_provider(&base);
        base.settings.insert(key.into(), json!(true));
        assert_ne!(
            original,
            GatewayRuntimeIdentity::for_provider(&base),
            "unknown {selector} key {key} stays conservative"
        );
    }
}

#[test]
fn resource_settings_and_native_credentials_remain_identity_inputs() {
    let base = config();
    let original = GatewayRuntimeIdentity::for_provider(&base);
    for selector in [
        "gemini",
        "openai",
        "anthropic",
        "mistral",
        "groq",
        "aiml_api",
        "bedrock",
        "vertex_ai",
        "github_copilot",
    ] {
        let mut resource = base.clone();
        resource.provider_type = selector.into();
        resource.api_key.clear();
        let original = GatewayRuntimeIdentity::for_provider(&resource);
        for (key, value) in [
            ("api_key", json!("setting-account")),
            ("google_api_key", json!("google-account")),
            ("gemini_api_key", json!("gemini-account")),
            ("base_url", json!("https://resource.invalid")),
            ("api_base", json!("https://resource.invalid")),
            ("api_version", json!("v2")),
            ("project_id", json!("project")),
            ("location", json!("region")),
            ("service_account_json", json!({"client_email": "account"})),
            ("headers", json!({"Authorization": "account"})),
            ("custom_headers", json!({"x-goog-user-project": "project"})),
            ("proxy", json!("https://proxy.invalid")),
            ("proxy_url", json!("https://proxy.invalid")),
            ("future_account_setting", json!("unknown-resource")),
            ("account_id", json!("cloudflare-account")),
            ("auth", json!({"type": "api_key", "token": "account"})),
            ("token_dir", json!("/credential-directory")),
            ("access_token_file", json!("access-token")),
            ("api_key_file", json!("api-key.json")),
            ("model_mappings", json!({"alias": "underlying-model"})),
            ("model_prefix", json!("deployment-prefix")),
            ("default_model", json!("deployment-model")),
            ("skip_api_key", json!(true)),
        ] {
            let mut changed = resource.clone();
            changed.settings.insert(key.into(), value);
            assert_ne!(
                original,
                GatewayRuntimeIdentity::for_provider(&changed),
                "{selector}: {key}"
            );
        }
    }
    let mut custom = base.clone();
    custom.provider_type = "custom-provider".into();
    let identity = GatewayRuntimeIdentity::for_provider(&custom);
    for (key, _, value) in behavior_settings() {
        let mut changed = custom.clone();
        changed.settings.insert(key.into(), value);
        assert_ne!(
            identity,
            GatewayRuntimeIdentity::for_provider(&changed),
            "custom: {key}"
        );
    }
    for field in [
        "api_key",
        "base_url",
        "organization",
        "project",
        "api_version",
    ] {
        let mut changed = base.clone();
        match field {
            "api_key" => changed.api_key = "rotated-account".into(),
            "base_url" => changed.base_url = Some("https://resource.invalid".into()),
            "organization" => changed.organization = Some("organization".into()),
            "project" => changed.project = Some("project".into()),
            "api_version" => changed.api_version = Some("v2".into()),
            _ => unreachable!(),
        }
        assert_ne!(
            original,
            GatewayRuntimeIdentity::for_provider(&changed),
            "{field}"
        );
    }
    assert_ne!(
        original.with_credential_digest([1; 32]),
        original.with_credential_digest([2; 32])
    );
    assert_ne!(original.for_model("first"), original.for_model("second"));
    assert_eq!(
        format!("{original:?}"),
        "GatewayRuntimeIdentity([REDACTED])"
    );
}

#[cfg(feature = "gateway")]
mod gateway {
    use super::*;
    use crate::config::models::storage::RedisConfig;
    use crate::core::router::deployment::current_timestamp;
    use crate::core::router::selection::DeploymentLease;
    use crate::core::router::{CooldownReason, RouterConfig, RouterError, UnifiedRouter as Router};
    use crate::core::types::model::ProviderCapability;
    use crate::storage::redis::RedisPool;
    use std::sync::Arc;
    use std::sync::atomic::Ordering::Acquire;
    use std::time::Duration;

    async fn construct(config: &ProviderConfig, pool: Option<&Arc<RedisPool>>) -> Router {
        let router = Router::from_gateway_config(
            std::slice::from_ref(config),
            Some(RouterConfig {
                enable_pre_call_checks: false,
                cooldown_time_secs: 60,
                ..RouterConfig::default()
            }),
        )
        .await
        .unwrap();
        match pool {
            Some(pool) => router
                .with_admission_redis(pool.clone())
                .with_circuit_redis(pool.clone()),
            None => router,
        }
    }

    async fn select(router: &Router, model: &str) -> Result<DeploymentLease, RouterError> {
        router
            .select_deployment_lease_for_capability_matching_with_estimate_async(
                model,
                &ProviderCapability::ChatCompletion,
                40,
                |_| true,
            )
            .await
    }

    async fn wait_for_window(pool: Option<&Arc<RedisPool>>) {
        loop {
            let local = current_timestamp() % 60;
            let shared = if let Some(pool) = pool {
                let mut connection = pool.open_live_connection().await.unwrap();
                let (seconds, _micros): (u64, u64) = redis::cmd("TIME")
                    .query_async(&mut connection)
                    .await
                    .unwrap();
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
                None => return,
            }
        }
    }

    async fn shared_counts(pool: &RedisPool, id: &str) -> (u64, u64, u64) {
        let mut connection = pool.open_live_connection().await.unwrap();
        redis::cmd("HMGET")
            .arg(RedisPool::admission_key(id))
            .arg(&["p", "r", "t"])
            .query_async(&mut connection)
            .await
            .unwrap()
    }

    async fn exercise_reload(selector: &str, pool: Option<&Arc<RedisPool>>) {
        for limit in ["rpm", "tpm", "cooldown"] {
            wait_for_window(pool).await;
            let mut original_config = config();
            original_config.name = format!("resource-settings-{}", uuid::Uuid::new_v4());
            original_config.provider_type = selector.into();
            if selector == "openai" {
                original_config.api_key = "sk-resource-settings-test-key".into();
                original_config.models = vec!["gpt-4o".into()];
            }
            if limit == "rpm" {
                original_config.rpm = 1;
            } else if limit == "tpm" {
                original_config.tpm = 64;
            }
            // Policy-aware clients require connect_timeout to remain 10.
            // Its 10/20 identity classification is tested without construction.
            let changes = behavior_settings()
                .into_iter()
                .filter(|(key, _, _)| {
                    *key != "connect_timeout"
                        && (selector == "gemini" || matches!(*key, "timeout" | "max_retries"))
                })
                .collect::<Vec<_>>();
            for (key, value, _) in &changes {
                original_config
                    .settings
                    .insert((*key).into(), value.clone());
            }
            let model = &original_config.models[0];
            let id = format!("{}-{model}", original_config.name);
            let original = construct(&original_config, pool).await;
            let old = original.get_deployment(&id).unwrap();
            assert_eq!(
                old.provider.provider_type().to_string(),
                selector,
                "use the actual native factory"
            );
            let old_shared_id = old.shared_state_id();
            let mut held = select(&original, model).await.unwrap();
            if let Some(pool) = pool {
                assert_eq!(shared_counts(pool, &old_shared_id).await, (1, 1, 40));
            }

            let mut behavior = original_config.clone();
            for (key, _, value) in &changes {
                behavior.settings.insert((*key).into(), value.clone());
            }
            let mut reloaded = construct(&behavior, pool).await;
            reloaded.circuit = original.circuit.clone();
            reloaded.inherit_runtime_state(&original);
            let same = reloaded.get_deployment(&id).unwrap();
            assert_eq!(
                same.shared_state_id(),
                old_shared_id,
                "{limit}: same namespace"
            );
            assert!(std::ptr::eq(
                &same.state.tpm_current,
                &old.state.tpm_current
            ));
            assert_eq!(same.state.active_requests.load(Acquire), 1);

            // Complete on the old snapshot after publication. The new snapshot
            // must see the same live counters, rather than a copied value.
            held.record_success(35, 1);
            held.commit_admission_async(35).await;
            drop(held);
            original
                .record_success_circuit_for_deployment_async(&old)
                .await;
            if limit == "cooldown" {
                original
                    .record_failure_with_reason_for_deployment_async(
                        &old,
                        CooldownReason::AuthError,
                    )
                    .await;
                assert!(same.is_in_cooldown());
                assert_eq!(same.state.fail_requests.load(Acquire), 1);
            }
            let minute = same.state.minute_counters(current_timestamp());
            assert_eq!((minute.rpm, minute.tpm), (1, 35));
            assert_eq!(same.state.active_requests.load(Acquire), 0);
            assert!(
                select(&reloaded, model).await.is_err(),
                "{limit}: behavior reload must stay blocked"
            );
            if let Some(pool) = pool {
                assert_eq!(shared_counts(pool, &old_shared_id).await, (0, 1, 35));
                // A separate replica has no inherited local state. Its denial
                // proves the preserved identity also reaches real Redis.
                let peer = construct(&behavior, Some(pool)).await;
                assert!(
                    select(&peer, model).await.is_err(),
                    "{limit}: shared namespace must stay blocked"
                );
            }

            for rotation in ["account", "endpoint"] {
                let mut changed = behavior.clone();
                if rotation == "account" {
                    changed.api_key = "sk-rotated-resource-settings-key-1234567890".into();
                } else {
                    changed.base_url = Some("https://gemini-rotation.invalid".into());
                }
                let mut rotated = construct(&changed, pool).await;
                rotated.circuit = reloaded.circuit.clone();
                rotated.inherit_runtime_state(&reloaded);
                let fresh = rotated.get_deployment(&id).unwrap();
                assert_ne!(fresh.shared_state_id(), old_shared_id, "{rotation}");
                assert!(!std::ptr::eq(
                    &fresh.state.tpm_current,
                    &old.state.tpm_current
                ));
                let minute = fresh.state.minute_counters(current_timestamp());
                assert_eq!((minute.rpm, minute.tpm, minute.failures), (0, 0, 0));
                assert!(!fresh.is_in_cooldown());
                let mut lease = select(&rotated, model)
                    .await
                    .expect("a new resource has its own capacity");
                let fresh_id = lease.deployment().shared_state_id();
                if let Some(pool) = pool {
                    assert_eq!(shared_counts(pool, &fresh_id).await, (1, 1, 40));
                }
                lease.cancel_admission_async().await;
                drop(lease);
                if let Some(pool) = pool {
                    assert_eq!(shared_counts(pool, &fresh_id).await, (0, 0, 0));
                    pool.delete(&RedisPool::admission_key(&fresh_id))
                        .await
                        .unwrap();
                    pool.delete(&RedisPool::circuit_key(&fresh_id))
                        .await
                        .unwrap();
                }
            }
            assert!(
                select(&reloaded, model).await.is_err(),
                "rotations must not reset the old resource"
            );
            if let Some(pool) = pool {
                assert_eq!(shared_counts(pool, &old_shared_id).await, (0, 1, 35));
                pool.delete(&RedisPool::admission_key(&old_shared_id))
                    .await
                    .unwrap();
                pool.delete(&RedisPool::circuit_key(&old_shared_id))
                    .await
                    .unwrap();
            }
        }
    }

    #[cfg(feature = "providers-extended")]
    #[tokio::test]
    async fn gemini_behavior_reload_preserves_local_quota_and_cooldown() {
        exercise_reload("gemini", None).await;
    }

    #[tokio::test]
    async fn openai_behavior_reload_preserves_local_quota_and_cooldown() {
        exercise_reload("openai", None).await;
    }

    async fn exercise_databricks_workspace_reload(pool: Option<&Arc<RedisPool>>) {
        use crate::core::net::ProviderEndpointAccess;
        use crate::core::router::health_probe::tests::sequence_server;
        use crate::core::types::context::RequestContext;

        // Real provider requests identify which workspace the enterprise
        // factory selected. A local 400 response avoids retries or paid calls.
        let (workspace_a, mut calls_a, task_a) = sequence_server(vec![400]).await;
        let (workspace_b, mut calls_b, task_b) = sequence_server(vec![400, 400]).await;
        let mut original_config = config();
        original_config.name = format!("databricks-workspace-{}", uuid::Uuid::new_v4());
        original_config.provider_type = "databricks".into();
        original_config.models = vec!["deployment".into()];
        original_config.base_url = Some("https://unused-fallback.invalid".into());
        original_config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        original_config.settings = serde_json::from_value(json!({
            "workspace_url": workspace_a.origin().ascii_serialization(),
            "base_url": "https://unused-base.invalid",
            "api_base": "https://unused-api.invalid"
        }))
        .unwrap();
        let id = format!("{}-deployment", original_config.name);
        let request = || {
            serde_json::from_value(json!({
                "model": "deployment",
                "messages": [{"role": "user", "content": "workspace probe"}]
            }))
            .unwrap()
        };
        wait_for_window(pool).await;
        let original = construct(&original_config, pool).await;
        let old = original.get_deployment(&id).unwrap();
        tokio::time::timeout(
            Duration::from_secs(5),
            old.provider
                .chat_completion(request(), RequestContext::default()),
        )
        .await
        .unwrap()
        .expect_err("local workspace A responds with 400");
        calls_a.try_recv().expect("provider must reach workspace A");
        let old_shared_id = old.shared_state_id();
        let mut completed = select(&original, "deployment").await.unwrap();
        completed.record_success(35, 1);
        completed.commit_admission_async(35).await;
        drop(completed);
        original
            .record_success_circuit_for_deployment_async(&old)
            .await;
        let mut held = select(&original, "deployment").await.unwrap();
        original
            .record_failure_with_reason_for_deployment_async(&old, CooldownReason::AuthError)
            .await;
        assert!(old.is_in_cooldown());
        assert_eq!(old.state.active_requests.load(Acquire), 1);
        if let Some(pool) = pool {
            assert_eq!(shared_counts(pool, &old_shared_id).await, (1, 2, 75));
        }

        let mut rotated_config = original_config.clone();
        rotated_config.settings.insert(
            "workspace_url".into(),
            json!(workspace_b.origin().ascii_serialization()),
        );
        let mut rotated = construct(&rotated_config, pool).await;
        rotated.circuit = original.circuit.clone();
        rotated.inherit_runtime_state(&original);
        let fresh = rotated.get_deployment(&id).unwrap();
        tokio::time::timeout(
            Duration::from_secs(5),
            fresh
                .provider
                .chat_completion(request(), RequestContext::default()),
        )
        .await
        .unwrap()
        .expect_err("local workspace B responds with 400");
        calls_b.try_recv().expect("provider must reach workspace B");
        assert_ne!(fresh.state.runtime_identity, old.state.runtime_identity);
        assert_ne!(fresh.shared_state_id(), old_shared_id);
        assert!(!std::ptr::eq(
            &fresh.state.tpm_current,
            &old.state.tpm_current
        ));
        let minute = fresh.state.minute_counters(current_timestamp());
        assert_eq!((minute.rpm, minute.tpm, minute.failures), (0, 0, 0));
        assert_eq!(fresh.state.active_requests.load(Acquire), 0);
        assert!(!fresh.is_in_cooldown());
        let mut lease = select(&rotated, "deployment")
            .await
            .expect("workspace B must have independent capacity and circuit state");
        lease.record_success(20, 1);
        lease.commit_admission_async(20).await;
        drop(lease);

        // A changed generic base URL is ignored while workspace_url remains
        // explicit. Preserve B's state instead of resetting its spent quota.
        let mut shadowed_config = rotated_config;
        shadowed_config.base_url = Some("https://changed-unused-fallback.invalid".into());
        let mut shadowed = construct(&shadowed_config, pool).await;
        shadowed.circuit = rotated.circuit.clone();
        shadowed.inherit_runtime_state(&rotated);
        let same = shadowed.get_deployment(&id).unwrap();
        tokio::time::timeout(
            Duration::from_secs(5),
            same.provider
                .chat_completion(request(), RequestContext::default()),
        )
        .await
        .unwrap()
        .expect_err("the changed fallback still reaches local workspace B");
        calls_b.try_recv().expect("shadowed base must still use B");
        assert_eq!(same.state.runtime_identity, fresh.state.runtime_identity);
        assert_eq!(same.shared_state_id(), fresh.shared_state_id());
        assert!(std::ptr::eq(
            &same.state.tpm_current,
            &fresh.state.tpm_current
        ));
        let minute = same.state.minute_counters(current_timestamp());
        assert_eq!((minute.rpm, minute.tpm), (1, 20));
        assert_eq!(old.state.active_requests.load(Acquire), 1);
        assert!(old.is_in_cooldown());
        assert!(select(&original, "deployment").await.is_err());
        held.cancel_admission_async().await;
        drop(held);
        assert_eq!(old.state.active_requests.load(Acquire), 0);
        let minute = old.state.minute_counters(current_timestamp());
        assert_eq!((minute.rpm, minute.tpm, minute.failures), (1, 35, 1));
        if let Some(pool) = pool {
            assert_eq!(shared_counts(pool, &old_shared_id).await, (0, 1, 35));
            assert_eq!(
                shared_counts(pool, &same.shared_state_id()).await,
                (0, 1, 20)
            );
            for shared_id in [old_shared_id, same.shared_state_id()] {
                pool.delete(&RedisPool::admission_key(&shared_id))
                    .await
                    .unwrap();
                pool.delete(&RedisPool::circuit_key(&shared_id))
                    .await
                    .unwrap();
            }
        }
        task_a.await.unwrap();
        task_b.await.unwrap();
    }

    #[tokio::test]
    async fn databricks_workspace_reload_isolates_local_state() {
        exercise_databricks_workspace_reload(None).await;
    }

    #[tokio::test]
    async fn databricks_workspace_reload_isolates_actual_redis_state() {
        if let Some(pool) = live_redis_pool().await {
            exercise_databricks_workspace_reload(Some(&pool)).await;
        }
    }

    async fn live_redis_pool() -> Option<Arc<RedisPool>> {
        let url = match std::env::var("REDIS_URL") {
            Ok(url) => url,
            Err(_) if std::env::var_os("CI").is_some() => panic!("CI requires actual REDIS_URL"),
            Err(_) => {
                eprintln!("Skipping shared resource-settings regression: REDIS_URL is not set");
                return None;
            }
        };
        let pool = RedisPool::new(&RedisConfig {
            url,
            enabled: true,
            allow_degraded: false,
            ..RedisConfig::default()
        })
        .await
        .unwrap();
        assert!(!pool.is_noop());
        pool.health_check().await.unwrap();
        Some(Arc::new(pool))
    }

    #[cfg(feature = "providers-extended")]
    #[tokio::test]
    async fn gemini_behavior_reload_preserves_actual_redis_quota_and_cooldown() {
        if let Some(pool) = live_redis_pool().await {
            exercise_reload("gemini", Some(&pool)).await;
        }
    }

    #[tokio::test]
    async fn openai_behavior_reload_preserves_actual_redis_quota_and_cooldown() {
        if let Some(pool) = live_redis_pool().await {
            exercise_reload("openai", Some(&pool)).await;
        }
    }
}

#[test]
fn shadowed_factory_inputs_do_not_rotate_runtime_resource() {
    let mut base = config();
    base.provider_type = "openai".into();
    base.base_url = Some("https://resource.example/v1".into());
    base.api_version = Some("version-one".into());
    base.organization = Some("organization-one".into());
    base.project = Some("project-one".into());
    let original = GatewayRuntimeIdentity::for_provider(&base);
    for key in [
        "base_url",
        "api_base",
        "api_key",
        "api_version",
        "organization",
        "project",
    ] {
        let mut changed = base.clone();
        changed.settings.insert(key.into(), json!("shadowed-value"));
        assert_eq!(
            original,
            GatewayRuntimeIdentity::for_provider(&changed),
            "{key}"
        );
    }
}

#[test]
fn databricks_workspace_alias_precedence_defines_resource_identity() {
    let mut base = config();
    base.provider_type = "databricks".into();
    base.base_url = Some("https://fallback.example".into());
    base.settings = serde_json::from_value(json!({
        "workspace_url": "https://workspace-a.example",
        "base_url": "https://shadowed-base.example",
        "api_base": "https://shadowed-api.example"
    }))
    .unwrap();
    let original = GatewayRuntimeIdentity::for_provider(&base);
    let mut rotated = base.clone();
    rotated
        .settings
        .insert("workspace_url".into(), json!("https://workspace-b.example"));
    assert_ne!(
        original,
        GatewayRuntimeIdentity::for_provider(&rotated),
        "the enterprise builder uses workspace_url before every generic endpoint alias"
    );
    for shadowed in ["top-level base_url", "base_url", "api_base"] {
        let mut unchanged = base.clone();
        if shadowed == "top-level base_url" {
            unchanged.base_url = Some("https://unused-rotation.example".into());
        } else {
            unchanged
                .settings
                .insert(shadowed.into(), json!("https://unused-rotation.example"));
        }
        assert_eq!(
            original,
            GatewayRuntimeIdentity::for_provider(&unchanged),
            "changing shadowed {shadowed} must preserve workspace quota"
        );
    }
    for alias in [
        "top-level base_url",
        "base_url",
        "api_base",
        "workspace_url",
    ] {
        let mut equivalent = base.clone();
        equivalent.base_url = None;
        equivalent.settings.clear();
        if alias == "top-level base_url" {
            equivalent.base_url = Some("https://workspace-a.example".into());
        } else {
            equivalent
                .settings
                .insert(alias.into(), json!("https://workspace-a.example"));
        }
        assert_eq!(
            original,
            GatewayRuntimeIdentity::for_provider(&equivalent),
            "equivalent {alias} must identify the same workspace"
        );
    }
}

#[test]
fn catalog_effective_organization_and_version_define_resource() {
    let mut base = config();
    base.provider_type = "deepseek".into();
    base.organization = Some("organization-a".into());
    base.api_version = Some("version-a".into());
    base.settings
        .insert("organization".into(), json!("organization-b"));
    base.settings
        .insert("api_version".into(), json!("version-b"));
    let original = GatewayRuntimeIdentity::for_provider(&base);
    for key in ["organization", "api_version"] {
        let mut changed = base.clone();
        changed.settings.insert(key.into(), json!("effective-c"));
        assert_ne!(
            original,
            GatewayRuntimeIdentity::for_provider(&changed),
            "{key}"
        );
        let mut shadowed = base.clone();
        if key == "organization" {
            shadowed.organization = Some("shadowed-c".into());
        } else {
            shadowed.api_version = Some("shadowed-c".into());
        }
        assert_eq!(
            original,
            GatewayRuntimeIdentity::for_provider(&shadowed),
            "shadowed {key}"
        );
    }
}
