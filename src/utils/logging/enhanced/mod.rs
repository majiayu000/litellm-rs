//! Enhanced logging utilities with structured logging and performance optimizations
//!
//! This module provides improved logging capabilities including structured logging,
//! log sampling, and async logging to minimize performance impact.

pub mod async_logger;
pub mod macros;
pub mod performance_logger;
pub mod sampler;
pub mod security_logger;
#[cfg(test)]
mod tests;
pub mod types;

// Re-export all public items for backward compatibility
pub use async_logger::{AsyncLogger, async_logger, init_async_logger};
pub use performance_logger::PerformanceLogger;
pub use sampler::LogSampler;
pub use security_logger::SecurityLogger;
pub use types::{AsyncLogRecord, AsyncLoggerConfig, HttpRequestMetrics};
