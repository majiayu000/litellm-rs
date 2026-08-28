//! Metadata-only lifecycle callbacks for provider-backed AI requests.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use parking_lot::Mutex;
use tracing::{error, warn};

use crate::core::integrations::callback_runtime::CallbackMetricsPermit;
use crate::core::integrations::{CallbackDispatcher, CallbackTerminalPermit};
use crate::core::pricing_service::PricingService;
use crate::core::pricing_service::PricingUsage;
use crate::core::traits::integration::{
    EmbeddingEndEvent, EmbeddingErrorEvent, EmbeddingStartEvent, LlmEndEvent, LlmErrorEvent,
    LlmStartEvent,
};
use crate::core::types::context::RequestContext;

#[derive(Clone)]
pub(super) struct CallbackLifecycle {
    inner: Arc<CallbackLifecycleInner>,
}

struct CallbackLifecycleInner {
    dispatcher: CallbackDispatcher,
    #[cfg(test)]
    pricing: Arc<PricingService>,
    request_id: String,
    user_id: Option<String>,
    requested_model: String,
    kind: CallbackKind,
    started_at: Mutex<Option<Instant>>,
    target: Mutex<Option<CallbackTarget>>,
    terminal_permit: Mutex<Option<CallbackTerminalPermit>>,
    metrics_permit: Mutex<Option<CallbackMetricsPermit>>,
    begin_attempted: AtomicBool,
    terminal_emitted: AtomicBool,
}

#[derive(Clone, Copy)]
enum CallbackKind {
    Llm,
    Embedding { input_count: usize },
}

#[derive(Clone)]
struct CallbackTarget {
    provider: String,
    model: String,
    pricing: super::spend::RequestPricing,
}

impl CallbackLifecycle {
    pub(super) fn new(
        dispatcher: &CallbackDispatcher,
        pricing: Arc<PricingService>,
        requested_model: impl Into<String>,
        context: &RequestContext,
    ) -> Self {
        Self::new_with_kind(
            dispatcher,
            pricing,
            requested_model,
            context,
            CallbackKind::Llm,
        )
    }

    pub(super) fn new_embedding(
        dispatcher: &CallbackDispatcher,
        pricing: Arc<PricingService>,
        requested_model: impl Into<String>,
        input_count: usize,
        context: &RequestContext,
    ) -> Self {
        Self::new_with_kind(
            dispatcher,
            pricing,
            requested_model,
            context,
            CallbackKind::Embedding { input_count },
        )
    }

    fn new_with_kind(
        dispatcher: &CallbackDispatcher,
        pricing: Arc<PricingService>,
        requested_model: impl Into<String>,
        context: &RequestContext,
        kind: CallbackKind,
    ) -> Self {
        #[cfg(not(test))]
        drop(pricing);
        let requested_model = requested_model.into();
        Self {
            inner: Arc::new(CallbackLifecycleInner {
                dispatcher: dispatcher.clone(),
                #[cfg(test)]
                pricing,
                request_id: context.request_id.clone(),
                user_id: context.user_id.clone(),
                requested_model,
                kind,
                started_at: Mutex::new(None),
                target: Mutex::new(None),
                terminal_permit: Mutex::new(None),
                metrics_permit: Mutex::new(None),
                begin_attempted: AtomicBool::new(false),
                terminal_emitted: AtomicBool::new(false),
            }),
        }
    }

    #[cfg(test)]
    pub(super) fn begin_provider_execution(
        &self,
        provider: impl Into<String>,
        model: impl Into<String>,
        pricing_provider: impl Into<String>,
        pricing_model: impl Into<String>,
    ) {
        let pricing = super::spend::RequestPricing::from_exact(
            self.inner.pricing.as_ref(),
            pricing_provider,
            pricing_model,
        );
        self.begin_provider_execution_with_pricing(provider, model, pricing);
    }

    pub(super) fn begin_provider_execution_with_pricing(
        &self,
        provider: impl Into<String>,
        model: impl Into<String>,
        pricing: super::spend::RequestPricing,
    ) {
        let target = CallbackTarget {
            provider: provider.into(),
            model: model.into(),
            pricing,
        };
        {
            let mut current_target = self.inner.target.lock();
            *current_target = Some(target.clone());
            if self
                .inner
                .begin_attempted
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                if let Some(metrics_permit) = self.inner.metrics_permit.lock().as_mut() {
                    metrics_permit.update_llm_target(&target.model, &target.provider);
                }
                return;
            }
        }

        let (mut metrics_permit, admission) = match self.inner.kind {
            CallbackKind::Llm => {
                let mut event = LlmStartEvent::new(&self.inner.request_id, &target.model)
                    .provider(target.provider);
                event.user_id.clone_from(&self.inner.user_id);
                (
                    self.inner.dispatcher.begin_llm_metrics(&event),
                    self.inner.dispatcher.begin_llm(event),
                )
            }
            CallbackKind::Embedding { input_count } => {
                let event = EmbeddingStartEvent {
                    request_id: self.inner.request_id.clone(),
                    model: target.model,
                    provider: Some(target.provider),
                    input_count,
                    user_id: self.inner.user_id.clone(),
                    timestamp_ms: chrono::Utc::now().timestamp_millis(),
                };
                (
                    self.inner.dispatcher.begin_embedding_metrics(&event),
                    self.inner.dispatcher.begin_embedding(event),
                )
            }
        };
        {
            let current_target = self.inner.target.lock();
            if let (Some(metrics_permit), Some(current_target)) =
                (metrics_permit.as_mut(), current_target.as_ref())
            {
                metrics_permit.update_llm_target(&current_target.model, &current_target.provider);
            }
            *self.inner.metrics_permit.lock() = metrics_permit;
        }
        *self.inner.started_at.lock() = Some(Instant::now());
        match admission {
            Ok(permit) => {
                *self.inner.terminal_permit.lock() = Some(permit);
            }
            Err(dispatch_error) => {
                error!(
                    request_id = %self.inner.request_id,
                    "Failed to reserve callback lifecycle capacity: {}",
                    dispatch_error
                );
            }
        }
    }

    pub(super) fn complete_usage(
        &self,
        usage: Option<&crate::core::types::responses::Usage>,
        outcome: &'static str,
    ) {
        let pricing_usage = usage.map(PricingUsage::from);
        self.complete(
            usage.map(|usage| (usage.prompt_tokens, usage.completion_tokens)),
            pricing_usage.as_ref(),
            outcome,
        );
    }

    pub(super) fn fail(&self, _message: impl Into<String>, error_type: &'static str) {
        if !self.has_started() {
            return;
        }
        if !self.claim_terminal() {
            return;
        }
        let terminal_permit = self.inner.terminal_permit.lock().take();
        let metrics_permit = self.inner.metrics_permit.lock().take();

        let (model, provider) = self
            .inner
            .target
            .lock()
            .clone()
            .map(|target| (target.model, Some(target.provider)))
            .unwrap_or_else(|| (self.inner.requested_model.clone(), None));
        match self.inner.kind {
            CallbackKind::Llm => {
                let mut event = LlmErrorEvent::new(
                    &self.inner.request_id,
                    &model,
                    safe_callback_error_message(error_type),
                )
                .error_type(error_type)
                .metadata("latency_ms", serde_json::json!(self.elapsed_ms()));
                if let Some(provider) = provider {
                    event = event.provider(provider);
                }
                if let Some(metrics_permit) = metrics_permit {
                    metrics_permit.emit_error(&event);
                }
                if let Some(terminal_permit) = terminal_permit {
                    terminal_permit.emit_error(event);
                }
            }
            CallbackKind::Embedding { .. } => {
                let event = EmbeddingErrorEvent {
                    request_id: self.inner.request_id.clone(),
                    model,
                    provider,
                    error_message: safe_callback_error_message(error_type).to_string(),
                    error_type: Some(error_type.to_string()),
                    latency_ms: self.elapsed_ms(),
                    timestamp_ms: chrono::Utc::now().timestamp_millis(),
                };
                if let Some(metrics_permit) = metrics_permit {
                    metrics_permit.emit_embedding_error();
                }
                if let Some(terminal_permit) = terminal_permit {
                    terminal_permit.emit_embedding_error(event);
                }
            }
        }
    }

    fn complete(
        &self,
        tokens: Option<(u32, u32)>,
        pricing_usage: Option<&PricingUsage>,
        outcome: &'static str,
    ) {
        if !self.has_started() {
            return;
        }
        if !self.claim_terminal() {
            return;
        }
        let terminal_permit = self.inner.terminal_permit.lock().take();
        let metrics_permit = self.inner.metrics_permit.lock().take();

        let target = self.inner.target.lock().clone();
        let model = target
            .as_ref()
            .map(|target| target.model.as_str())
            .unwrap_or(&self.inner.requested_model);
        let provider = target.as_ref().map(|target| target.provider.clone());
        let cost = target.as_ref().and_then(|target| {
            pricing_usage.and_then(|usage| match target.pricing.calculate_settlement(usage) {
                Ok(cost) => Some(cost.total_cost),
                Err(cost_error) => {
                    let pricing = target.pricing.priced_parts();
                    warn!(
                        request_id = %self.inner.request_id,
                        provider = pricing.map_or("unpriced", |(provider, _)| provider),
                        model = pricing.map_or("unpriced", |(_, model)| model),
                        "Callback cost is unavailable: {}",
                        cost_error
                    );
                    None
                }
            })
        });

        match self.inner.kind {
            CallbackKind::Llm => {
                let mut event = LlmEndEvent::new(&self.inner.request_id, model)
                    .latency(self.elapsed_ms())
                    .metadata("outcome", serde_json::json!(outcome));
                if let Some((input_tokens, output_tokens)) = tokens {
                    event = event.tokens(input_tokens, output_tokens);
                }
                if let Some(provider) = provider {
                    event = event.provider(provider);
                }
                if let Some(cost) = cost {
                    event = event.cost(cost);
                }
                if let Some(metrics_permit) = metrics_permit {
                    metrics_permit.emit_end(&event);
                }
                if let Some(terminal_permit) = terminal_permit {
                    terminal_permit.emit_end(event);
                }
            }
            CallbackKind::Embedding { .. } => {
                let event = EmbeddingEndEvent {
                    request_id: self.inner.request_id.clone(),
                    model: model.to_string(),
                    provider,
                    total_tokens: tokens.map(|(input, output)| input.saturating_add(output)),
                    cost_usd: cost,
                    latency_ms: self.elapsed_ms(),
                    timestamp_ms: chrono::Utc::now().timestamp_millis(),
                };
                if let Some(metrics_permit) = metrics_permit {
                    metrics_permit.emit_embedding_end(&event);
                }
                if let Some(terminal_permit) = terminal_permit {
                    terminal_permit.emit_embedding_end(event);
                }
            }
        }
    }

    fn claim_terminal(&self) -> bool {
        self.inner
            .terminal_emitted
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn has_started(&self) -> bool {
        self.inner.started_at.lock().is_some()
    }

    fn elapsed_ms(&self) -> u64 {
        self.inner
            .started_at
            .lock()
            .as_ref()
            .map(|started_at| started_at.elapsed().as_millis() as u64)
            .unwrap_or_default()
    }
}

fn safe_callback_error_message(error_type: &str) -> &'static str {
    match error_type {
        "timeout" => "provider request timed out",
        "client_disconnect" => "client disconnected",
        "cache_error" => "response cache operation failed",
        "serialization_error" => "response serialization failed",
        "conversion_error" => "provider response conversion failed",
        _ => "provider request failed",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;

    use super::*;
    use crate::core::integrations::{
        CallbackRuntime, IntegrationManager, IntegrationManagerConfig, PrometheusIntegration,
    };
    use crate::core::traits::integration::{Integration, IntegrationResult};

    type RecordedErrors = Arc<parking_lot::Mutex<Vec<(Option<String>, String)>>>;

    struct TerminalCounter {
        start_count: Arc<AtomicUsize>,
        end_count: Arc<AtomicUsize>,
        error_count: Arc<AtomicUsize>,
        embedding_error_count: Arc<AtomicUsize>,
        errors: RecordedErrors,
    }

    #[async_trait]
    impl Integration for TerminalCounter {
        fn name(&self) -> &'static str {
            "terminal-counter"
        }

        fn is_enabled(&self) -> bool {
            true
        }

        async fn on_llm_start(&self, _event: &LlmStartEvent) -> IntegrationResult<()> {
            self.start_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn on_llm_end(&self, _event: &LlmEndEvent) -> IntegrationResult<()> {
            self.end_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()> {
            self.error_count.fetch_add(1, Ordering::SeqCst);
            self.errors
                .lock()
                .push((event.error_type.clone(), event.error_message.clone()));
            Ok(())
        }

        async fn on_embedding_error(&self, _event: &EmbeddingErrorEvent) -> IntegrationResult<()> {
            self.embedding_error_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn flush(&self) -> IntegrationResult<()> {
            Ok(())
        }

        async fn shutdown(&self) -> IntegrationResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn lifecycle_emits_exactly_one_terminal_event() {
        let start_count = Arc::new(AtomicUsize::new(0));
        let end_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));
        let errors = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let manager = Arc::new(IntegrationManager::new(
            IntegrationManagerConfig::default().parallel(false),
        ));
        manager
            .register(Arc::new(TerminalCounter {
                start_count: Arc::clone(&start_count),
                end_count: Arc::clone(&end_count),
                error_count: Arc::clone(&error_count),
                embedding_error_count: Arc::new(AtomicUsize::new(0)),
                errors,
            }))
            .await;
        let prometheus = Arc::new(PrometheusIntegration::with_defaults());
        let runtime = match CallbackRuntime::new(manager, 8) {
            Ok(runtime) => runtime,
            Err(error) => panic!("callback runtime should start: {error}"),
        }
        .with_callback_metrics(Some(prometheus));
        let dispatcher = runtime.dispatcher();
        let pricing = Arc::new(PricingService::new(None));
        let context = RequestContext::default();
        let lifecycle = CallbackLifecycle::new(&dispatcher, pricing, "model", &context);
        lifecycle.begin_provider_execution(
            "first-provider",
            "first-model",
            "first-provider",
            "first-model",
        );
        lifecycle.begin_provider_execution(
            "final-provider",
            "final-model",
            "final-provider",
            "final-model",
        );
        lifecycle.complete_usage(None, "success");
        lifecycle.fail("late error", "provider_error");
        let rendered = dispatcher
            .render_prometheus_metrics()
            .expect("configured metrics should render");
        assert!(runtime.shutdown().await.is_ok());
        assert_eq!(start_count.load(Ordering::SeqCst), 1);
        assert_eq!(end_count.load(Ordering::SeqCst), 1);
        assert_eq!(error_count.load(Ordering::SeqCst), 0);
        assert!(rendered.contains(
            "litellm_requests_total{model=\"final-model\",provider=\"final-provider\"} 1"
        ));
        assert!(rendered.contains(
            "litellm_requests_success_total{model=\"final-model\",provider=\"final-provider\"} 1"
        ));
        assert!(!rendered.contains("litellm_requests_total{model=\"first-model\""));
        assert!(rendered.contains("litellm_active_requests 0"));
    }

    #[test]
    fn dropping_started_lifecycle_uses_final_target_and_releases_active_metric() {
        let runtime = CallbackRuntime::disabled()
            .with_callback_metrics(Some(Arc::new(PrometheusIntegration::with_defaults())));
        let dispatcher = runtime.dispatcher();
        {
            let lifecycle = CallbackLifecycle::new(
                &dispatcher,
                Arc::new(PricingService::new(None)),
                "model",
                &RequestContext::default(),
            );
            lifecycle.begin_provider_execution(
                "first-provider",
                "first-model",
                "first-provider",
                "first-model",
            );
            lifecycle.begin_provider_execution(
                "final-provider",
                "final-model",
                "final-provider",
                "final-model",
            );
            let active = dispatcher
                .render_prometheus_metrics()
                .expect("configured metrics should render");
            assert!(active.contains("litellm_active_requests 1"));
        }

        let released = dispatcher
            .render_prometheus_metrics()
            .expect("configured metrics should render");
        assert!(released.contains("litellm_active_requests 0"));
        assert!(released.contains(
            "litellm_requests_total{model=\"final-model\",provider=\"final-provider\"} 1"
        ));
        assert!(!released.contains("litellm_requests_total{model=\"first-model\""));
    }

    #[tokio::test]
    async fn embedding_failure_uses_embedding_terminal_and_preserves_active_llm_metrics() {
        let llm_error_count = Arc::new(AtomicUsize::new(0));
        let embedding_error_count = Arc::new(AtomicUsize::new(0));
        let manager = Arc::new(IntegrationManager::new(
            IntegrationManagerConfig::default().parallel(false),
        ));
        manager
            .register(Arc::new(TerminalCounter {
                start_count: Arc::new(AtomicUsize::new(0)),
                end_count: Arc::new(AtomicUsize::new(0)),
                error_count: Arc::clone(&llm_error_count),
                embedding_error_count: Arc::clone(&embedding_error_count),
                errors: Arc::new(parking_lot::Mutex::new(Vec::new())),
            }))
            .await;
        let runtime = CallbackRuntime::new(manager, 6)
            .expect("callback runtime should start")
            .with_callback_metrics(Some(Arc::new(PrometheusIntegration::with_defaults())));
        let dispatcher = runtime.dispatcher();
        let pricing = Arc::new(PricingService::new(None));
        let context = RequestContext::default();

        let llm = CallbackLifecycle::new(&dispatcher, Arc::clone(&pricing), "llm", &context);
        llm.begin_provider_execution("llm-provider", "llm-model", "llm-provider", "llm-model");
        let embedding =
            CallbackLifecycle::new_embedding(&dispatcher, pricing, "embedding", 1, &context);
        embedding.begin_provider_execution(
            "embedding-provider",
            "embedding-model",
            "embedding-provider",
            "embedding-model",
        );
        embedding.fail("embedding failed", "provider_error");

        let rendered = dispatcher
            .render_prometheus_metrics()
            .expect("configured metrics should render");
        assert!(rendered.contains("litellm_active_requests 1"));
        assert!(rendered.contains("litellm_embedding_requests_total 1"));
        assert!(!rendered.contains("model=\"embedding-model\""));

        drop(llm);
        assert!(runtime.shutdown().await.is_ok());
        assert_eq!(llm_error_count.load(Ordering::SeqCst), 0);
        assert_eq!(embedding_error_count.load(Ordering::SeqCst), 1);
        let released = dispatcher
            .render_prometheus_metrics()
            .expect("configured metrics should render");
        assert!(released.contains("litellm_active_requests 0"));
    }

    #[tokio::test]
    async fn streaming_terminal_failure_kinds_are_safe_and_terminal_once() {
        let start_count = Arc::new(AtomicUsize::new(0));
        let end_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));
        let errors = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let manager = Arc::new(IntegrationManager::new(
            IntegrationManagerConfig::default().parallel(false),
        ));
        manager
            .register(Arc::new(TerminalCounter {
                start_count: Arc::clone(&start_count),
                end_count: Arc::clone(&end_count),
                error_count: Arc::clone(&error_count),
                embedding_error_count: Arc::new(AtomicUsize::new(0)),
                errors: Arc::clone(&errors),
            }))
            .await;
        let runtime = match CallbackRuntime::new(manager, 16) {
            Ok(runtime) => runtime,
            Err(error) => panic!("callback runtime should start: {error}"),
        };
        let dispatcher = runtime.dispatcher();
        let pricing = Arc::new(PricingService::new(None));

        for error_type in [
            "provider_error",
            "timeout",
            "conversion_error",
            "serialization_error",
            "client_disconnect",
        ] {
            let lifecycle = CallbackLifecycle::new(
                &dispatcher,
                Arc::clone(&pricing),
                "model",
                &RequestContext::default(),
            );
            lifecycle.begin_provider_execution("provider", "model", "provider", "model");
            lifecycle.fail("upstream-secret-must-not-leak", error_type);
            lifecycle.complete_usage(None, "late_success");
        }
        assert!(runtime.shutdown().await.is_ok());

        assert_eq!(start_count.load(Ordering::SeqCst), 5);
        assert_eq!(end_count.load(Ordering::SeqCst), 0);
        assert_eq!(error_count.load(Ordering::SeqCst), 5);
        let errors = errors.lock();
        assert_eq!(errors.len(), 5);
        assert!(errors.iter().all(|(_, message)| {
            !message.contains("upstream-secret-must-not-leak")
                && !message.contains("prompt")
                && !message.contains("output")
        }));
    }

    #[tokio::test]
    async fn pre_provider_rejection_emits_no_lifecycle_events() {
        let start_count = Arc::new(AtomicUsize::new(0));
        let end_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));
        let manager = Arc::new(IntegrationManager::new(
            IntegrationManagerConfig::default().parallel(false),
        ));
        manager
            .register(Arc::new(TerminalCounter {
                start_count: Arc::clone(&start_count),
                end_count: Arc::clone(&end_count),
                error_count: Arc::clone(&error_count),
                embedding_error_count: Arc::new(AtomicUsize::new(0)),
                errors: Arc::new(parking_lot::Mutex::new(Vec::new())),
            }))
            .await;
        let runtime = match CallbackRuntime::new(manager, 4) {
            Ok(runtime) => runtime,
            Err(error) => panic!("callback runtime should start: {error}"),
        };

        let lifecycle = CallbackLifecycle::new(
            &runtime.dispatcher(),
            Arc::new(PricingService::new(None)),
            "model",
            &RequestContext::default(),
        );
        lifecycle.fail("budget rejected before provider call", "provider_error");
        assert!(runtime.shutdown().await.is_ok());
        assert_eq!(start_count.load(Ordering::SeqCst), 0);
        assert_eq!(end_count.load(Ordering::SeqCst), 0);
        assert_eq!(error_count.load(Ordering::SeqCst), 0);
    }
}
