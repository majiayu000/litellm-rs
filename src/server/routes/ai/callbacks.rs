//! Metadata-only lifecycle callbacks for provider-backed AI requests.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use parking_lot::Mutex;
use tracing::{error, warn};

use crate::core::integrations::CallbackDispatcher;
use crate::core::pricing_service::{PricingService, PricingUsage};
use crate::core::traits::integration::{LlmEndEvent, LlmErrorEvent, LlmStartEvent};
use crate::core::types::context::RequestContext;

#[derive(Clone)]
pub(super) struct CallbackLifecycle {
    inner: Arc<CallbackLifecycleInner>,
}

struct CallbackLifecycleInner {
    dispatcher: CallbackDispatcher,
    pricing: Arc<PricingService>,
    request_id: String,
    requested_model: String,
    started_at: Instant,
    target: Mutex<Option<CallbackTarget>>,
    terminal_emitted: AtomicBool,
}

#[derive(Clone)]
struct CallbackTarget {
    provider: String,
    model: String,
    pricing_provider: String,
    pricing_model: String,
}

impl CallbackLifecycle {
    pub(super) fn start(
        dispatcher: &CallbackDispatcher,
        pricing: Arc<PricingService>,
        requested_model: impl Into<String>,
        context: &RequestContext,
    ) -> Self {
        let requested_model = requested_model.into();
        let mut event = LlmStartEvent::new(&context.request_id, &requested_model);
        event.user_id.clone_from(&context.user_id);
        if let Err(dispatch_error) = dispatcher.emit_start(event) {
            error!(
                request_id = %context.request_id,
                "Failed to enqueue callback start event: {}",
                dispatch_error
            );
        }

        Self {
            inner: Arc::new(CallbackLifecycleInner {
                dispatcher: dispatcher.clone(),
                pricing,
                request_id: context.request_id.clone(),
                requested_model,
                started_at: Instant::now(),
                target: Mutex::new(None),
                terminal_emitted: AtomicBool::new(false),
            }),
        }
    }

    pub(super) fn select_target(
        &self,
        provider: impl Into<String>,
        model: impl Into<String>,
        pricing_provider: impl Into<String>,
        pricing_model: impl Into<String>,
    ) {
        *self.inner.target.lock() = Some(CallbackTarget {
            provider: provider.into(),
            model: model.into(),
            pricing_provider: pricing_provider.into(),
            pricing_model: pricing_model.into(),
        });
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
        if !self.claim_terminal() {
            return;
        }

        let target = self.inner.target.lock().clone();
        let model = target
            .as_ref()
            .map(|target| target.model.as_str())
            .unwrap_or(&self.inner.requested_model);
        let mut event = LlmErrorEvent::new(
            &self.inner.request_id,
            model,
            safe_callback_error_message(error_type),
        )
        .error_type(error_type)
        .metadata(
            "latency_ms",
            serde_json::json!(self.inner.started_at.elapsed().as_millis() as u64),
        );
        if let Some(target) = target {
            event = event.provider(target.provider);
        }
        if let Err(dispatch_error) = self.inner.dispatcher.emit_error(event) {
            error!(
                request_id = %self.inner.request_id,
                "Failed to enqueue callback error event: {}",
                dispatch_error
            );
        }
    }

    fn complete(
        &self,
        tokens: Option<(u32, u32)>,
        pricing_usage: Option<&PricingUsage>,
        outcome: &'static str,
    ) {
        if !self.claim_terminal() {
            return;
        }

        let target = self.inner.target.lock().clone();
        let model = target
            .as_ref()
            .map(|target| target.model.as_str())
            .unwrap_or(&self.inner.requested_model);
        let mut event = LlmEndEvent::new(&self.inner.request_id, model)
            .latency(self.inner.started_at.elapsed().as_millis() as u64)
            .metadata("outcome", serde_json::json!(outcome));
        if let Some((input_tokens, output_tokens)) = tokens {
            event = event.tokens(input_tokens, output_tokens);
        }
        if let Some(target) = target {
            event = event.provider(target.provider);
            if let Some(usage) = pricing_usage {
                match self
                    .inner
                    .pricing
                    .calculate_loaded_settlement_cost_for_provider(
                        &target.pricing_provider,
                        &target.pricing_model,
                        usage,
                    ) {
                    Ok(cost) => event = event.cost(cost.total_cost),
                    Err(cost_error) => warn!(
                        request_id = %self.inner.request_id,
                        provider = %target.pricing_provider,
                        model = %target.pricing_model,
                        "Callback cost is unavailable: {}",
                        cost_error
                    ),
                }
            }
        }
        if let Err(dispatch_error) = self.inner.dispatcher.emit_end(event) {
            error!(
                request_id = %self.inner.request_id,
                "Failed to enqueue callback success event: {}",
                dispatch_error
            );
        }
    }

    fn claim_terminal(&self) -> bool {
        self.inner
            .terminal_emitted
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
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
        CallbackRuntime, IntegrationManager, IntegrationManagerConfig,
    };
    use crate::core::traits::integration::{Integration, IntegrationResult};

    type RecordedErrors = Arc<parking_lot::Mutex<Vec<(Option<String>, String)>>>;

    struct TerminalCounter {
        end_count: Arc<AtomicUsize>,
        error_count: Arc<AtomicUsize>,
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

        async fn flush(&self) -> IntegrationResult<()> {
            Ok(())
        }

        async fn shutdown(&self) -> IntegrationResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn lifecycle_emits_exactly_one_terminal_event() {
        let end_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));
        let errors = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let manager = Arc::new(IntegrationManager::new(
            IntegrationManagerConfig::default().parallel(false),
        ));
        manager
            .register(Arc::new(TerminalCounter {
                end_count: Arc::clone(&end_count),
                error_count: Arc::clone(&error_count),
                errors,
            }))
            .await;
        let runtime = match CallbackRuntime::new(manager, 8) {
            Ok(runtime) => runtime,
            Err(error) => panic!("callback runtime should start: {error}"),
        };
        let pricing = Arc::new(PricingService::new(None));
        let context = RequestContext::default();
        let lifecycle = CallbackLifecycle::start(&runtime.dispatcher(), pricing, "model", &context);
        lifecycle.complete_usage(None, "success");
        lifecycle.fail("late error", "provider_error");
        assert!(runtime.shutdown().await.is_ok());
        assert_eq!(end_count.load(Ordering::SeqCst), 1);
        assert_eq!(error_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn streaming_terminal_failure_kinds_are_safe_and_terminal_once() {
        let end_count = Arc::new(AtomicUsize::new(0));
        let error_count = Arc::new(AtomicUsize::new(0));
        let errors = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let manager = Arc::new(IntegrationManager::new(
            IntegrationManagerConfig::default().parallel(false),
        ));
        manager
            .register(Arc::new(TerminalCounter {
                end_count: Arc::clone(&end_count),
                error_count: Arc::clone(&error_count),
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
            let lifecycle = CallbackLifecycle::start(
                &dispatcher,
                Arc::clone(&pricing),
                "model",
                &RequestContext::default(),
            );
            lifecycle.fail("upstream-secret-must-not-leak", error_type);
            lifecycle.complete_usage(None, "late_success");
        }
        assert!(runtime.shutdown().await.is_ok());

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
}
