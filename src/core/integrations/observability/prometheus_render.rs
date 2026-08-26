use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use parking_lot::RwLock;

use super::{Counter, Histogram, Labels, PrometheusIntegration};

pub(super) fn render_metrics(integration: &PrometheusIntegration) -> String {
    let mut output = String::new();
    let prefix = &integration.config.prefix;

    render_counter_map(
        &mut output,
        integration,
        "requests_total",
        "Total number of LLM requests",
        &integration.metrics.requests_total,
    );
    render_counter_map(
        &mut output,
        integration,
        "requests_success_total",
        "Total number of successful LLM requests",
        &integration.metrics.requests_success,
    );
    render_counter_map(
        &mut output,
        integration,
        "requests_error_total",
        "Total number of failed LLM requests",
        &integration.metrics.requests_error,
    );
    render_counter_map(
        &mut output,
        integration,
        "input_tokens_total",
        "Total number of input tokens",
        &integration.metrics.input_tokens_total,
    );
    render_counter_map(
        &mut output,
        integration,
        "output_tokens_total",
        "Total number of output tokens",
        &integration.metrics.output_tokens_total,
    );

    let global_labels = Labels::new(None, None).to_prometheus_string(&integration.config.labels);
    output.push_str(&format!(
        "# HELP {prefix}_active_requests Current number of active requests\n\
         # TYPE {prefix}_active_requests gauge\n\
         {prefix}_active_requests{global_labels} {}\n",
        integration.metrics.active_requests.get()
    ));

    render_atomic_counter_map(
        &mut output,
        integration,
        "cost_usd_total",
        "Total estimated LLM cost in US dollars",
        &integration.metrics.cost_total,
    );
    render_scalar_counter(
        &mut output,
        prefix,
        "embedding_requests_total",
        "Total number of embedding requests",
        &global_labels,
        integration.metrics.embedding_requests.get(),
    );
    render_scalar_counter(
        &mut output,
        prefix,
        "embedding_tokens_total",
        "Total number of embedding tokens",
        &global_labels,
        integration.metrics.embedding_tokens.get(),
    );

    render_histogram(
        &mut output,
        integration,
        "request_latency_seconds",
        "Request latency in seconds",
        &integration.metrics.request_latency,
    );
    output
}

fn render_counter_map(
    output: &mut String,
    integration: &PrometheusIntegration,
    name: &str,
    help: &str,
    map: &RwLock<HashMap<Labels, Arc<Counter>>>,
) {
    let prefix = &integration.config.prefix;
    output.push_str(&format!(
        "# HELP {prefix}_{name} {help}\n# TYPE {prefix}_{name} counter\n"
    ));
    for (labels, counter) in map.read().iter() {
        let labels = labels.to_prometheus_string(&integration.config.labels);
        output.push_str(&format!("{prefix}_{name}{labels} {}\n", counter.get()));
    }
}

fn render_atomic_counter_map(
    output: &mut String,
    integration: &PrometheusIntegration,
    name: &str,
    help: &str,
    map: &RwLock<HashMap<Labels, AtomicU64>>,
) {
    let prefix = &integration.config.prefix;
    output.push_str(&format!(
        "# HELP {prefix}_{name} {help}\n# TYPE {prefix}_{name} counter\n"
    ));
    for (labels, counter) in map.read().iter() {
        let labels = labels.to_prometheus_string(&integration.config.labels);
        output.push_str(&format!(
            "{prefix}_{name}{labels} {}\n",
            f64::from_bits(counter.load(Ordering::Relaxed))
        ));
    }
}

fn render_scalar_counter(
    output: &mut String,
    prefix: &str,
    name: &str,
    help: &str,
    labels: &str,
    value: u64,
) {
    output.push_str(&format!(
        "# HELP {prefix}_{name} {help}\n\
         # TYPE {prefix}_{name} counter\n\
         {prefix}_{name}{labels} {value}\n"
    ));
}

fn render_histogram(
    output: &mut String,
    integration: &PrometheusIntegration,
    name: &str,
    help: &str,
    map: &RwLock<HashMap<Labels, Arc<Histogram>>>,
) {
    let prefix = &integration.config.prefix;
    output.push_str(&format!(
        "# HELP {prefix}_{name} {help}\n# TYPE {prefix}_{name} histogram\n"
    ));
    for (labels, histogram) in map.read().iter() {
        let labels = labels.to_prometheus_string(&integration.config.labels);
        let snapshot = histogram.snapshot();
        for (index, bucket) in histogram.buckets.iter().enumerate() {
            let bucket_labels = with_bucket_label(&labels, &(bucket / 1000.0).to_string());
            output.push_str(&format!(
                "{prefix}_{name}_bucket{bucket_labels} {}\n",
                snapshot.counts[index]
            ));
        }
        let infinite_labels = with_bucket_label(&labels, "+Inf");
        output.push_str(&format!(
            "{prefix}_{name}_bucket{infinite_labels} {}\n",
            snapshot.count
        ));
        output.push_str(&format!(
            "{prefix}_{name}_sum{labels} {}\n",
            snapshot.sum / 1000.0
        ));
        output.push_str(&format!(
            "{prefix}_{name}_count{labels} {}\n",
            snapshot.count
        ));
    }
}

fn with_bucket_label(labels: &str, upper_bound: &str) -> String {
    if labels.is_empty() {
        format!("{{le=\"{upper_bound}\"}}")
    } else {
        format!("{{{},le=\"{upper_bound}\"}}", &labels[1..labels.len() - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::integration::{
        EmbeddingEndEvent, EmbeddingStartEvent, LlmEndEvent, LlmStartEvent,
    };

    #[test]
    fn renders_every_recorded_family_with_valid_histograms() {
        let integration = PrometheusIntegration::with_defaults();
        integration.record_llm_start(&LlmStartEvent::new("request", "model").provider("provider"));
        integration.record_llm_end(
            &LlmEndEvent::new("request", "model")
                .provider("provider")
                .tokens(12, 7)
                .latency(125)
                .cost(0.125),
        );
        integration.record_embedding_start(&EmbeddingStartEvent {
            request_id: "embedding".to_string(),
            model: "embedding-model".to_string(),
            provider: Some("provider".to_string()),
            input_count: 1,
            user_id: None,
            timestamp_ms: 0,
        });
        integration.record_embedding_end(&EmbeddingEndEvent {
            request_id: "embedding".to_string(),
            model: "embedding-model".to_string(),
            provider: Some("provider".to_string()),
            total_tokens: Some(9),
            cost_usd: None,
            latency_ms: 10,
            timestamp_ms: 0,
        });

        let rendered = integration.render_metrics();
        let labels = "model=\"model\",provider=\"provider\"";
        assert!(rendered.contains(&format!("litellm_cost_usd_total{{{labels}}} 0.125")));
        assert!(rendered.contains(&format!(
            "litellm_request_latency_seconds_bucket{{{labels},le=\"+Inf\"}} 1"
        )));
        assert!(!rendered.contains("litellm_time_to_first_token_seconds"));
        assert!(rendered.contains("litellm_embedding_requests_total 1"));
        assert!(rendered.contains("litellm_embedding_tokens_total 9"));
        assert!(!rendered.contains("litellm_cache_hits_total"));
        assert!(!rendered.contains("litellm_cache_misses_total"));
    }
}
