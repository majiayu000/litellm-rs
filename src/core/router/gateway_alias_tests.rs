use super::*;
use crate::core::router::config::{RouterConfig, RoutingStrategy};
use crate::core::types::model::ProviderCapability;

fn provider(name: &str, models: &[&str], priority: u32) -> ProviderConfig {
    ProviderConfig {
        name: name.to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-test".to_string(),
        models: models.iter().map(|model| (*model).to_string()).collect(),
        priority,
        ..ProviderConfig::default()
    }
}

fn aliases(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(alias, target)| ((*alias).to_string(), (*target).to_string()))
        .collect()
}

#[tokio::test]
async fn aliases_are_flattened_past_runtime_hop_limit_and_routable_for_chat_modes() {
    let mut forward = HashMap::new();
    for index in 0..20 {
        forward.insert(
            format!("alias-{index}"),
            if index == 19 {
                "gpt-4o".to_string()
            } else {
                format!("alias-{}", index + 1)
            },
        );
    }
    forward.insert("public-chat".to_string(), "alias-0".to_string());
    let mut reverse_entries = forward
        .iter()
        .map(|(alias, target)| (alias.clone(), target.clone()))
        .collect::<Vec<_>>();
    reverse_entries.sort_unstable_by(|(left, _), (right, _)| right.cmp(left));
    let reverse = reverse_entries.into_iter().collect::<HashMap<_, _>>();

    let first = Router::from_gateway_config_with_aliases(
        &[provider("primary", &["gpt-4o"], 0)],
        None,
        &forward,
    )
    .await
    .expect("forward alias graph should build");
    let second = Router::from_gateway_config_with_aliases(
        &[provider("primary", &["gpt-4o"], 0)],
        None,
        &reverse,
    )
    .await
    .expect("reverse alias graph should build");

    assert_eq!(first.model_aliases(), second.model_aliases());
    assert!(
        first
            .model_aliases()
            .values()
            .all(|target| target == "gpt-4o")
    );
    assert_eq!(first.resolve_model_name("public-chat"), "gpt-4o");

    for capability in [
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletionStream,
    ] {
        let lease = first
            .select_deployment_lease_for_capability("public-chat", &capability)
            .expect("configured alias should route for normal and streaming chat");
        assert_eq!(lease.deployment_id(), "primary-gpt-4o");
    }
}

#[tokio::test]
async fn phase_b_rejects_collisions_missing_and_disabled_only_targets() {
    let configured = provider("primary", &["gpt-4o", "gpt-4"], 0);
    let collision = aliases(&[("gpt-4o", "gpt-4")]);
    let error = Router::from_gateway_config_with_aliases(
        std::slice::from_ref(&configured),
        None,
        &collision,
    )
    .await
    .expect_err("canonical alias key must not shadow a deployment model");
    assert!(error.to_string().contains("collides"), "{error}");

    let missing = aliases(&[("public", "missing-model")]);
    let error = Router::from_gateway_config_with_aliases(&[configured], None, &missing)
        .await
        .expect_err("missing final target must fail before publication");
    assert!(error.to_string().contains("unavailable"), "{error}");

    let mut disabled = provider("disabled", &["disabled-model"], 1);
    disabled.enabled = false;
    let disabled_alias = aliases(&[("public", "disabled-model")]);
    let error = Router::from_gateway_config_with_aliases(&[disabled], None, &disabled_alias)
        .await
        .expect_err("disabled provider models must not satisfy alias targets");
    assert!(error.to_string().contains("unavailable"), "{error}");
}

#[tokio::test]
async fn aliases_accept_dynamic_and_provider_name_fallback_models() {
    let dynamic_alias = aliases(&[("dynamic", "gpt-4o")]);
    let dynamic = Router::from_gateway_config_with_aliases(
        &[provider("dynamic-openai", &[], 0)],
        None,
        &dynamic_alias,
    )
    .await
    .expect("provider catalog model should satisfy dynamic alias target");
    assert_eq!(dynamic.resolve_model_name("dynamic"), "gpt-4o");

    let fallback_provider = ProviderConfig {
        name: "local-fallback".to_string(),
        provider_type: "vllm".to_string(),
        ..ProviderConfig::default()
    };
    let fallback_alias = aliases(&[("local-chat", "local-fallback")]);
    let fallback =
        Router::from_gateway_config_with_aliases(&[fallback_provider], None, &fallback_alias)
            .await
            .expect("empty provider catalog should fall back to the provider name");
    assert_eq!(fallback.resolve_model_name("local-chat"), "local-fallback");
}

#[tokio::test]
async fn provider_priority_propagates_and_lower_value_wins_through_alias() {
    let router_config = RouterConfig {
        routing_strategy: RoutingStrategy::PriorityBased,
        ..RouterConfig::default()
    };
    let providers = [
        provider("fallback", &["gpt-4o"], 10),
        provider("primary", &["gpt-4o"], 1),
    ];
    let model_aliases = aliases(&[("production-chat", "gpt-4o")]);
    let router =
        Router::from_gateway_config_with_aliases(&providers, Some(router_config), &model_aliases)
            .await
            .expect("priority router should build");

    assert_eq!(
        router
            .get_deployment("fallback-gpt-4o")
            .expect("fallback deployment should exist")
            .config
            .priority,
        10
    );
    assert_eq!(
        router
            .get_deployment("primary-gpt-4o")
            .expect("primary deployment should exist")
            .config
            .priority,
        1
    );
    let lease = router
        .select_deployment_lease("production-chat")
        .expect("alias should select a deployment");
    assert_eq!(lease.deployment_id(), "primary-gpt-4o");
}

#[tokio::test]
async fn legacy_constructor_keeps_empty_aliases_and_zero_priority() {
    let router = Router::from_gateway_config(&[provider("legacy", &["gpt-4o"], 0)], None)
        .await
        .expect("legacy constructor should remain available");

    assert!(router.model_aliases().is_empty());
    assert_eq!(
        router
            .get_deployment("legacy-gpt-4o")
            .expect("legacy deployment should exist")
            .config
            .priority,
        0
    );
}
