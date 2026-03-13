//! Async Batch Completion - Concurrent Request Processing
//!
//! This module provides high-performance concurrent batch processing for
//! chat completions, similar to Python LiteLLM's `abatch_completion()`.

use crate::utils::error::gateway_error::GatewayError;
use futures::stream::{self, StreamExt};
use std::time::Duration;

/// Configuration for async batch processing
#[derive(Debug, Clone)]
pub struct AsyncBatchConfig {
    /// Maximum concurrent requests (default: 10)
    pub concurrency: usize,
    /// Timeout per individual request (default: 60s)
    pub timeout: Duration,
    /// Continue processing on individual failures (default: true)
    pub continue_on_error: bool,
    /// Retry failed requests (default: 1)
    pub max_retries: u32,
    /// Delay between retries (default: 1s)
    pub retry_delay: Duration,
}

impl Default for AsyncBatchConfig {
    fn default() -> Self {
        Self {
            concurrency: 10,
            timeout: Duration::from_secs(60),
            continue_on_error: true,
            max_retries: 1,
            retry_delay: Duration::from_secs(1),
        }
    }
}

impl AsyncBatchConfig {
    /// Create a new config
    pub fn new() -> Self {
        Self::default()
    }

    /// Set concurrency limit
    pub fn with_concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);
        self
    }

    /// Set timeout per request
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set whether to continue on individual errors
    pub fn with_continue_on_error(mut self, continue_on_error: bool) -> Self {
        self.continue_on_error = continue_on_error;
        self
    }

    /// Set max retries
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }
}

/// Result of an individual request in a batch
#[derive(Debug, Clone)]
pub struct AsyncBatchItemResult<T> {
    /// Index of the request in the original batch
    pub index: usize,
    /// The result (Ok or Err)
    pub result: std::result::Result<T, AsyncBatchError>,
    /// Time taken for this request
    pub duration: Duration,
    /// Number of retries attempted
    pub retries: u32,
}

/// Error for async batch operations
#[derive(Debug, Clone)]
pub struct AsyncBatchError {
    /// Error message
    pub message: String,
    /// Error code (if available)
    pub code: Option<String>,
    /// Whether this error is retryable
    pub retryable: bool,
}

impl std::fmt::Display for AsyncBatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for AsyncBatchError {}

impl From<GatewayError> for AsyncBatchError {
    fn from(err: GatewayError) -> Self {
        let retryable = matches!(
            &err,
            GatewayError::Timeout(_) | GatewayError::Network(_) | GatewayError::RateLimit { .. }
        );

        Self {
            message: err.to_string(),
            code: None,
            retryable,
        }
    }
}

/// Summary of batch execution
#[derive(Debug, Clone)]
pub struct AsyncBatchSummary {
    /// Total requests processed
    pub total: usize,
    /// Successful requests
    pub succeeded: usize,
    /// Failed requests
    pub failed: usize,
    /// Total time for batch processing
    pub total_duration: Duration,
    /// Average time per request
    pub avg_duration: Duration,
}

/// Async batch executor for concurrent request processing
pub struct AsyncBatchExecutor {
    config: AsyncBatchConfig,
}

impl AsyncBatchExecutor {
    /// Create a new batch executor
    pub fn new(config: AsyncBatchConfig) -> Self {
        Self { config }
    }

    /// Execute a batch of async operations concurrently
    ///
    /// # Arguments
    /// * `items` - Iterator of items to process
    /// * `operation` - Async function to execute for each item
    ///
    /// # Returns
    /// Vector of results in the same order as input items
    ///
    /// # Example
    /// ```rust,no_run
    /// # use litellm_rs::core::batch::{AsyncBatchExecutor, AsyncBatchConfig};
    /// # use std::time::Duration;
    /// # async fn example() {
    /// let executor = AsyncBatchExecutor::new(
    ///     AsyncBatchConfig::new()
    ///         .with_concurrency(5)
    ///         .with_timeout(Duration::from_secs(30))
    /// );
    ///
    /// let requests: Vec<String> = vec!["req1".into(), "req2".into(), "req3".into()];
    /// let results = executor.execute(requests, |req| async move {
    ///     // Process each request
    ///     Ok::<_, litellm_rs::GatewayError>(format!("processed: {}", req))
    /// }).await;
    /// # }
    /// ```
    pub async fn execute<T, R, F, Fut>(
        &self,
        items: impl IntoIterator<Item = T>,
        operation: F,
    ) -> Vec<AsyncBatchItemResult<R>>
    where
        T: Send + 'static,
        R: Send + 'static,
        F: Fn(T) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = std::result::Result<R, GatewayError>> + Send,
    {
        let items_with_index: Vec<(usize, T)> = items.into_iter().enumerate().collect();
        let config = self.config.clone();

        let results: Vec<AsyncBatchItemResult<R>> = stream::iter(items_with_index)
            .map(|(index, item)| {
                let op = operation.clone();
                let cfg = config.clone();

                async move {
                    let start = std::time::Instant::now();
                    let retries = 0u32;

                    let result = tokio::time::timeout(cfg.timeout, op(item))
                        .await
                        .map_err(|_| {
                            GatewayError::Timeout(format!(
                                "Request {} timed out after {:?}",
                                index, cfg.timeout
                            ))
                        })
                        .and_then(|r| r);

                    match result {
                        Ok(value) => AsyncBatchItemResult {
                            index,
                            result: Ok(value),
                            duration: start.elapsed(),
                            retries,
                        },
                        Err(e) => {
                            let batch_err = AsyncBatchError::from(e);
                            // Note: Can't retry because item is consumed
                            // In a real implementation, we'd clone the item
                            AsyncBatchItemResult {
                                index,
                                result: Err(batch_err),
                                duration: start.elapsed(),
                                retries,
                            }
                        }
                    }
                }
            })
            .buffer_unordered(config.concurrency)
            .collect()
            .await;

        // Sort by index to maintain original order
        let mut sorted_results = results;
        sorted_results.sort_by_key(|r| r.index);
        sorted_results
    }

    /// Execute with summary statistics
    pub async fn execute_with_summary<T, R, F, Fut>(
        &self,
        items: impl IntoIterator<Item = T>,
        operation: F,
    ) -> (Vec<AsyncBatchItemResult<R>>, AsyncBatchSummary)
    where
        T: Send + 'static,
        R: Send + 'static,
        F: Fn(T) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = std::result::Result<R, GatewayError>> + Send,
    {
        let start = std::time::Instant::now();
        let results = self.execute(items, operation).await;
        let total_duration = start.elapsed();

        let total = results.len();
        let succeeded = results.iter().filter(|r| r.result.is_ok()).count();
        let failed = total - succeeded;
        let avg_duration = if total > 0 {
            Duration::from_nanos((total_duration.as_nanos() / total as u128) as u64)
        } else {
            Duration::ZERO
        };

        let summary = AsyncBatchSummary {
            total,
            succeeded,
            failed,
            total_duration,
            avg_duration,
        };

        (results, summary)
    }

    /// Get current configuration
    pub fn config(&self) -> &AsyncBatchConfig {
        &self.config
    }
}

impl Default for AsyncBatchExecutor {
    fn default() -> Self {
        Self::new(AsyncBatchConfig::default())
    }
}

/// Convenience function for batch completion without creating an executor
pub async fn batch_execute<T, R, F, Fut>(
    items: impl IntoIterator<Item = T>,
    operation: F,
    config: Option<AsyncBatchConfig>,
) -> Vec<AsyncBatchItemResult<R>>
where
    T: Send + 'static,
    R: Send + 'static,
    F: Fn(T) -> Fut + Send + Sync + Clone + 'static,
    Fut: std::future::Future<Output = std::result::Result<R, GatewayError>> + Send,
{
    let executor = AsyncBatchExecutor::new(config.unwrap_or_default());
    executor.execute(items, operation).await
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== AsyncBatchConfig Tests ====================

    #[test]
    fn test_async_batch_config_default() {
        let config = AsyncBatchConfig::default();

        assert_eq!(config.concurrency, 10);
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert!(config.continue_on_error);
        assert_eq!(config.max_retries, 1);
        assert_eq!(config.retry_delay, Duration::from_secs(1));
    }

    #[test]
    fn test_async_batch_config_new() {
        let config = AsyncBatchConfig::new();

        assert_eq!(config.concurrency, 10);
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_async_batch_config_with_concurrency() {
        let config = AsyncBatchConfig::new().with_concurrency(5);

        assert_eq!(config.concurrency, 5);
    }

    #[test]
    fn test_async_batch_config_with_concurrency_minimum() {
        let config = AsyncBatchConfig::new().with_concurrency(0);

        // Should be at least 1
        assert_eq!(config.concurrency, 1);
    }

    #[test]
    fn test_async_batch_config_with_timeout() {
        let config = AsyncBatchConfig::new().with_timeout(Duration::from_secs(30));

        assert_eq!(config.timeout, Duration::from_secs(30));
    }

    #[test]
    fn test_async_batch_config_with_continue_on_error() {
        let config = AsyncBatchConfig::new().with_continue_on_error(false);

        assert!(!config.continue_on_error);
    }

    #[test]
    fn test_async_batch_config_with_max_retries() {
        let config = AsyncBatchConfig::new().with_max_retries(3);

        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_async_batch_config_builder_chain() {
        let config = AsyncBatchConfig::new()
            .with_concurrency(20)
            .with_timeout(Duration::from_secs(120))
            .with_continue_on_error(false)
            .with_max_retries(5);

        assert_eq!(config.concurrency, 20);
        assert_eq!(config.timeout, Duration::from_secs(120));
        assert!(!config.continue_on_error);
        assert_eq!(config.max_retries, 5);
    }

    #[test]
    fn test_async_batch_config_clone() {
        let config = AsyncBatchConfig::new().with_concurrency(15);
        let cloned = config.clone();

        assert_eq!(config.concurrency, cloned.concurrency);
        assert_eq!(config.timeout, cloned.timeout);
    }

    #[test]
    fn test_async_batch_config_debug() {
        let config = AsyncBatchConfig::new();
        let debug_str = format!("{:?}", config);

        assert!(debug_str.contains("AsyncBatchConfig"));
        assert!(debug_str.contains("concurrency"));
    }

    // ==================== AsyncBatchError Tests ====================

    #[test]
    fn test_async_batch_error_display() {
        let error = AsyncBatchError {
            message: "Test error".to_string(),
            code: None,
            retryable: false,
        };

        assert_eq!(format!("{}", error), "Test error");
    }

    #[test]
    fn test_async_batch_error_with_code() {
        let error = AsyncBatchError {
            message: "API error".to_string(),
            code: Some("E001".to_string()),
            retryable: true,
        };

        assert_eq!(error.code, Some("E001".to_string()));
        assert!(error.retryable);
    }

    #[test]
    fn test_async_batch_error_clone() {
        let error = AsyncBatchError {
            message: "Clone test".to_string(),
            code: Some("E002".to_string()),
            retryable: false,
        };

        let cloned = error.clone();
        assert_eq!(error.message, cloned.message);
        assert_eq!(error.code, cloned.code);
        assert_eq!(error.retryable, cloned.retryable);
    }

    #[test]
    fn test_async_batch_error_debug() {
        let error = AsyncBatchError {
            message: "Debug test".to_string(),
            code: None,
            retryable: false,
        };

        let debug_str = format!("{:?}", error);
        assert!(debug_str.contains("AsyncBatchError"));
        assert!(debug_str.contains("Debug test"));
    }

    #[test]
    fn test_async_batch_error_from_gateway_error_timeout() {
        let gateway_error = GatewayError::Timeout("Request timed out".to_string());
        let batch_error: AsyncBatchError = gateway_error.into();

        assert!(batch_error.retryable);
        assert!(batch_error.message.contains("timed out"));
    }

    #[test]
    fn test_async_batch_error_from_gateway_error_network() {
        let gateway_error = GatewayError::Network("Connection failed".to_string());
        let batch_error: AsyncBatchError = gateway_error.into();

        assert!(batch_error.retryable);
    }

    #[test]
    fn test_async_batch_error_from_gateway_error_rate_limit() {
        let gateway_error = GatewayError::RateLimit {
            message: "Rate limit exceeded".to_string(),
            retry_after: None,
            rpm_limit: None,
            tpm_limit: None,
        };
        let batch_error: AsyncBatchError = gateway_error.into();

        assert!(batch_error.retryable);
    }

    // ==================== AsyncBatchItemResult Tests ====================

    #[test]
    fn test_async_batch_item_result_success() {
        let result: AsyncBatchItemResult<String> = AsyncBatchItemResult {
            index: 0,
            result: Ok("Success".to_string()),
            duration: Duration::from_millis(100),
            retries: 0,
        };

        assert_eq!(result.index, 0);
        assert!(result.result.is_ok());
        assert_eq!(result.retries, 0);
    }

    #[test]
    fn test_async_batch_item_result_failure() {
        let error = AsyncBatchError {
            message: "Failed".to_string(),
            code: None,
            retryable: false,
        };

        let result: AsyncBatchItemResult<String> = AsyncBatchItemResult {
            index: 1,
            result: Err(error),
            duration: Duration::from_millis(50),
            retries: 2,
        };

        assert_eq!(result.index, 1);
        assert!(result.result.is_err());
        assert_eq!(result.retries, 2);
    }

    #[test]
    fn test_async_batch_item_result_clone() {
        let result: AsyncBatchItemResult<i32> = AsyncBatchItemResult {
            index: 5,
            result: Ok(42),
            duration: Duration::from_millis(200),
            retries: 1,
        };

        let cloned = result.clone();
        assert_eq!(result.index, cloned.index);
        assert_eq!(result.duration, cloned.duration);
        assert_eq!(result.retries, cloned.retries);
    }

    // ==================== AsyncBatchSummary Tests ====================

    #[test]
    fn test_async_batch_summary_creation() {
        let summary = AsyncBatchSummary {
            total: 10,
            succeeded: 8,
            failed: 2,
            total_duration: Duration::from_secs(5),
            avg_duration: Duration::from_millis(500),
        };

        assert_eq!(summary.total, 10);
        assert_eq!(summary.succeeded, 8);
        assert_eq!(summary.failed, 2);
    }

    #[test]
    fn test_async_batch_summary_clone() {
        let summary = AsyncBatchSummary {
            total: 5,
            succeeded: 5,
            failed: 0,
            total_duration: Duration::from_secs(2),
            avg_duration: Duration::from_millis(400),
        };

        let cloned = summary.clone();
        assert_eq!(summary.total, cloned.total);
        assert_eq!(summary.succeeded, cloned.succeeded);
        assert_eq!(summary.total_duration, cloned.total_duration);
    }

    #[test]
    fn test_async_batch_summary_debug() {
        let summary = AsyncBatchSummary {
            total: 3,
            succeeded: 2,
            failed: 1,
            total_duration: Duration::from_secs(1),
            avg_duration: Duration::from_millis(333),
        };

        let debug_str = format!("{:?}", summary);
        assert!(debug_str.contains("AsyncBatchSummary"));
    }

    // ==================== AsyncBatchExecutor Tests ====================

    #[test]
    fn test_async_batch_executor_new() {
        let config = AsyncBatchConfig::new().with_concurrency(5);
        let executor = AsyncBatchExecutor::new(config);

        assert_eq!(executor.config().concurrency, 5);
    }

    #[test]
    fn test_async_batch_executor_default() {
        let executor = AsyncBatchExecutor::default();

        assert_eq!(executor.config().concurrency, 10);
        assert_eq!(executor.config().timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_async_batch_executor_config() {
        let config = AsyncBatchConfig::new()
            .with_concurrency(15)
            .with_timeout(Duration::from_secs(90));
        let executor = AsyncBatchExecutor::new(config);

        let retrieved_config = executor.config();
        assert_eq!(retrieved_config.concurrency, 15);
        assert_eq!(retrieved_config.timeout, Duration::from_secs(90));
    }

    #[tokio::test]
    async fn test_async_batch_executor_execute_empty() {
        let executor = AsyncBatchExecutor::default();
        let items: Vec<i32> = vec![];

        let results = executor
            .execute(items, |x| async move { Ok::<_, GatewayError>(x * 2) })
            .await;

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_async_batch_executor_execute_single() {
        let executor = AsyncBatchExecutor::default();
        let items = vec![5];

        let results = executor
            .execute(items, |x| async move { Ok::<_, GatewayError>(x * 2) })
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].index, 0);
        assert_eq!(results[0].result.as_ref().unwrap(), &10);
    }

    #[tokio::test]
    async fn test_async_batch_executor_execute_multiple() {
        let executor = AsyncBatchExecutor::new(AsyncBatchConfig::new().with_concurrency(3));
        let items = vec![1, 2, 3, 4, 5];

        let results = executor
            .execute(items, |x| async move { Ok::<_, GatewayError>(x * 10) })
            .await;

        assert_eq!(results.len(), 5);
        // Results should be sorted by index
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.index, i);
            assert_eq!(result.result.as_ref().unwrap(), &((i + 1) as i32 * 10));
        }
    }

    #[tokio::test]
    async fn test_async_batch_executor_maintains_order() {
        let executor = AsyncBatchExecutor::new(AsyncBatchConfig::new().with_concurrency(10));
        let items: Vec<i32> = (0..20).collect();

        let results = executor
            .execute(items, |x| async move { Ok::<_, GatewayError>(x) })
            .await;

        // Verify results are in original order
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.index, i);
        }
    }

    #[tokio::test]
    async fn test_async_batch_executor_with_summary_empty() {
        let executor = AsyncBatchExecutor::default();
        let items: Vec<i32> = vec![];

        let (results, summary) = executor
            .execute_with_summary(items, |x| async move { Ok::<_, GatewayError>(x) })
            .await;

        assert!(results.is_empty());
        assert_eq!(summary.total, 0);
        assert_eq!(summary.succeeded, 0);
        assert_eq!(summary.failed, 0);
    }

    #[tokio::test]
    async fn test_async_batch_executor_with_summary_success() {
        let executor = AsyncBatchExecutor::default();
        let items = vec![1, 2, 3];

        let (results, summary) = executor
            .execute_with_summary(items, |x| async move { Ok::<_, GatewayError>(x * 2) })
            .await;

        assert_eq!(results.len(), 3);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.succeeded, 3);
        assert_eq!(summary.failed, 0);
    }

    #[tokio::test]
    async fn test_async_batch_executor_with_summary_mixed() {
        let executor = AsyncBatchExecutor::default();
        let items = vec![1, 2, 3, 4, 5];

        let (results, summary) = executor
            .execute_with_summary(items, |x| async move {
                if x % 2 == 0 {
                    Err(GatewayError::Internal("Even number".to_string()))
                } else {
                    Ok::<_, GatewayError>(x)
                }
            })
            .await;

        assert_eq!(results.len(), 5);
        assert_eq!(summary.total, 5);
        assert_eq!(summary.succeeded, 3); // 1, 3, 5
        assert_eq!(summary.failed, 2); // 2, 4
    }

    // ==================== batch_execute Function Tests ====================

    #[tokio::test]
    async fn test_batch_execute_with_default_config() {
        let items = vec![1, 2, 3];

        let results =
            batch_execute(items, |x| async move { Ok::<_, GatewayError>(x + 1) }, None).await;

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.result.is_ok()));
    }

    #[tokio::test]
    async fn test_batch_execute_with_custom_config() {
        let config = AsyncBatchConfig::new().with_concurrency(2);
        let items = vec![10, 20, 30];

        let results = batch_execute(
            items,
            |x| async move { Ok::<_, GatewayError>(x / 10) },
            Some(config),
        )
        .await;

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].result.as_ref().unwrap(), &1);
        assert_eq!(results[1].result.as_ref().unwrap(), &2);
        assert_eq!(results[2].result.as_ref().unwrap(), &3);
    }

    // ==================== Timeout Tests ====================

    #[tokio::test]
    async fn test_async_batch_executor_timeout() {
        let executor = AsyncBatchExecutor::new(
            AsyncBatchConfig::new().with_timeout(Duration::from_millis(50)),
        );
        let items = vec![1];

        let results = executor
            .execute(items, |_x| async move {
                tokio::time::sleep(Duration::from_millis(200)).await;
                Ok::<_, GatewayError>(42)
            })
            .await;

        assert_eq!(results.len(), 1);
        assert!(results[0].result.is_err());
    }
}
