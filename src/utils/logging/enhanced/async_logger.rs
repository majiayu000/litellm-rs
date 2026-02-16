//! Async logger implementation for high-performance logging

use crate::utils::logging::enhanced::types::{AsyncLogRecord, AsyncLoggerConfig};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::mpsc;
use tracing::{Level, debug, error, info, warn};
use uuid::Uuid;

/// Async logger for high-performance logging
#[allow(dead_code)]
pub struct AsyncLogger {
    sender: mpsc::Sender<AsyncLogRecord>,
    config: AsyncLoggerConfig,
    sample_counter: AtomicU64,
}

#[allow(dead_code)]
impl AsyncLogger {
    /// Create a new async logger with bounded channel to prevent memory leaks
    pub fn new(config: AsyncLoggerConfig) -> Self {
        // Use bounded channel with configured buffer size to prevent OOM
        let (sender, mut receiver) = mpsc::channel::<AsyncLogRecord>(config.buffer_size);

        // Spawn background task to process log entries
        tokio::spawn(async move {
            while let Some(entry) = receiver.recv().await {
                Self::process_log_entry(entry).await;
            }
        });

        Self {
            sender,
            config,
            sample_counter: AtomicU64::new(0),
        }
    }

    /// Try to send a log entry, handling backpressure
    fn try_send(&self, entry: AsyncLogRecord) -> bool {
        match self.sender.try_send(entry) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                if !self.config.drop_on_overflow {
                    // Log overflow warning (but don't recurse)
                    warn!("Async logger buffer full, log entry dropped");
                }
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                error!("Async logger channel closed");
                false
            }
        }
    }

    /// Log a message with structured fields
    pub fn log_structured(
        &self,
        level: Level,
        logger: &str,
        message: &str,
        fields: HashMap<String, serde_json::Value>,
        request_id: Option<String>,
        user_id: Option<Uuid>,
    ) {
        // Apply sampling if configured (rate < 1.0)
        if self.config.sample_rate < 1.0 {
            if self.config.sample_rate <= 0.0 {
                return; // 0% sampling = drop all
            }
            let counter = self.sample_counter.fetch_add(1, Ordering::Relaxed);
            // Correct sampling: sample every N logs where N = 1/rate
            // e.g., rate=0.1 means keep 1 in 10, rate=0.5 means keep 1 in 2
            let sample_interval = (1.0 / self.config.sample_rate) as u64;
            if !counter.is_multiple_of(sample_interval) {
                return;
            }
        }

        // Truncate message if too long
        let truncated_message = if message.len() > self.config.max_message_length {
            format!("{}...", &message[..self.config.max_message_length - 3])
        } else {
            message.to_string()
        };

        let entry = AsyncLogRecord {
            timestamp: chrono::Utc::now(),
            level: level.to_string(),
            logger: logger.to_string(),
            message: truncated_message,
            fields,
            request_id,
            user_id,
            trace_id: Self::current_trace_id(),
        };

        // Use try_send with backpressure handling instead of blocking send
        self.try_send(entry);
    }

    /// Log a simple message
    pub fn log(&self, level: Level, logger: &str, message: &str) {
        self.log_structured(level, logger, message, HashMap::new(), None, None);
    }

    /// Log with request context
    pub fn log_with_context(
        &self,
        level: Level,
        logger: &str,
        message: &str,
        request_id: Option<String>,
        user_id: Option<Uuid>,
    ) {
        self.log_structured(level, logger, message, HashMap::new(), request_id, user_id);
    }

    /// Process a log entry (background task)
    async fn process_log_entry(entry: AsyncLogRecord) {
        // In a real implementation, you might:
        // - Write to files
        // - Send to external logging services
        // - Store in databases
        // - Forward to monitoring systems

        // For now, just output to tracing
        let level = match entry.level.as_str() {
            "ERROR" => Level::ERROR,
            "WARN" => Level::WARN,
            "INFO" => Level::INFO,
            "DEBUG" => Level::DEBUG,
            _ => Level::INFO,
        };

        match level {
            Level::ERROR => error!(
                logger = entry.logger,
                request_id = entry.request_id,
                user_id = ?entry.user_id,
                trace_id = entry.trace_id,
                fields = ?entry.fields,
                "{}",
                entry.message
            ),
            Level::WARN => warn!(
                logger = entry.logger,
                request_id = entry.request_id,
                user_id = ?entry.user_id,
                trace_id = entry.trace_id,
                fields = ?entry.fields,
                "{}",
                entry.message
            ),
            Level::INFO => info!(
                logger = entry.logger,
                request_id = entry.request_id,
                user_id = ?entry.user_id,
                trace_id = entry.trace_id,
                fields = ?entry.fields,
                "{}",
                entry.message
            ),
            Level::DEBUG => debug!(
                logger = entry.logger,
                request_id = entry.request_id,
                user_id = ?entry.user_id,
                trace_id = entry.trace_id,
                fields = ?entry.fields,
                "{}",
                entry.message
            ),
            _ => info!(
                logger = entry.logger,
                request_id = entry.request_id,
                user_id = ?entry.user_id,
                trace_id = entry.trace_id,
                fields = ?entry.fields,
                "{}",
                entry.message
            ),
        }
    }

    /// Get current trace ID from tracing context
    fn current_trace_id() -> Option<String> {
        // In a real implementation, extract from tracing span context
        // For now, return None
        None
    }
}

/// Global async logger instance
#[allow(dead_code)]
static ASYNC_LOGGER: OnceLock<AsyncLogger> = OnceLock::new();

/// Initialize the global async logger
#[allow(dead_code)]
pub fn init_async_logger(config: AsyncLoggerConfig) {
    ASYNC_LOGGER.get_or_init(|| AsyncLogger::new(config));
}

/// Get the global async logger
#[allow(dead_code)]
pub fn async_logger() -> Option<&'static AsyncLogger> {
    ASYNC_LOGGER.get()
}
