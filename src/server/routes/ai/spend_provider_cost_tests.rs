use super::*;
use crate::core::budget::{ProviderLimitConfig, ResetPeriod};
use crate::core::keys::{CreateKeyConfig, InMemoryKeyRepository};
use std::sync::Mutex;

struct Settled {
    cost: Option<f64>,
    charge_basis: Option<String>,
    provider_spend: f64,
    key_cost: f64,
}

async fn settle(provider: &str, model: &str, reported_cost: Option<f64>) -> Settled {
    let budget = UnifiedBudgetLimits::new();
    budget.providers.set_provider_limit(
        provider,
        ProviderLimitConfig::new(1000.0, ResetPeriod::Monthly),
    );
    let keys = KeyManager::new(InMemoryKeyRepository::new());
    let (key_id, _) = keys
        .generate_key(CreateKeyConfig {
            name: "provider cost key".to_string(),
            ..Default::default()
        })
        .await
        .expect("test key should be created");
    let mut usage = Usage::new(1000, 500);
    usage.cost = reported_cost;

    let facts = SharedRequestLedgerFacts::new(Mutex::new(Default::default()));
    record_completion_spend_with_reservation(
        usage_spend_settlement(
            (&budget, &keys, Some(key_id)),
            (provider, model, Some(&usage)),
            None,
            None,
        )
        .with_ledger_facts(Some(facts.clone())),
    )
    .await;

    let recorded = crate::core::request_ledger::snapshot_facts(&facts);
    Settled {
        cost: recorded.cost,
        charge_basis: recorded.billing.and_then(|billing| billing.charge_basis),
        provider_spend: budget
            .providers
            .get_provider_usage(provider)
            .map_or(0.0, |usage| usage.current_spend),
        key_cost: keys
            .get_usage_stats(key_id)
            .await
            .expect("usage stats should be readable")
            .total_cost,
    }
}

#[tokio::test]
async fn openrouter_reported_cost_settles_without_catalog_price() {
    // Not in the pricing catalog: only the provider-reported cost can price it.
    let settled = settle("openrouter", "openrouter/acme/unlisted-model", Some(0.0123)).await;
    assert_eq!(settled.cost, Some(0.0123));
    assert_eq!(settled.charge_basis.as_deref(), Some("provider_reported"));
    assert_eq!(settled.provider_spend, 0.0123);
    assert_eq!(settled.key_cost, 0.0123);
}

#[tokio::test]
async fn openrouter_reported_cost_overrides_catalog_price() {
    let settled = settle("openrouter", "openrouter/openai/gpt-4o", Some(0.0004)).await;
    assert_eq!(settled.cost, Some(0.0004));
    assert_eq!(settled.charge_basis.as_deref(), Some("provider_reported"));
    assert_eq!(settled.provider_spend, 0.0004);
}

#[tokio::test]
async fn invalid_openrouter_reported_cost_falls_back_to_catalog_price() {
    let catalog = settle("openrouter", "openrouter/openai/gpt-4o", None).await;
    let catalog_cost = catalog.cost.expect("catalog should price this model");
    assert!(catalog_cost > 0.0);
    assert_eq!(catalog.charge_basis.as_deref(), Some("gateway_pricing"));

    for invalid in [-1.0, f64::NAN, f64::INFINITY] {
        let settled = settle("openrouter", "openrouter/openai/gpt-4o", Some(invalid)).await;
        assert_eq!(settled.cost, Some(catalog_cost), "reported cost {invalid}");
        assert_eq!(settled.charge_basis.as_deref(), Some("gateway_pricing"));
    }
}

#[tokio::test]
async fn reported_cost_from_other_providers_is_ignored() {
    let settled = settle("openai", "gpt-4o", Some(99.0)).await;
    let cost = settled.cost.expect("catalog should price gpt-4o");
    assert!(cost < 1.0, "self-reported cost must not be trusted: {cost}");
    assert_eq!(settled.charge_basis.as_deref(), Some("gateway_pricing"));
}

/// Cost a completed LLM callback reports to Prometheus for one OpenRouter response.
fn callback_cost(model: &str, reported_cost: Option<f64>) -> Option<f64> {
    use crate::core::integrations::{CallbackRuntime, PrometheusIntegration};
    use crate::server::routes::ai::callbacks::CallbackLifecycle;
    use std::sync::Arc;

    let runtime = CallbackRuntime::disabled()
        .with_callback_metrics(Some(Arc::new(PrometheusIntegration::with_defaults())));
    let dispatcher = runtime.dispatcher();
    let pricing = PricingService::with_embedded_default().expect("embedded prices should load");
    let lifecycle = CallbackLifecycle::new(
        &dispatcher,
        Arc::new(pricing),
        model,
        &crate::core::types::context::RequestContext::default(),
    );
    lifecycle.begin_provider_execution("openrouter", model, "openrouter", model);
    let mut usage = Usage::new(1000, 500);
    usage.cost = reported_cost;
    lifecycle.complete_usage(Some(&usage), "success");

    dispatcher
        .render_prometheus_metrics()
        .expect("configured metrics should render")
        .lines()
        .find(|line| line.starts_with("litellm_cost_usd_total{") && line.contains("openrouter"))
        .and_then(|line| line.split_whitespace().last())
        .and_then(|value| value.parse::<f64>().ok())
}

#[tokio::test]
async fn callback_cost_matches_the_settled_cost() {
    let model = "openrouter/openai/gpt-4o";
    // Provider-reported, then the catalog fallback for a missing report.
    for reported_cost in [Some(0.0123), None] {
        let settled = settle("openrouter", model, reported_cost).await;
        assert!(settled.cost.is_some());
        assert_eq!(
            callback_cost(model, reported_cost),
            settled.cost,
            "reported cost {reported_cost:?}"
        );
    }
}
