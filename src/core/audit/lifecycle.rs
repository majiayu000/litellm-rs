#[cfg(feature = "gateway")]
use tokio::sync::oneshot;

use super::events::AuditEvent;
#[cfg(feature = "gateway")]
use super::logger::AuditLogger;
#[cfg(feature = "gateway")]
use std::sync::Arc;

pub(super) enum AuditCommand {
    Event(AuditEvent),
    #[cfg(feature = "gateway")]
    Request {
        started: AuditEvent,
        terminal: oneshot::Receiver<AuditEvent>,
    },
}

#[cfg(feature = "gateway")]
pub(crate) struct AuditEventPermit {
    terminal: Option<oneshot::Sender<AuditEvent>>,
    cancellation: Option<Box<dyn FnOnce() -> AuditEvent>>,
    logger: Option<Arc<AuditLogger>>,
}

#[cfg(feature = "gateway")]
impl AuditEventPermit {
    pub(super) fn disabled() -> Self {
        Self {
            terminal: None,
            cancellation: None,
            logger: None,
        }
    }

    pub(super) fn new(
        terminal: oneshot::Sender<AuditEvent>,
        cancellation: impl FnOnce() -> AuditEvent + 'static,
        logger: Arc<AuditLogger>,
    ) -> Self {
        Self {
            terminal: Some(terminal),
            cancellation: Some(Box::new(cancellation)),
            logger: Some(logger),
        }
    }

    pub(super) fn complete(mut self, event: AuditEvent) -> bool {
        self.cancellation = None;
        match self.terminal.take() {
            Some(terminal) => terminal.send(event).is_ok(),
            None => true,
        }
    }
}

#[cfg(feature = "gateway")]
impl Drop for AuditEventPermit {
    fn drop(&mut self) {
        let (Some(terminal), Some(cancellation), Some(logger)) = (
            self.terminal.take(),
            self.cancellation.take(),
            self.logger.take(),
        ) else {
            return;
        };
        let event = logger.prepare_event(cancellation());
        if terminal.send(event).is_err() {
            tracing::error!("Audit worker rejected a cancelled-request terminal event");
        }
    }
}
