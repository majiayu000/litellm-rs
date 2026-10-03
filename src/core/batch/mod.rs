//! Batch processing system for handling multiple requests efficiently
//!
//! This module provides batch processing capabilities for chat completions,
//! embeddings, and other API operations.

mod async_batch;
mod types;

#[cfg(test)]
mod tests;

// Re-export all public types
pub use async_batch::{
    AsyncBatchConfig, AsyncBatchError, AsyncBatchExecutor, AsyncBatchItemResult, AsyncBatchSummary,
    batch_execute,
};
pub use types::{
    BatchError, BatchHttpResponse, BatchItem, BatchRecord, BatchRequest, BatchRequestCounts,
    BatchResponse, BatchResult, BatchStatus, BatchType,
};
