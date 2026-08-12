//! Batch processing system for handling multiple requests efficiently
//!
//! This module provides batch processing capabilities for chat completions,
//! embeddings, and other API operations.

mod async_batch;
mod processor;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use async_batch::{
    AsyncBatchConfig, AsyncBatchError, AsyncBatchExecutor, AsyncBatchItemResult, AsyncBatchSummary,
    batch_execute,
};
#[deprecated(
    since = "0.6.0",
    note = "BatchProcessor is unreachable from the gateway and is scheduled for removal in 0.7.0; use the /v1/batches provider proxy"
)]
pub use processor::core::BatchProcessor;
pub use types::{
    BatchError, BatchHttpResponse, BatchItem, BatchRecord, BatchRequest, BatchRequestCounts,
    BatchResponse, BatchResult, BatchStatus, BatchType,
};
