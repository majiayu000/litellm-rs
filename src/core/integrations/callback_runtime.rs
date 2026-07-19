//! Non-blocking ordered runtime for integration lifecycle callbacks.

use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::{error, warn};

use super::IntegrationManager;
use crate::core::traits::integration::{
    IntegrationError, IntegrationResult, LlmEndEvent, LlmErrorEvent, LlmStartEvent, LlmStreamEvent,
};

#[derive(Debug)]
enum CallbackEvent {
    Start(LlmStartEvent),
    End(LlmEndEvent),
    Error(LlmErrorEvent),
    Stream(LlmStreamEvent),
}

/// Error returned when an event cannot enter the non-blocking callback queue.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CallbackDispatchError {
    /// The bounded queue is at capacity.
    #[error("callback queue is full")]
    QueueFull,
    /// The callback worker is no longer accepting events.
    #[error("callback queue is closed")]
    QueueClosed,
}

/// Cloneable request-path handle for non-blocking callback delivery.
#[derive(Clone, Default)]
pub struct CallbackDispatcher {
    sender: Option<mpsc::Sender<CallbackEvent>>,
    manager: Option<Arc<IntegrationManager>>,
}

impl CallbackDispatcher {
    /// Create a dispatcher that performs no external callback work.
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Whether this dispatcher has an active callback worker.
    pub fn is_enabled(&self) -> bool {
        self.sender.is_some()
    }

    /// List the callback integrations registered with this dispatcher.
    pub async fn registered_integrations(&self) -> Vec<&'static str> {
        match &self.manager {
            Some(manager) => manager.list_integrations().await,
            None => Vec::new(),
        }
    }

    /// Enqueue an LLM start event without waiting for exporter I/O.
    pub fn emit_start(&self, event: LlmStartEvent) -> Result<(), CallbackDispatchError> {
        self.try_send(CallbackEvent::Start(event))
    }

    /// Enqueue an LLM success event without waiting for exporter I/O.
    pub fn emit_end(&self, event: LlmEndEvent) -> Result<(), CallbackDispatchError> {
        self.try_send(CallbackEvent::End(event))
    }

    /// Enqueue an LLM error event without waiting for exporter I/O.
    pub fn emit_error(&self, event: LlmErrorEvent) -> Result<(), CallbackDispatchError> {
        self.try_send(CallbackEvent::Error(event))
    }

    /// Enqueue an LLM stream event without waiting for exporter I/O.
    pub fn emit_stream(&self, event: LlmStreamEvent) -> Result<(), CallbackDispatchError> {
        self.try_send(CallbackEvent::Stream(event))
    }

    fn try_send(&self, event: CallbackEvent) -> Result<(), CallbackDispatchError> {
        let Some(sender) = &self.sender else {
            return Ok(());
        };
        sender.try_send(event).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => CallbackDispatchError::QueueFull,
            mpsc::error::TrySendError::Closed(_) => CallbackDispatchError::QueueClosed,
        })
    }
}

/// Owns the callback queue worker and its graceful-shutdown signal.
pub struct CallbackRuntime {
    dispatcher: CallbackDispatcher,
    shutdown_tx: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<IntegrationResult<()>>>,
}

impl CallbackRuntime {
    /// Start a callback worker around an existing integration manager.
    pub fn new(manager: Arc<IntegrationManager>, queue_capacity: usize) -> IntegrationResult<Self> {
        if queue_capacity == 0 {
            return Err(IntegrationError::config(
                "callback queue capacity must be greater than 0",
            ));
        }

        let (sender, receiver) = mpsc::channel(queue_capacity);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let worker_manager = Arc::clone(&manager);
        let worker = tokio::spawn(async move {
            run_callback_worker(worker_manager, receiver, shutdown_rx).await
        });

        Ok(Self {
            dispatcher: CallbackDispatcher {
                sender: Some(sender),
                manager: Some(manager),
            },
            shutdown_tx: Some(shutdown_tx),
            worker: Some(worker),
        })
    }

    /// Construct a disabled runtime.
    pub fn disabled() -> Self {
        Self {
            dispatcher: CallbackDispatcher::disabled(),
            shutdown_tx: None,
            worker: None,
        }
    }

    /// Obtain the request-path dispatcher.
    pub fn dispatcher(&self) -> CallbackDispatcher {
        self.dispatcher.clone()
    }

    /// Drain pending events, flush integrations, and stop the worker.
    pub async fn shutdown(mut self) -> IntegrationResult<()> {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
        let Some(worker) = self.worker.take() else {
            return Ok(());
        };
        worker
            .await
            .map_err(|error| IntegrationError::other(format!("callback worker failed: {error}")))?
    }
}

impl Drop for CallbackRuntime {
    fn drop(&mut self) {
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

async fn run_callback_worker(
    manager: Arc<IntegrationManager>,
    mut receiver: mpsc::Receiver<CallbackEvent>,
    mut shutdown_rx: oneshot::Receiver<()>,
) -> IntegrationResult<()> {
    loop {
        tokio::select! {
            biased;
            _ = &mut shutdown_rx => {
                receiver.close();
                while let Some(event) = receiver.recv().await {
                    dispatch_event(manager.as_ref(), event).await;
                }
                break;
            }
            event = receiver.recv() => {
                let Some(event) = event else {
                    break;
                };
                dispatch_event(manager.as_ref(), event).await;
            }
        }
    }

    if let Err(error) = manager.flush().await {
        warn!("Callback integration flush failed: {}", error);
    }
    manager.shutdown().await
}

async fn dispatch_event(manager: &IntegrationManager, event: CallbackEvent) {
    let result = match event {
        CallbackEvent::Start(event) => manager.on_llm_start(&event).await,
        CallbackEvent::End(event) => manager.on_llm_end(&event).await,
        CallbackEvent::Error(event) => manager.on_llm_error(&event).await,
        CallbackEvent::Stream(event) => manager.on_llm_stream(&event).await,
    };
    if let Err(error) = result {
        error!("Callback event dispatch failed: {}", error);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use async_trait::async_trait;
    use parking_lot::Mutex;
    use tokio::sync::Notify;

    use super::*;
    use crate::core::integrations::IntegrationManagerConfig;
    use crate::core::traits::integration::{Integration, LlmErrorEvent};

    struct RecordingIntegration {
        events: Arc<Mutex<Vec<&'static str>>>,
        flushed: Arc<AtomicBool>,
    }

    struct BlockingIntegration {
        entered: Arc<Notify>,
        release: Arc<Notify>,
    }

    struct FailingIntegration;

    #[async_trait]
    impl Integration for FailingIntegration {
        fn name(&self) -> &'static str {
            "failing"
        }

        fn is_enabled(&self) -> bool {
            true
        }

        async fn on_llm_start(&self, _event: &LlmStartEvent) -> IntegrationResult<()> {
            Err(IntegrationError::other("test callback failure"))
        }

        async fn on_llm_end(&self, _event: &LlmEndEvent) -> IntegrationResult<()> {
            Err(IntegrationError::other("test callback failure"))
        }

        async fn on_llm_error(&self, _event: &LlmErrorEvent) -> IntegrationResult<()> {
            Err(IntegrationError::other("test callback failure"))
        }

        async fn flush(&self) -> IntegrationResult<()> {
            Ok(())
        }

        async fn shutdown(&self) -> IntegrationResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl Integration for BlockingIntegration {
        fn name(&self) -> &'static str {
            "blocking"
        }

        fn is_enabled(&self) -> bool {
            true
        }

        async fn on_llm_start(&self, _event: &LlmStartEvent) -> IntegrationResult<()> {
            self.entered.notify_one();
            self.release.notified().await;
            Ok(())
        }

        async fn on_llm_end(&self, _event: &LlmEndEvent) -> IntegrationResult<()> {
            Ok(())
        }

        async fn on_llm_error(&self, _event: &LlmErrorEvent) -> IntegrationResult<()> {
            Ok(())
        }

        async fn flush(&self) -> IntegrationResult<()> {
            Ok(())
        }

        async fn shutdown(&self) -> IntegrationResult<()> {
            Ok(())
        }
    }

    #[async_trait]
    impl Integration for RecordingIntegration {
        fn name(&self) -> &'static str {
            "recording"
        }

        fn is_enabled(&self) -> bool {
            true
        }

        async fn on_llm_start(&self, _event: &LlmStartEvent) -> IntegrationResult<()> {
            self.events.lock().push("start");
            Ok(())
        }

        async fn on_llm_end(&self, _event: &LlmEndEvent) -> IntegrationResult<()> {
            self.events.lock().push("end");
            Ok(())
        }

        async fn on_llm_error(&self, _event: &LlmErrorEvent) -> IntegrationResult<()> {
            self.events.lock().push("error");
            Ok(())
        }

        async fn flush(&self) -> IntegrationResult<()> {
            self.flushed.store(true, Ordering::SeqCst);
            Ok(())
        }

        async fn shutdown(&self) -> IntegrationResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn runtime_preserves_order_and_flushes_on_shutdown() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let flushed = Arc::new(AtomicBool::new(false));
        let manager = Arc::new(IntegrationManager::with_defaults());
        manager
            .register(Arc::new(RecordingIntegration {
                events: Arc::clone(&events),
                flushed: Arc::clone(&flushed),
            }))
            .await;
        let runtime = match CallbackRuntime::new(manager, 8) {
            Ok(runtime) => runtime,
            Err(error) => panic!("callback runtime should start: {error}"),
        };
        let dispatcher = runtime.dispatcher();

        assert!(
            dispatcher
                .emit_start(LlmStartEvent::new("req-1", "model"))
                .is_ok()
        );
        assert!(
            dispatcher
                .emit_end(LlmEndEvent::new("req-1", "model"))
                .is_ok()
        );
        assert!(runtime.shutdown().await.is_ok());
        assert_eq!(*events.lock(), vec!["start", "end"]);
        assert!(flushed.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn runtime_rejects_zero_capacity() {
        let manager = Arc::new(IntegrationManager::with_defaults());
        let error = match CallbackRuntime::new(manager, 0) {
            Ok(_) => panic!("zero-capacity callback runtime must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("capacity"));
    }

    #[tokio::test]
    async fn dispatcher_reports_full_and_closed_queue() {
        let entered = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let manager = Arc::new(IntegrationManager::new(
            IntegrationManagerConfig::default().parallel(false),
        ));
        manager
            .register(Arc::new(BlockingIntegration {
                entered: Arc::clone(&entered),
                release: Arc::clone(&release),
            }))
            .await;
        let runtime = match CallbackRuntime::new(manager, 1) {
            Ok(runtime) => runtime,
            Err(error) => panic!("callback runtime should start: {error}"),
        };
        let dispatcher = runtime.dispatcher();
        let entered_wait = entered.notified();

        assert!(
            dispatcher
                .emit_start(LlmStartEvent::new("req-1", "model"))
                .is_ok()
        );
        entered_wait.await;
        assert!(
            dispatcher
                .emit_end(LlmEndEvent::new("req-1", "model"))
                .is_ok()
        );
        assert_eq!(
            dispatcher.emit_error(LlmErrorEvent::new("req-1", "model", "failed")),
            Err(CallbackDispatchError::QueueFull)
        );

        release.notify_one();
        assert!(runtime.shutdown().await.is_ok());
        assert_eq!(
            dispatcher.emit_start(LlmStartEvent::new("req-2", "model")),
            Err(CallbackDispatchError::QueueClosed)
        );
    }

    #[tokio::test]
    async fn failing_backend_does_not_block_healthy_backend() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let manager = Arc::new(IntegrationManager::new(
            IntegrationManagerConfig::default()
                .parallel(false)
                .fail_fast(false),
        ));
        manager.register(Arc::new(FailingIntegration)).await;
        manager
            .register(Arc::new(RecordingIntegration {
                events: Arc::clone(&events),
                flushed: Arc::new(AtomicBool::new(false)),
            }))
            .await;
        let runtime = match CallbackRuntime::new(manager, 8) {
            Ok(runtime) => runtime,
            Err(error) => panic!("callback runtime should start: {error}"),
        };
        let dispatcher = runtime.dispatcher();

        assert!(
            dispatcher
                .emit_start(LlmStartEvent::new("req-1", "model"))
                .is_ok()
        );
        assert!(
            dispatcher
                .emit_end(LlmEndEvent::new("req-1", "model"))
                .is_ok()
        );
        assert!(runtime.shutdown().await.is_ok());
        assert_eq!(*events.lock(), vec!["start", "end"]);
    }
}
