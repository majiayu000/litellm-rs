//! Audit Logging System
//!
//! A decoupled audit logging system for tracking all gateway operations.
//!
//! # Features
//!
//! - **Request/Response Logging**: Log all API requests and responses
//! - **User Action Tracking**: Track user actions and authentication events
//! - **Multiple Outputs**: Support for file, database, and custom outputs
//! - **Async Non-blocking**: All logging operations are async
//! - **Configurable Retention**: Control log retention policies
//!
//! # Architecture
//!
//! The audit logging system follows these principles:
//! - **Decoupled**: Logging is independent of business logic
//! - **Extensible**: Easy to add new output targets via the `AuditOutput` trait
//! - **Performant**: Non-blocking async operations
//! - **Structured**: All logs are structured JSON for easy querying
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use litellm_rs::core::audit::{AuditLogger, AuditConfig, AuditEvent};
//!
//! let config = AuditConfig::default()
//!     .enable_file_output("./logs/audit.log")
//!     .enable_request_logging(true);
//!
//! let logger = AuditLogger::new(config).await?;
//!
//! // Log an event
//! logger.log(AuditEvent::request_started("req-123", "/v1/chat/completions")).await?;
//! ```

pub mod config;
pub mod events;
pub mod logger;
#[cfg(feature = "gateway")]
pub mod middleware;
#[cfg(feature = "gateway")]
mod middleware_body;
pub mod outputs;
pub mod types;

#[cfg(test)]
mod tests;

// Re-export main types
pub use config::AuditConfig;
pub use events::{AuditEvent, EventType};
pub use logger::AuditLogger;
#[cfg(feature = "gateway")]
pub use middleware::{AuditMiddleware, AuditMiddlewareService};
pub use outputs::{AuditOutput, FileOutput, MemoryOutput};
pub use types::{AuditError, AuditResult, LogLevel, RequestLog, ResponseLog, UserAction};
