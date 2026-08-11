//! Audit Logger
//!
//! The main logger that orchestrates audit event collection and output.

use futures::FutureExt;
use futures::stream::{FuturesUnordered, StreamExt};
use regex::Regex;
use std::sync::{Arc, RwLock as StdRwLock};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio::time::{Duration, MissedTickBehavior, interval};
use tracing::{debug, error, info, warn};

use super::config::AuditConfig;
use super::events::AuditEvent;
use super::lifecycle::AuditCommand;
#[cfg(feature = "gateway")]
pub(crate) use super::lifecycle::AuditEventPermit;
use super::outputs::{BoxedAuditOutput, FileOutput, NullOutput, StderrOutput};
use super::types::{AuditError, AuditResult, LogLevel};

struct AuditWorker {
    shutdown: oneshot::Sender<()>,
    handle: JoinHandle<AuditResult<()>>,
}

/// The main audit logger
pub struct AuditLogger {
    config: AuditConfig,
    sender: mpsc::Sender<AuditCommand>,
    outputs: Arc<Vec<BoxedAuditOutput>>,
    redact_patterns: Vec<Regex>,
    failure: Arc<StdRwLock<Option<String>>>,
    worker: Mutex<Option<AuditWorker>>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub async fn new(config: AuditConfig) -> AuditResult<Self> {
        Self::new_with_outputs(config, Vec::new()).await
    }

    async fn new_with_outputs(
        config: AuditConfig,
        mut outputs: Vec<BoxedAuditOutput>,
    ) -> AuditResult<Self> {
        config.validate().map_err(AuditError::Config)?;
        // Add file output if configured
        if let Some(ref file_config) = config.file_output {
            info!("Initializing file audit output: {:?}", file_config.path);
            let file_output = FileOutput::new(&file_config.path).await?;
            outputs.push(Box::new(file_output));
        }

        // Enabled logging always gets an external destination independent of
        // ordinary tracing filters. File/custom outputs take precedence.
        if outputs.is_empty() {
            debug!("No file audit output configured, using structured stderr");
            outputs.push(Box::new(StderrOutput::new(config.buffer_size)?));
        }

        // Compile redact patterns
        let redact_patterns: Vec<Regex> = config
            .redact_patterns
            .iter()
            .filter_map(|p| {
                Regex::new(p)
                    .map_err(|e| warn!("Invalid redact pattern '{}': {}", p, e))
                    .ok()
            })
            .collect();

        let outputs = Arc::new(outputs);
        let (sender, receiver) = mpsc::channel(config.buffer_size);

        // Start background writer
        let writer_outputs = outputs.clone();
        let flush_interval = config.flush_interval_ms;
        let min_level = config.min_level;
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let failure = Arc::new(StdRwLock::new(None));
        let writer_failure = Arc::clone(&failure);

        let handle = tokio::spawn(async move {
            Self::background_writer(
                receiver,
                writer_outputs,
                flush_interval,
                min_level,
                shutdown_receiver,
                writer_failure,
            )
            .await
        });

        info!("Audit logger initialized with {} outputs", outputs.len());

        Ok(Self {
            config,
            sender,
            outputs,
            redact_patterns,
            failure,
            worker: Mutex::new(Some(AuditWorker { shutdown, handle })),
        })
    }

    /// Create a shared logger
    pub async fn shared(config: AuditConfig) -> AuditResult<Arc<Self>> {
        Ok(Arc::new(Self::new(config).await?))
    }

    /// Create a disabled logger (null output)
    pub fn disabled() -> Self {
        let outputs: Vec<BoxedAuditOutput> = vec![Box::new(NullOutput)];
        let (sender, _) = mpsc::channel(1);

        Self {
            config: AuditConfig::default(),
            sender,
            outputs: Arc::new(outputs),
            redact_patterns: Vec::new(),
            failure: Arc::new(StdRwLock::new(None)),
            worker: Mutex::new(None),
        }
    }

    /// Background writer task
    async fn background_writer(
        mut receiver: mpsc::Receiver<AuditCommand>,
        outputs: Arc<Vec<BoxedAuditOutput>>,
        flush_interval_ms: u64,
        min_level: LogLevel,
        mut shutdown: oneshot::Receiver<()>,
        failure: Arc<StdRwLock<Option<String>>>,
    ) -> AuditResult<()> {
        let mut flush_timer = interval(Duration::from_millis(flush_interval_ms));
        flush_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
        let mut terminals = FuturesUnordered::new();

        let writer_result: AuditResult<()> = async {
            loop {
                tokio::select! {
                    biased;
                    _ = &mut shutdown => {
                        receiver.close();
                        while let Some(command) = receiver.recv().await {
                            Self::accept_command(command, &outputs, min_level, &mut terminals)
                                .await?;
                        }
                        while let Some(terminal) = terminals.next().now_or_never().flatten() {
                            let event = terminal.map_err(|_| AuditError::Channel(
                                "audit request ended without a terminal event".to_string(),
                            ))?;
                            Self::write_event(&outputs, &event, min_level).await?;
                        }
                        break;
                    }
                    _ = flush_timer.tick() => {
                        Self::flush_outputs(&outputs).await?;
                    }
                    terminal = terminals.next(), if !terminals.is_empty() => {
                        match terminal {
                            Some(Ok(event)) => Self::write_event(&outputs, &event, min_level).await?,
                            Some(Err(_)) => return Err(AuditError::Channel(
                                "audit request ended without a terminal event".to_string(),
                            )),
                            None => {}
                        }
                    }
                    command = receiver.recv() => {
                        match command {
                            Some(command) => Self::accept_command(
                                command,
                                &outputs,
                                min_level,
                                &mut terminals,
                            ).await?,
                            None => break,
                        }
                    }
                    else => break,
                }
            }
            Ok(())
        }
        .await;

        if let Err(error) = &writer_result {
            Self::record_failure(&failure, error.to_string());
        }
        let close_result = Self::flush_and_close_outputs(&outputs).await;
        if let Err(error) = &close_result {
            Self::record_failure(&failure, error.to_string());
        }
        writer_result.and(close_result)
    }

    async fn accept_command(
        command: AuditCommand,
        outputs: &[BoxedAuditOutput],
        min_level: LogLevel,
        terminals: &mut FuturesUnordered<oneshot::Receiver<AuditEvent>>,
    ) -> AuditResult<()> {
        match command {
            AuditCommand::Event(event) => Self::write_event(outputs, &event, min_level).await,
            #[cfg(feature = "gateway")]
            AuditCommand::Request { started, terminal } => {
                Self::write_event(outputs, &started, min_level).await?;
                terminals.push(terminal);
                Ok(())
            }
        }
    }

    async fn write_event(
        outputs: &[BoxedAuditOutput],
        event: &AuditEvent,
        min_level: LogLevel,
    ) -> AuditResult<()> {
        if !event.level.should_log(min_level) {
            return Ok(());
        }
        for output in outputs {
            output.write(event).await.map_err(|error| {
                AuditError::Output(format!("write audit output '{}': {error}", output.name()))
            })?;
        }
        Ok(())
    }

    async fn flush_outputs(outputs: &[BoxedAuditOutput]) -> AuditResult<()> {
        for output in outputs {
            output.flush().await.map_err(|error| {
                AuditError::Output(format!("flush audit output '{}': {error}", output.name()))
            })?;
        }
        Ok(())
    }

    async fn flush_and_close_outputs(outputs: &[BoxedAuditOutput]) -> AuditResult<()> {
        let mut failures = Vec::new();
        for output in outputs.iter() {
            for (operation, result) in [
                ("flush audit output during shutdown", output.flush().await),
                ("close audit output during shutdown", output.close().await),
            ] {
                if let Err(error) = result {
                    error!("Failed to {} '{}': {}", operation, output.name(), error);
                    failures.push(format!("{operation} '{}': {error}", output.name()));
                }
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(AuditError::Output(failures.join("; ")))
        }
    }

    /// Log an audit event
    pub async fn log(&self, event: AuditEvent) -> AuditResult<()> {
        if !self.config.enabled {
            return Ok(());
        }
        self.ensure_available()?;
        let event = self.prepare_event(event);
        self.sender
            .try_send(AuditCommand::Event(event))
            .map_err(|error| {
                AuditError::Channel(format!("audit event queue rejected an event: {error}"))
            })
    }

    #[cfg(feature = "gateway")]
    pub(crate) fn start_request(
        self: &Arc<Self>,
        event: AuditEvent,
        cancellation: impl FnOnce() -> AuditEvent + 'static,
    ) -> AuditResult<AuditEventPermit> {
        if !self.config.enabled {
            return Ok(AuditEventPermit::disabled());
        }
        self.ensure_available()?;
        let (terminal, terminal_receiver) = oneshot::channel();
        self.sender
            .try_send(AuditCommand::Request {
                started: self.prepare_event(event),
                terminal: terminal_receiver,
            })
            .map_err(|error| {
                AuditError::Channel(format!("audit start event was rejected: {error}"))
            })?;
        Ok(AuditEventPermit::new(
            terminal,
            cancellation,
            Arc::clone(self),
        ))
    }

    #[cfg(feature = "gateway")]
    pub(crate) fn complete_request(
        &self,
        terminal: AuditEventPermit,
        event: AuditEvent,
    ) -> AuditResult<()> {
        self.ensure_available()?;
        if terminal.complete(self.prepare_event(event)) {
            Ok(())
        } else {
            Err(AuditError::Channel(
                "audit worker rejected a terminal event".to_string(),
            ))
        }
    }

    pub(super) fn prepare_event(&self, event: AuditEvent) -> AuditEvent {
        if self.config.redact_sensitive {
            self.redact_event(event)
        } else {
            event
        }
    }

    fn ensure_available(&self) -> AuditResult<()> {
        let failure = self.failure.read().unwrap_or_else(|lock| lock.into_inner());
        match failure.as_deref() {
            Some(failure) => Err(AuditError::Output(format!(
                "audit worker unavailable: {failure}"
            ))),
            None => Ok(()),
        }
    }

    fn record_failure(failure: &StdRwLock<Option<String>>, message: String) {
        error!("Audit worker stopped: {message}");
        let mut failure = failure.write().unwrap_or_else(|lock| lock.into_inner());
        if failure.is_none() {
            *failure = Some(message);
        }
    }

    /// Redact sensitive data from an event
    fn redact_event(&self, event: AuditEvent) -> AuditEvent {
        super::redaction::redact_event(&self.redact_patterns, event)
    }

    /// Check if logging is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Whether an enabled audit worker can still accept lifecycle events.
    pub fn is_available(&self) -> bool {
        !self.config.enabled || self.ensure_available().is_ok()
    }

    /// Check if a path should be logged
    pub fn should_log_path(&self, path: &str) -> bool {
        self.config.enabled && !self.config.is_path_excluded(path)
    }

    /// Get configuration
    pub fn config(&self) -> &AuditConfig {
        &self.config
    }

    /// Flush all outputs
    pub async fn flush(&self) -> AuditResult<()> {
        for output in self.outputs.iter() {
            output.flush().await?;
        }
        Ok(())
    }

    /// Stop accepting events, drain the queue, and close every output.
    pub async fn shutdown(&self) -> AuditResult<()> {
        let Some(worker) = self.worker.lock().await.take() else {
            return self.flush().await;
        };

        if worker.shutdown.send(()).is_err() {
            warn!("Audit worker stopped before receiving the shutdown signal");
        }
        worker
            .handle
            .await
            .map_err(|error| AuditError::Output(format!("audit worker join failed: {error}")))?
    }
}

/// Builder for AuditLogger
pub struct AuditLoggerBuilder {
    config: AuditConfig,
    custom_outputs: Vec<BoxedAuditOutput>,
}

impl AuditLoggerBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            config: AuditConfig::default(),
            custom_outputs: Vec::new(),
        }
    }

    /// Set configuration
    pub fn config(mut self, config: AuditConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a custom output
    pub fn add_output(mut self, output: BoxedAuditOutput) -> Self {
        self.custom_outputs.push(output);
        self
    }

    /// Build the logger
    pub async fn build(self) -> AuditResult<AuditLogger> {
        AuditLogger::new_with_outputs(self.config, self.custom_outputs).await
    }
}

impl Default for AuditLoggerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::audit::events::EventType;
    use crate::core::audit::outputs::AuditOutput;
    use crate::core::audit::types::{AuditError, RequestLog, ResponseLog, UserAction};
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_logger_creation() {
        let config = AuditConfig::new().enable();
        let logger = AuditLogger::new(config).await.unwrap();

        assert!(logger.is_enabled());
        logger.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_logger_disabled() {
        let logger = AuditLogger::disabled();
        assert!(!logger.is_enabled());
    }

    #[tokio::test]
    async fn test_logger_log_event() {
        let config = AuditConfig::new().enable();
        let logger = AuditLogger::new(config).await.unwrap();

        let event = AuditEvent::new(EventType::System, "Test event");
        assert!(logger.log(event).await.is_ok());
        logger.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_logger_path_exclusion() {
        let config = AuditConfig::new().enable();
        let logger = AuditLogger::new(config).await.unwrap();

        assert!(!logger.should_log_path("/health"));
        assert!(!logger.should_log_path("/metrics"));
        assert!(logger.should_log_path("/v1/chat/completions"));
        logger.shutdown().await.unwrap();
    }

    #[test]
    fn test_redact_string() {
        let config = AuditConfig::new().enable();
        let logger = AuditLogger::disabled();

        // Create logger with patterns
        let patterns: Vec<Regex> = vec![Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap()];

        let logger = AuditLogger {
            config,
            sender: logger.sender,
            outputs: logger.outputs,
            redact_patterns: patterns,
            failure: logger.failure,
            worker: Mutex::new(None),
        };

        let input = "API key: sk-abcdefghijklmnopqrstuvwxyz";
        let redacted = super::super::redaction::redact_string(&logger.redact_patterns, input);

        assert!(redacted.contains("[REDACTED]"));
        assert!(!redacted.contains("sk-abcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn test_default_redact_patterns_cover_common_secret_shapes() {
        let config = AuditConfig::new().enable();
        let disabled = AuditLogger::disabled();
        let patterns = config
            .redact_patterns
            .iter()
            .map(|pattern| Regex::new(pattern).unwrap())
            .collect();

        let logger = AuditLogger {
            config,
            sender: disabled.sender,
            outputs: disabled.outputs,
            redact_patterns: patterns,
            failure: disabled.failure,
            worker: Mutex::new(None),
        };

        let input = concat!(
            "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.signature ",
            "aws=AKIAIOSFODNN7EXAMPLE ",
            "anthropic=sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789 ",
            "gateway=gw-abcdefghijklmnopqrstuvwxyz0123456789"
        );
        let redacted = super::super::redaction::redact_string(&logger.redact_patterns, input);

        assert!(!redacted.contains("Bearer eyJ"));
        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!redacted.contains("sk-ant-api03"));
        assert!(!redacted.contains("gw-abcdefghijklmnopqrstuvwxyz"));
        assert!(redacted.matches("[REDACTED]").count() >= 4);
    }

    #[test]
    fn test_redaction_covers_all_string_bearing_event_fields() {
        let config = AuditConfig::new().enable();
        let disabled = AuditLogger::disabled();
        let patterns = config
            .redact_patterns
            .iter()
            .map(|pattern| Regex::new(pattern).unwrap())
            .collect();
        let logger = AuditLogger {
            config,
            sender: disabled.sender,
            outputs: disabled.outputs,
            redact_patterns: patterns,
            failure: disabled.failure,
            worker: Mutex::new(None),
        };
        let secret = "sk-abcdefghijklmnopqrstuvwxyz";
        let request = RequestLog::new(secret, secret, secret)
            .with_header(secret, secret)
            .with_body(secret, secret.len())
            .with_client_ip(secret)
            .with_user_agent(secret);
        let response = ResponseLog::new(secret, 200, 1)
            .with_header(secret, secret)
            .with_body(secret, secret.len());
        let mut event = AuditEvent::new(EventType::UserAction, secret)
            .with_request_id(secret)
            .with_user_id(secret)
            .with_api_key_id(secret)
            .with_team_id(secret)
            .with_request(request)
            .with_response(response)
            .with_action(UserAction::Custom(secret.to_string()))
            .with_metadata(secret, serde_json::json!({secret: [secret]}))
            .with_source(secret);
        event.id = secret.to_string();
        event
            .request
            .as_mut()
            .unwrap()
            .query_params
            .insert(secret.to_string(), secret.to_string());

        let serialized = serde_json::to_string(&logger.redact_event(event)).unwrap();

        assert!(!serialized.contains(secret));
        assert!(serialized.matches("[REDACTED]").count() >= 20);
    }

    #[derive(Clone)]
    struct RecordingOutput {
        events: Arc<Mutex<Vec<AuditEvent>>>,
        close_count: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl AuditOutput for RecordingOutput {
        fn name(&self) -> &str {
            "recording"
        }

        async fn write(&self, event: &AuditEvent) -> AuditResult<()> {
            self.events.lock().await.push(event.clone());
            Ok(())
        }

        async fn flush(&self) -> AuditResult<()> {
            Ok(())
        }

        async fn close(&self) -> AuditResult<()> {
            self.close_count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_shutdown_drains_pending_events_and_closes_custom_output() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let close_count = Arc::new(AtomicUsize::new(0));
        let output = RecordingOutput {
            events: Arc::clone(&events),
            close_count: Arc::clone(&close_count),
        };
        let logger = AuditLoggerBuilder::new()
            .config(AuditConfig::new().enable())
            .add_output(Box::new(output))
            .build()
            .await
            .unwrap();

        for index in 0..32 {
            assert!(
                logger
                    .log(AuditEvent::system(format!("event-{index}")))
                    .await
                    .is_ok()
            );
        }
        logger.shutdown().await.unwrap();

        assert_eq!(events.lock().await.len(), 32);
        assert_eq!(close_count.load(Ordering::SeqCst), 1);
    }

    struct FailingShutdownOutput {
        flush_count: Arc<AtomicUsize>,
        close_count: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl AuditOutput for FailingShutdownOutput {
        fn name(&self) -> &str {
            "failing_shutdown"
        }

        async fn write(&self, _event: &AuditEvent) -> AuditResult<()> {
            Ok(())
        }

        async fn flush(&self) -> AuditResult<()> {
            self.flush_count.fetch_add(1, Ordering::SeqCst);
            Err(AuditError::Output("flush failed".to_string()))
        }

        async fn close(&self) -> AuditResult<()> {
            self.close_count.fetch_add(1, Ordering::SeqCst);
            Err(AuditError::Output("close failed".to_string()))
        }
    }

    #[tokio::test]
    async fn test_shutdown_flush_and_close_failures_are_observed() {
        let flush_count = Arc::new(AtomicUsize::new(0));
        let close_count = Arc::new(AtomicUsize::new(0));
        let outputs: Vec<BoxedAuditOutput> = vec![Box::new(FailingShutdownOutput {
            flush_count: Arc::clone(&flush_count),
            close_count: Arc::clone(&close_count),
        })];

        let result = AuditLogger::flush_and_close_outputs(&outputs).await;

        assert!(result.is_err());
        assert_eq!(flush_count.load(Ordering::SeqCst), 1);
        assert_eq!(close_count.load(Ordering::SeqCst), 1);
    }

    struct BlockingFirstOutput {
        writes: AtomicUsize,
        started: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }

    #[async_trait::async_trait]
    impl AuditOutput for BlockingFirstOutput {
        fn name(&self) -> &str {
            "blocking_first"
        }

        async fn write(&self, _event: &AuditEvent) -> AuditResult<()> {
            if self.writes.fetch_add(1, Ordering::SeqCst) == 0 {
                self.started.notify_one();
                self.release.notified().await;
            }
            Ok(())
        }

        async fn flush(&self) -> AuditResult<()> {
            Ok(())
        }

        async fn close(&self) -> AuditResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn full_audit_queue_fails_fast_instead_of_waiting() {
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let output = BlockingFirstOutput {
            writes: AtomicUsize::new(0),
            started: Arc::clone(&started),
            release: Arc::clone(&release),
        };
        let mut config = AuditConfig::new().enable();
        config.buffer_size = 2;
        let logger = AuditLoggerBuilder::new()
            .config(config)
            .add_output(Box::new(output))
            .build()
            .await
            .unwrap();

        assert!(logger.log(AuditEvent::system("first")).await.is_ok());
        started.notified().await;
        assert!(logger.log(AuditEvent::system("second")).await.is_ok());
        assert!(logger.log(AuditEvent::system("third")).await.is_ok());
        assert!(matches!(
            logger.log(AuditEvent::system("fourth")).await,
            Err(AuditError::Channel(_))
        ));

        release.notify_one();
        logger.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_logger_with_file_output() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("audit.log");

        let config = AuditConfig::new().enable().with_file_output(&path);

        let logger = AuditLogger::new(config).await.unwrap();

        let event = AuditEvent::new(EventType::System, "Logger test event");
        assert!(logger.log(event).await.is_ok());
        logger.shutdown().await.unwrap();

        // Verify file was written
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("Logger test event"));
    }

    #[tokio::test]
    async fn test_builder() {
        let config = AuditConfig::new().enable();
        let logger = AuditLoggerBuilder::new()
            .config(config)
            .build()
            .await
            .unwrap();

        assert!(logger.is_enabled());
        logger.shutdown().await.unwrap();
    }
}
