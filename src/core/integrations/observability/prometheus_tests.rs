use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use super::*;

#[tokio::test]
async fn test_prometheus_integration_creation() {
    let integration = PrometheusIntegration::with_defaults();
    assert_eq!(integration.name(), "prometheus");
    assert!(integration.is_enabled());
}

#[tokio::test]
async fn llm_lifecycle_updates_active_requests() {
    let integration = PrometheusIntegration::with_defaults();
    let start = LlmStartEvent::new("req-1", "gpt-4").provider("openai");
    integration.on_llm_start(&start).await.unwrap();
    assert_eq!(integration.metrics.active_requests.get(), 1);

    let end = LlmEndEvent::new("req-1", "gpt-4")
        .provider("openai")
        .tokens(100, 50)
        .latency(150);
    integration.on_llm_end(&end).await.unwrap();
    assert_eq!(integration.metrics.active_requests.get(), 0);
}

#[tokio::test]
async fn llm_error_updates_active_requests() {
    let integration = PrometheusIntegration::with_defaults();
    integration
        .on_llm_start(&LlmStartEvent::new("req-1", "gpt-4").provider("openai"))
        .await
        .unwrap();
    integration
        .on_llm_error(&LlmErrorEvent::new("req-1", "gpt-4", "Rate limited").provider("openai"))
        .await
        .unwrap();
    assert_eq!(integration.metrics.active_requests.get(), 0);
}

#[tokio::test]
async fn cache_hit_is_not_exported_until_runtime_wiring_exists() {
    let integration = PrometheusIntegration::with_defaults();
    integration
        .on_cache_hit(&CacheHitEvent {
            request_id: "req-1".to_string(),
            cache_key: "key-1".to_string(),
            cache_backend: "redis".to_string(),
            time_saved_ms: Some(100),
            cost_saved_usd: Some(0.01),
            timestamp_ms: 0,
        })
        .await
        .unwrap();

    assert!(!integration.render_metrics().contains("cache_hits_total"));
}

#[test]
fn valid_custom_prefix_is_rendered() {
    let constructor: fn(PrometheusConfig) -> PrometheusIntegration = PrometheusIntegration::new;
    let config = PrometheusConfig {
        enabled: true,
        prefix: "myapp".to_string(),
        labels: HashMap::new(),
        per_model_metrics: true,
        per_provider_metrics: true,
        latency_buckets: vec![10.0, 100.0],
        token_buckets: vec![10.0, 100.0],
    };
    let integration = constructor(config);
    integration.record_llm_start(&LlmStartEvent::new("req-1", "gpt-4"));
    assert!(
        integration
            .render_metrics()
            .contains("myapp_requests_total")
    );
}

#[test]
fn invalid_programmatic_config_is_rejected() {
    for prefix in ["", "1bad", "bad-name"] {
        let config = PrometheusConfig {
            prefix: prefix.to_string(),
            ..Default::default()
        };
        assert!(PrometheusIntegration::try_new(config).is_err());
    }

    for key in ["bad-key", "__internal", "model", "provider", "le"] {
        let mut config = PrometheusConfig::default();
        config.labels.insert(key.to_string(), "value".to_string());
        assert!(PrometheusIntegration::try_new(config).is_err());
    }

    for buckets in [
        vec![],
        vec![0.0],
        vec![1.0, 1.0],
        vec![2.0, 1.0],
        vec![f64::NAN],
        vec![f64::INFINITY],
    ] {
        let config = PrometheusConfig {
            latency_buckets: buckets,
            ..Default::default()
        };
        assert!(PrometheusIntegration::try_new(config).is_err());
    }

    let invalid = PrometheusConfig {
        token_buckets: vec![-1.0],
        ..Default::default()
    };
    assert!(catch_unwind(AssertUnwindSafe(|| PrometheusIntegration::new(invalid))).is_err());
}

#[test]
fn invalid_costs_do_not_poison_counter() {
    let integration = PrometheusIntegration::with_defaults();
    for cost in [-1.0, f64::NAN, f64::INFINITY] {
        integration.record_llm_end(&LlmEndEvent::new("request", "model").cost(cost));
    }
    integration.record_llm_end(&LlmEndEvent::new("request", "model").cost(1.25));

    let rendered = integration.render_metrics();
    assert!(rendered.contains("litellm_cost_usd_total{model=\"model\"} 1.25"));
    assert!(!rendered.contains(" NaN"));
    assert!(!rendered.contains(" inf"));
    assert!(!rendered.contains(" -"));
}

#[test]
fn finite_cost_overflow_keeps_last_finite_counter_value() {
    let integration = PrometheusIntegration::with_defaults();
    for request_id in ["first", "second"] {
        integration.record_llm_end(&LlmEndEvent::new(request_id, "model").cost(f64::MAX));
    }

    let rendered = integration.render_metrics();
    let cost = rendered
        .lines()
        .find(|line| line.starts_with("litellm_cost_usd_total{model=\"model\"}"))
        .and_then(|line| line.split_whitespace().last())
        .and_then(|value| value.parse::<f64>().ok())
        .expect("cost counter should contain a numeric sample");
    assert_eq!(cost, f64::MAX);
    assert!(cost.is_finite());
    assert!(rendered.contains("litellm_requests_success_total{model=\"model\"} 2"));
    assert!(!rendered.contains(" NaN"));
    assert!(!rendered.contains(" inf"));
}

#[test]
fn concurrent_histogram_scrapes_are_internally_consistent() {
    let integration = Arc::new(PrometheusIntegration::with_defaults());
    integration.record_llm_end(&LlmEndEvent::new("seed", "model").latency(5));
    let writer = {
        let integration = Arc::clone(&integration);
        std::thread::spawn(move || {
            for index in 0..5_000 {
                integration.record_llm_end(
                    &LlmEndEvent::new(format!("request-{index}"), "model").latency(5),
                );
            }
        })
    };

    while !writer.is_finished() {
        assert_histogram_scrape(&integration.render_metrics());
    }
    writer.join().expect("histogram writer should finish");
    assert_histogram_scrape(&integration.render_metrics());
}

fn assert_histogram_scrape(metrics: &str) {
    let prefix = "litellm_request_latency_seconds";
    let mut finite_buckets = Vec::new();
    let mut infinite_bucket = None;
    let mut count = None;
    let mut sum = None;
    for line in metrics.lines().filter(|line| line.starts_with(prefix)) {
        let raw_value = line.split_whitespace().last();
        let value = raw_value.and_then(|value| value.parse::<u64>().ok());
        if line.contains("_bucket") && line.contains("le=\"+Inf\"") {
            infinite_bucket = value;
        } else if line.contains("_bucket") {
            finite_buckets.extend(value);
        } else if line.contains("_count") {
            count = value;
        } else if line.contains("_sum") {
            sum = raw_value.and_then(|value| value.parse::<f64>().ok());
        }
    }

    assert!(!finite_buckets.is_empty());
    assert!(finite_buckets.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(finite_buckets.last().copied(), infinite_bucket);
    assert_eq!(infinite_bucket, count);
    let expected_sum = count.expect("histogram count should render") as f64 * 0.005;
    assert!((sum.expect("histogram sum should render") - expected_sum).abs() < 1e-6);
}

#[tokio::test]
async fn label_values_are_escaped_in_rendered_metrics() {
    let mut config = PrometheusConfig::default();
    config.labels.insert(
        "tenant".to_string(),
        "team\\\"\nforged_config_metric 1".to_string(),
    );
    let integration = PrometheusIntegration::new(config);
    let event = LlmStartEvent::new("req-1", "model\\\"\nforged_model_metric 1")
        .provider("provider\\\"\nforged_provider_metric 1");

    integration.on_llm_start(&event).await.unwrap();
    let metrics = integration.render_metrics();
    assert!(metrics.contains(r#"tenant="team\\\"\nforged_config_metric 1""#));
    assert!(metrics.contains(r#"model="model\\\"\nforged_model_metric 1""#));
    assert!(metrics.contains(r#"provider="provider\\\"\nforged_provider_metric 1""#));
    assert!(!metrics.contains("\nforged_config_metric 1"));
    assert!(!metrics.contains("\nforged_model_metric 1"));
    assert!(!metrics.contains("\nforged_provider_metric 1"));
}
