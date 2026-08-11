//! Audit Logger
//!
//! The main logger that orchestrates audit event collection and output.

use regex::Regex;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};
use tracing::{debug, error, info, warn};

use super::config::AuditConfig;
use super::events::AuditEvent;
use super::outputs::{AuditOutput, BoxedAuditOutput, FileOutput, NullOutput, TracingOutput};
use super::types::{AuditResult, LogLevel};

/// The main audit logger
pub struct AuditLogger {
    config: AuditConfig,
    sender: mpsc::Sender<AuditEvent>,
    outputs: Arc<Vec<BoxedAuditOutput>>,
    redact_patterns: Vec<Regex>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub async fn new(config: AuditConfig) -> AuditResult<Self> {
        let mut outputs: Vec<BoxedAuditOutput> = Vec::new();

        // Add file output if configured
        if let Some(ref file_config) = config.file_output {
            info!("Initializing file audit output: {:?}", file_config.path);
            let file_output = FileOutput::new(&file_config.path).await?;
            outputs.push(Box::new(file_output));
        }

        // A configured audit logger must always have an externally observable
        // destination. File output is optional; tracing is the safe default.
        if outputs.is_empty() {
            debug!("No file audit output configured, using structured tracing");
            outputs.push(Box::new(TracingOutput));
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

        tokio::spawn(async move {
            Self::background_writer(receiver, writer_outputs, flush_interval, min_level).await;
        });

        info!("Audit logger initialized with {} outputs", outputs.len());

        Ok(Self {
            config,
            sender,
            outputs,
            redact_patterns,
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
        }
    }

    /// Background writer task
    async fn background_writer(
        mut receiver: mpsc::Receiver<AuditEvent>,
        outputs: Arc<Vec<BoxedAuditOutput>>,
        flush_interval_ms: u64,
        min_level: LogLevel,
    ) {
        let mut flush_timer = interval(Duration::from_millis(flush_interval_ms));

        loop {
            tokio::select! {
                Some(event) = receiver.recv() => {
                    // Check log level
                    if !event.level.should_log(min_level) {
                        continue;
                    }

                    // Write to all outputs
                    for output in outputs.iter() {
                        if let Err(e) = output.write(&event).await {
                            error!("Failed to write to audit output '{}': {}", output.name(), e);
                        }
                    }
                }
                _ = flush_timer.tick() => {
                    // Periodic flush
                    for output in outputs.iter() {
                        if let Err(e) = output.flush().await {
                            error!("Failed to flush audit output '{}': {}", output.name(), e);
                        }
                    }
                }
                else => break,
            }
        }

        Self::flush_and_close_outputs(&outputs).await;
    }

    async fn flush_and_close_outputs(outputs: &[BoxedAuditOutput]) {
        for output in outputs.iter() {
            Self::log_output_result(
                output.as_ref(),
                "flush audit output during shutdown",
                output.flush().await,
            );
            Self::log_output_result(
                output.as_ref(),
                "close audit output during shutdown",
                output.close().await,
            );
        }
    }

    fn log_output_result(
        output: &dyn AuditOutput,
        operation: &'static str,
        result: AuditResult<()>,
    ) {
        if let Err(e) = result {
            error!("Failed to {} '{}': {}", operation, output.name(), e);
        }
    }

    /// Log an audit event
    pub async fn log(&self, event: AuditEvent) {
        if !self.config.enabled {
            return;
        }

        // Apply redaction if needed
        let event = if self.config.redact_sensitive {
            self.redact_event(event)
        } else {
            event
        };

        if let Err(e) = self.sender.send(event).await {
            error!("Failed to send audit event: {}", e);
        }
    }

    /// Log an event without async (fire and forget)
    pub fn log_sync(&self, event: AuditEvent) {
        if !self.config.enabled {
            return;
        }

        let event = if self.config.redact_sensitive {
            self.redact_event(event)
        } else {
            event
        };

        let sender = self.sender.clone();
        tokio::spawn(async move {
            if let Err(e) = sender.send(event).await {
                error!("Failed to send audit event from sync logger: {}", e);
            }
        });
    }

    /// Redact sensitive data from an event
    fn redact_event(&self, mut event: AuditEvent) -> AuditEvent {
        // Redact message
        event.message = self.redact_string(&event.message);

        // Redact request body
        if let Some(ref mut request) = event.request
            && let Some(ref mut body) = request.body
        {
            *body = self.redact_string(body);
        }

        // Redact response body
        if let Some(ref mut response) = event.response
            && let Some(ref mut body) = response.body
        {
            *body = self.redact_string(body);
        }

        event
    }

    /// Redact sensitive data from a string
    fn redact_string(&self, s: &str) -> String {
        let mut result = s.to_string();
        for pattern in &self.redact_patterns {
            result = pattern.replace_all(&result, "[REDACTED]").to_string();
        }
        result
    }

    /// Check if logging is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
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
        let mut logger = AuditLogger::new(self.config).await?;

        // Add custom outputs
        let mut outputs = Arc::try_unwrap(logger.outputs).unwrap_or_else(|arc| {
            (*arc)
                .iter()
                .map(|_| Box::new(NullOutput) as BoxedAuditOutput)
                .collect()
        });

        for output in self.custom_outputs {
            outputs.push(output);
        }

        logger.outputs = Arc::new(outputs);
        Ok(logger)
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
    use crate::core::audit::types::AuditError;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn test_logger_creation() {
        let config = AuditConfig::new().enable();
        let logger = AuditLogger::new(config).await.unwrap();

        assert!(logger.is_enabled());
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
        logger.log(event).await;

        // Give time for async processing
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    #[tokio::test]
    async fn test_logger_path_exclusion() {
        let config = AuditConfig::new().enable();
        let logger = AuditLogger::new(config).await.unwrap();

        assert!(!logger.should_log_path("/health"));
        assert!(!logger.should_log_path("/metrics"));
        assert!(logger.should_log_path("/v1/chat/completions"));
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
        };

        let input = "API key: sk-abcdefghijklmnopqrstuvwxyz";
        let redacted = logger.redact_string(input);

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
        };

        let input = concat!(
            "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.signature ",
            "aws=AKIAIOSFODNN7EXAMPLE ",
            "anthropic=sk-ant-api03-abcdefghijklmnopqrstuvwxyz0123456789 ",
            "gateway=gw-abcdefghijklmnopqrstuvwxyz0123456789"
        );
        let redacted = logger.redact_string(input);

        assert!(!redacted.contains("Bearer eyJ"));
        assert!(!redacted.contains("AKIAIOSFODNN7EXAMPLE"));
        assert!(!redacted.contains("sk-ant-api03"));
        assert!(!redacted.contains("gw-abcdefghijklmnopqrstuvwxyz"));
        assert!(redacted.matches("[REDACTED]").count() >= 4);
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

        AuditLogger::flush_and_close_outputs(&outputs).await;

        assert_eq!(flush_count.load(Ordering::SeqCst), 1);
        assert_eq!(close_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_logger_with_file_output() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_audit_logger.log");

        // Clean up if exists
        let _ = tokio::fs::remove_file(&path).await;

        let config = AuditConfig::new().enable().with_file_output(&path);

        let logger = AuditLogger::new(config).await.unwrap();

        let event = AuditEvent::new(EventType::System, "Logger test event");
        logger.log(event).await;

        // Flush and wait
        logger.flush().await.unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Verify file was written
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("Logger test event"));

        // Clean up
        let _ = tokio::fs::remove_file(&path).await;
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
    }
}
