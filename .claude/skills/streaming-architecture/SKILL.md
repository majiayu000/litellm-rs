---
name: streaming-architecture
description: LiteLLM-RS Streaming Architecture. Covers UnifiedSSEParser, SSETransformer trait, VecDeque buffering, provider-specific transformers, and real-time event handling. Use when debugging SSE parsing, writing or modifying a provider stream transformer, wiring the stream processing pipeline, or tuning buffer sizes, timeouts, and backpressure.
---

# Streaming Architecture Guide

## Overview

LiteLLM-RS implements a unified streaming system that handles Server-Sent Events (SSE) from 66+ providers with provider-specific transformations while presenting a consistent OpenAI-compatible output format.

### Streaming Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    Provider SSE Stream                          │
│  (OpenAI, Anthropic, Google, etc.)                             │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    UnifiedSSEParser                             │
│  - Buffer management with VecDeque                              │
│  - Line-based SSE parsing                                       │
│  - Event type detection                                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    SSETransformer                               │
│  - Provider-specific data parsing                               │
│  - Format normalization to ChatChunk                            │
│  - Error handling                                               │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    OpenAI-Compatible Output                     │
│  ChatChunk (data: {...}\n\n or [DONE])                         │
└─────────────────────────────────────────────────────────────────┘
```

---

## Core Components

### UnifiedSSEParser

```rust
use std::collections::VecDeque;

/// Unified SSE parser that handles various provider formats
pub struct UnifiedSSEParser {
    /// Buffer for incomplete lines
    buffer: VecDeque<u8>,
    /// Current event type being parsed
    current_event_type: Option<String>,
    /// Maximum buffer size to prevent memory issues
    max_buffer_size: usize,
}

impl UnifiedSSEParser {
    pub fn new() -> Self {
        Self {
            buffer: VecDeque::with_capacity(8192),
            current_event_type: None,
            max_buffer_size: 1024 * 1024, // 1MB max buffer
        }
    }

    /// Feed bytes into the parser and extract complete events
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<SSEEvent> {
        // Add bytes to buffer
        for &byte in bytes {
            if self.buffer.len() < self.max_buffer_size {
                self.buffer.push_back(byte);
            }
        }

        self.extract_events()
    }

    /// Extract complete SSE events from the buffer
    fn extract_events(&mut self) -> Vec<SSEEvent> {
        let mut events = Vec::new();
        let mut current_data = String::new();

        // Convert buffer to string for processing
        let text: String = self.buffer.iter().map(|&b| b as char).collect();

        // Process line by line
        let mut processed_len = 0;
        for line in text.split('\n') {
            processed_len += line.len() + 1; // +1 for \n

            let line = line.trim_end_matches('\r');

            if line.is_empty() {
                // Empty line marks end of event
                if !current_data.is_empty() {
                    events.push(SSEEvent {
                        event_type: self.current_event_type.take(),
                        data: current_data.clone(),
                    });
                    current_data.clear();
                }
                continue;
            }

            if let Some(event_type) = line.strip_prefix("event: ") {
                self.current_event_type = Some(event_type.to_string());
            } else if let Some(data) = line.strip_prefix("data: ") {
                if !current_data.is_empty() {
                    current_data.push('\n');
                }
                current_data.push_str(data);
            } else if line.starts_with(':') {
                // Comment line, ignore
                continue;
            } else if let Some(id) = line.strip_prefix("id: ") {
                // Event ID, can be stored if needed
                let _ = id;
            } else if let Some(retry) = line.strip_prefix("retry: ") {
                // Retry interval, can be stored if needed
                let _ = retry;
            }
        }

        // Remove processed bytes from buffer
        // Keep any incomplete line
        let last_newline = text.rfind('\n').map(|i| i + 1).unwrap_or(0);
        for _ in 0..last_newline {
            self.buffer.pop_front();
        }

        events
    }

    /// Reset parser state
    pub fn reset(&mut self) {
        self.buffer.clear();
        self.current_event_type = None;
    }
}

#[derive(Debug, Clone)]
pub struct SSEEvent {
    pub event_type: Option<String>,
    pub data: String,
}
```

---

## SSETransformer Trait

```rust
use crate::core::types::responses::ChatChunk;

/// Trait for transforming provider-specific SSE data to ChatChunk
#[async_trait]
pub trait SSETransformer: Send + Sync {
    /// Transform raw SSE data to ChatChunk
    fn transform(&self, event: &SSEEvent) -> Result<Option<ChatChunk>, StreamError>;

    /// Check if the event indicates stream end
    fn is_done(&self, event: &SSEEvent) -> bool;

    /// Get provider name for error context
    fn provider_name(&self) -> &'static str;

    /// Handle provider-specific error events
    fn handle_error(&self, event: &SSEEvent) -> Option<StreamError>;
}

#[derive(Debug, thiserror::Error)]
pub enum StreamError {
    #[error("[{provider}] Parse error: {message}")]
    Parse {
        provider: &'static str,
        message: String,
    },

    #[error("[{provider}] Stream interrupted: {message}")]
    Interrupted {
        provider: &'static str,
        message: String,
    },

    #[error("[{provider}] Provider error: {message}")]
    ProviderError {
        provider: &'static str,
        message: String,
    },
}
```

---

## References
- [reference/provider-transformers.md](reference/provider-transformers.md) — OpenAI, Anthropic, and Google Gemini SSETransformer implementations
- [reference/stream-pipeline.md](reference/stream-pipeline.md) — StreamProcessor pipeline and actix HTTP response streaming
- [reference/buffer-management.md](reference/buffer-management.md) — VecDeque buffer trimming and overflow protection internals
- [reference/configuration.md](reference/configuration.md) — streaming YAML configuration: buffer sizes, timeouts, retry
- [reference/best-practices.md](reference/best-practices.md) — incomplete events, stream ordering, resource cleanup, backpressure
