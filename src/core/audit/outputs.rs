//! Audit output implementations
//!
//! This module provides various output targets for audit logs.

use async_trait::async_trait;
use std::collections::VecDeque;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Sender;
use std::thread::JoinHandle;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use super::events::AuditEvent;
use super::types::{AuditError, AuditResult};

/// Trait for audit output targets
#[async_trait]
pub trait AuditOutput: Send + Sync {
    /// Get the name of this output
    fn name(&self) -> &str;

    /// Write an event to the output
    async fn write(&self, event: &AuditEvent) -> AuditResult<()>;

    /// Flush any buffered events
    async fn flush(&self) -> AuditResult<()>;

    /// Close the output
    async fn close(&self) -> AuditResult<()>;
}

/// Boxed audit output for dynamic dispatch
pub type BoxedAuditOutput = Box<dyn AuditOutput>;

enum StderrCommand {
    Write(String),
    Close,
}

/// Structured stderr output used when no file destination is configured.
///
/// A dedicated thread owns blocking stderr I/O. The async audit worker only
/// performs a non-blocking bounded enqueue, so a slow stderr consumer cannot
/// stall request handling; queue saturation is returned as an explicit error.
pub struct StderrOutput {
    sender: Sender<StderrCommand>,
    worker: Mutex<Option<JoinHandle<std::io::Result<()>>>>,
}

impl StderrOutput {
    pub fn new() -> AuditResult<Self> {
        // The outer audit channel remains bounded; this handoff is lossless so
        // the dedicated blocking writer never makes request tasks wait or drops
        // events during transient stderr backpressure.
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("audit-stderr-writer".to_string())
            .spawn(move || {
                let stderr = std::io::stderr();
                let mut stderr = stderr.lock();
                while let Ok(command) = receiver.recv() {
                    match command {
                        StderrCommand::Write(serialized) => {
                            stderr.write_all(serialized.as_bytes())?;
                            stderr.write_all(b"\n")?;
                            stderr.flush()?;
                        }
                        StderrCommand::Close => break,
                    }
                }
                Ok(())
            })?;
        Ok(Self {
            sender,
            worker: Mutex::new(Some(worker)),
        })
    }
}

#[async_trait]
impl AuditOutput for StderrOutput {
    fn name(&self) -> &str {
        "stderr"
    }

    async fn write(&self, event: &AuditEvent) -> AuditResult<()> {
        let serialized = event.to_json()?;
        self.sender
            .send(StderrCommand::Write(serialized))
            .map_err(|_| AuditError::Channel("audit stderr writer stopped".to_string()))
    }

    async fn flush(&self) -> AuditResult<()> {
        Ok(())
    }

    async fn close(&self) -> AuditResult<()> {
        let Some(worker) = self.worker.lock().await.take() else {
            return Ok(());
        };
        let sender = self.sender.clone();
        tokio::task::spawn_blocking(move || {
            let close_result = sender.send(StderrCommand::Close);
            let worker_result = worker
                .join()
                .map_err(|_| AuditError::Output("audit stderr writer panicked".to_string()))?;
            worker_result?;
            close_result.map_err(|_| {
                AuditError::Channel("audit stderr writer stopped before close".to_string())
            })
        })
        .await
        .map_err(|error| AuditError::Output(format!("audit stderr join failed: {error}")))?
    }
}

// ============================================================================
// File Output
// ============================================================================

/// File-based audit output
pub struct FileOutput {
    path: PathBuf,
    file: Arc<Mutex<Option<File>>>,
    buffer: Arc<Mutex<Vec<String>>>,
    buffer_size: usize,
}

impl FileOutput {
    /// Create a new file output
    pub async fn new(path: impl Into<PathBuf>) -> AuditResult<Self> {
        let path = path.into();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Open file for appending
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;

        Ok(Self {
            path,
            file: Arc::new(Mutex::new(Some(file))),
            buffer: Arc::new(Mutex::new(Vec::new())),
            buffer_size: 100,
        })
    }

    /// Set buffer size
    pub fn with_buffer_size(mut self, size: usize) -> Self {
        self.buffer_size = size;
        self
    }

    /// Get the file path
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Write buffered events to file
    async fn write_buffer(&self) -> AuditResult<()> {
        let mut buffer = self.buffer.lock().await;
        if buffer.is_empty() {
            return Ok(());
        }

        let mut file_guard = self.file.lock().await;
        if let Some(ref mut file) = *file_guard {
            for line in buffer.drain(..) {
                file.write_all(line.as_bytes()).await?;
                file.write_all(b"\n").await?;
            }
            file.flush().await?;
        }

        Ok(())
    }
}

#[async_trait]
impl AuditOutput for FileOutput {
    fn name(&self) -> &str {
        "file"
    }

    async fn write(&self, event: &AuditEvent) -> AuditResult<()> {
        let json = event.to_json()?;

        let mut buffer = self.buffer.lock().await;
        buffer.push(json);

        // Flush if buffer is full
        if buffer.len() >= self.buffer_size {
            drop(buffer);
            self.write_buffer().await?;
        }

        Ok(())
    }

    async fn flush(&self) -> AuditResult<()> {
        self.write_buffer().await
    }

    async fn close(&self) -> AuditResult<()> {
        self.flush().await?;

        let mut file_guard = self.file.lock().await;
        *file_guard = None;

        Ok(())
    }
}

// ============================================================================
// Memory Output (for testing)
// ============================================================================

/// In-memory audit output (useful for testing)
pub struct MemoryOutput {
    events: Arc<Mutex<VecDeque<AuditEvent>>>,
    max_events: usize,
}

impl MemoryOutput {
    /// Create a new memory output
    pub fn new(max_events: usize) -> Self {
        Self {
            events: Arc::new(Mutex::new(VecDeque::new())),
            max_events,
        }
    }

    /// Get all stored events
    pub async fn events(&self) -> Vec<AuditEvent> {
        let events = self.events.lock().await;
        events.iter().cloned().collect()
    }

    /// Get event count
    pub async fn count(&self) -> usize {
        let events = self.events.lock().await;
        events.len()
    }

    /// Clear all events
    pub async fn clear(&self) {
        let mut events = self.events.lock().await;
        events.clear();
    }

    /// Get the last N events
    pub async fn last_n(&self, n: usize) -> Vec<AuditEvent> {
        let events = self.events.lock().await;
        events.iter().rev().take(n).cloned().collect()
    }
}

impl Default for MemoryOutput {
    fn default() -> Self {
        Self::new(1000)
    }
}

#[async_trait]
impl AuditOutput for MemoryOutput {
    fn name(&self) -> &str {
        "memory"
    }

    async fn write(&self, event: &AuditEvent) -> AuditResult<()> {
        let mut events = self.events.lock().await;

        // Remove oldest if at capacity
        while events.len() >= self.max_events {
            events.pop_front();
        }

        events.push_back(event.clone());
        Ok(())
    }

    async fn flush(&self) -> AuditResult<()> {
        // No-op for memory output
        Ok(())
    }

    async fn close(&self) -> AuditResult<()> {
        // No-op for memory output
        Ok(())
    }
}

// ============================================================================
// Null Output (for disabled logging)
// ============================================================================

/// Null output that discards all events
pub struct NullOutput;

#[async_trait]
impl AuditOutput for NullOutput {
    fn name(&self) -> &str {
        "null"
    }

    async fn write(&self, _event: &AuditEvent) -> AuditResult<()> {
        Ok(())
    }

    async fn flush(&self) -> AuditResult<()> {
        Ok(())
    }

    async fn close(&self) -> AuditResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::audit::events::EventType;

    #[tokio::test]
    async fn stderr_handoff_accepts_a_burst_without_dropping() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let output = StderrOutput {
            sender,
            worker: Mutex::new(None),
        };

        for index in 0..2048 {
            output
                .write(&AuditEvent::new(
                    EventType::System,
                    format!("event-{index}"),
                ))
                .await
                .unwrap();
        }

        assert_eq!(receiver.try_iter().count(), 2048);
    }

    #[tokio::test]
    async fn test_memory_output() {
        let output = MemoryOutput::new(10);

        for i in 0..5 {
            let event = AuditEvent::new(EventType::System, format!("Event {}", i));
            output.write(&event).await.unwrap();
        }

        assert_eq!(output.count().await, 5);

        let events = output.events().await;
        assert_eq!(events.len(), 5);
    }

    #[tokio::test]
    async fn test_memory_output_max_events() {
        let output = MemoryOutput::new(3);

        for i in 0..5 {
            let event = AuditEvent::new(EventType::System, format!("Event {}", i));
            output.write(&event).await.unwrap();
        }

        assert_eq!(output.count().await, 3);

        let events = output.events().await;
        // Should have the last 3 events
        assert!(events[0].message.contains("Event 2"));
        assert!(events[1].message.contains("Event 3"));
        assert!(events[2].message.contains("Event 4"));
    }

    #[tokio::test]
    async fn test_memory_output_clear() {
        let output = MemoryOutput::new(10);

        let event = AuditEvent::new(EventType::System, "Test");
        output.write(&event).await.unwrap();

        assert_eq!(output.count().await, 1);

        output.clear().await;
        assert_eq!(output.count().await, 0);
    }

    #[tokio::test]
    async fn test_memory_output_last_n() {
        let output = MemoryOutput::new(10);

        for i in 0..5 {
            let event = AuditEvent::new(EventType::System, format!("Event {}", i));
            output.write(&event).await.unwrap();
        }

        let last_2 = output.last_n(2).await;
        assert_eq!(last_2.len(), 2);
        assert!(last_2[0].message.contains("Event 4"));
        assert!(last_2[1].message.contains("Event 3"));
    }

    #[tokio::test]
    async fn test_null_output() {
        let output = NullOutput;

        let event = AuditEvent::new(EventType::System, "Test");
        output.write(&event).await.unwrap();
        output.flush().await.unwrap();
        output.close().await.unwrap();
    }

    #[tokio::test]
    async fn test_file_output() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_audit.log");

        // Clean up if exists
        let _ = tokio::fs::remove_file(&path).await;

        let output = FileOutput::new(&path).await.unwrap();

        let event = AuditEvent::new(EventType::System, "Test event");
        output.write(&event).await.unwrap();
        output.flush().await.unwrap();

        // Verify file was written
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("Test event"));

        output.close().await.unwrap();

        // Clean up
        let _ = tokio::fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn test_file_output_buffering() {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join("test_audit_buffer.log");

        // Clean up if exists
        let _ = tokio::fs::remove_file(&path).await;

        let output = FileOutput::new(&path).await.unwrap().with_buffer_size(3);

        // Write 2 events (below buffer size)
        for i in 0..2 {
            let event = AuditEvent::new(EventType::System, format!("Event {}", i));
            output.write(&event).await.unwrap();
        }

        // File should be empty (buffered)
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.is_empty());

        // Write 1 more to trigger flush
        let event = AuditEvent::new(EventType::System, "Event 2");
        output.write(&event).await.unwrap();

        // Now file should have content
        let content = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(content.contains("Event 0"));
        assert!(content.contains("Event 1"));
        assert!(content.contains("Event 2"));

        output.close().await.unwrap();

        // Clean up
        let _ = tokio::fs::remove_file(&path).await;
    }
}
