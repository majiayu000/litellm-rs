//! Gateway startup wiring for configured callback integrations.

use std::sync::Arc;

use tracing::{info, warn};

use crate::config::models::monitoring::{CallbackBackendConfig, CallbackConfig, MetricsConfig};
use crate::core::integrations::{
    CallbackRuntime, DataDogIntegration, IntegrationManager, IntegrationManagerConfig,
    LangfuseIntegration, OpenTelemetryIntegration, PrometheusIntegration,
    callback_runtime::CallbackMetricsRecorder,
};
use crate::core::traits::integration::BoxedIntegration;

pub(crate) async fn build_callback_runtime(
    config: &CallbackConfig,
    metrics: &MetricsConfig,
) -> CallbackRuntime {
    if config.backends.is_empty() && !metrics.enabled {
        return CallbackRuntime::disabled();
    }

    let manager = Arc::new(IntegrationManager::new(
        IntegrationManagerConfig::new()
            .fail_fast(false)
            .parallel(true)
            .timeout_ms(config.timeout_ms)
            .log_errors(true),
    ));
    for backend in &config.backends {
        match build_backend(backend) {
            Ok(integration) => {
                let name = integration.name();
                manager.register(integration).await;
                info!("Callback backend initialized: {}", name);
            }
            Err(error) => {
                warn!(
                    backend = backend.kind(),
                    "Callback backend initialization failed; continuing without it: {}", error
                );
            }
        }
    }

    let prometheus_metrics: Option<CallbackMetricsRecorder> = if metrics.enabled {
        let integration = Arc::new(PrometheusIntegration::with_defaults());
        info!("Callback backend initialized: prometheus");
        Some(integration)
    } else {
        None
    };

    if manager.count().await == 0 && prometheus_metrics.is_none() {
        warn!("No configured callback backend initialized successfully");
        return CallbackRuntime::disabled();
    }

    if manager.count().await == 0 {
        return CallbackRuntime::disabled().with_callback_metrics(prometheus_metrics);
    }

    match CallbackRuntime::new(manager, config.queue_capacity) {
        Ok(runtime) => runtime.with_callback_metrics(prometheus_metrics),
        Err(error) => {
            warn!(
                "Callback runtime initialization failed; continuing without callbacks: {}",
                error
            );
            CallbackRuntime::disabled().with_callback_metrics(prometheus_metrics)
        }
    }
}

fn build_backend(
    backend: &CallbackBackendConfig,
) -> crate::core::traits::integration::IntegrationResult<BoxedIntegration> {
    match backend {
        CallbackBackendConfig::OpenTelemetry(config) => {
            OpenTelemetryIntegration::try_new(config.clone())
                .map(|integration| Arc::new(integration) as BoxedIntegration)
        }
        CallbackBackendConfig::Datadog(config) => DataDogIntegration::new(config.clone())
            .map(|integration| Arc::new(integration) as BoxedIntegration),
        CallbackBackendConfig::Langfuse(config) => LangfuseIntegration::new(config.clone())
            .map(|integration| Arc::new(integration) as BoxedIntegration)
            .map_err(|error| {
                crate::core::traits::integration::IntegrationError::config(format!(
                    "Langfuse initialization failed: {error}"
                ))
            }),
    }
}

#[cfg(test)]
mod tests {
    use crate::core::integrations::{LangfuseConfig, OpenTelemetryConfig};
    use crate::core::traits::integration::{LlmEndEvent, LlmStartEvent};

    use super::*;

    #[tokio::test]
    async fn startup_registers_healthy_backend_and_skips_failed_backend() {
        let config = CallbackConfig {
            queue_capacity: 16,
            backends: vec![
                CallbackBackendConfig::Langfuse(LangfuseConfig {
                    public_key: None,
                    secret_key: None,
                    ..LangfuseConfig::default()
                }),
                CallbackBackendConfig::OpenTelemetry(OpenTelemetryConfig {
                    max_batch_size: 1,
                    ..OpenTelemetryConfig::default()
                }),
            ],
            ..CallbackConfig::default()
        };

        let metrics = MetricsConfig {
            enabled: false,
            ..MetricsConfig::default()
        };
        let runtime = build_callback_runtime(&config, &metrics).await;
        let dispatcher = runtime.dispatcher();
        assert!(dispatcher.is_enabled());
        assert_eq!(
            dispatcher.registered_integrations().await,
            vec!["opentelemetry"]
        );
        assert!(runtime.shutdown().await.is_ok());
    }

    #[tokio::test]
    async fn startup_wires_prometheus_when_metrics_are_enabled() {
        let config = CallbackConfig {
            queue_capacity: 16,
            ..CallbackConfig::default()
        };

        let runtime = build_callback_runtime(&config, &MetricsConfig::default()).await;
        let dispatcher = runtime.dispatcher();
        assert!(dispatcher.is_enabled());
        assert_eq!(
            dispatcher.registered_integrations().await,
            vec!["prometheus"]
        );
        let start = LlmStartEvent::new("request-1", "gpt-4").provider("openai");
        let metrics = dispatcher
            .begin_llm_metrics(&start)
            .expect("Prometheus metrics lifecycle should start");
        metrics.emit_end(&LlmEndEvent::new("request-1", "gpt-4").provider("openai"));
        assert!(runtime.shutdown().await.is_ok());
        assert!(
            dispatcher
                .render_prometheus_metrics()
                .is_some_and(|metrics| metrics
                    .contains("litellm_requests_total{model=\"gpt-4\",provider=\"openai\"} 1"))
        );
    }

    #[tokio::test]
    async fn disabled_metrics_without_callback_backends_stays_disabled() {
        let metrics = MetricsConfig {
            enabled: false,
            ..MetricsConfig::default()
        };
        let runtime = build_callback_runtime(&CallbackConfig::default(), &metrics).await;
        let dispatcher = runtime.dispatcher();

        assert!(!dispatcher.is_enabled());
        assert!(dispatcher.render_prometheus_metrics().is_none());
        assert!(runtime.shutdown().await.is_ok());
    }
}
