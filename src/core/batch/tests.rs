//! Tests for batch processing

#[cfg(test)]
use super::async_batch::*;
use super::types::*;
use crate::utils::error::gateway_error::GatewayError;
use std::time::Duration;

#[test]
fn test_batch_status_transitions() {
    assert_eq!(BatchStatus::Validating, BatchStatus::Validating);
    assert_ne!(BatchStatus::Validating, BatchStatus::InProgress);
}

// Async Batch Tests

#[test]
fn test_async_batch_config_builder() {
    let config = AsyncBatchConfig::new()
        .with_concurrency(20)
        .with_timeout(Duration::from_secs(120))
        .with_continue_on_error(false)
        .with_max_retries(3);

    assert_eq!(config.concurrency, 20);
    assert_eq!(config.timeout, Duration::from_secs(120));
    assert!(!config.continue_on_error);
    assert_eq!(config.max_retries, 3);
}

#[test]
fn test_async_batch_config_min_concurrency() {
    let config = AsyncBatchConfig::new().with_concurrency(0);
    assert_eq!(config.concurrency, 1); // Should be at least 1
}

#[tokio::test]
async fn test_async_batch_executor_success() {
    let executor = AsyncBatchExecutor::new(
        AsyncBatchConfig::new()
            .with_concurrency(2)
            .with_timeout(Duration::from_secs(5)),
    );

    let items = vec![1, 2, 3, 4, 5];

    let results = executor
        .execute(items, |n| async move {
            // Simulate async work
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok::<_, GatewayError>(n * 2)
        })
        .await;

    assert_eq!(results.len(), 5);

    // Check results are in order
    for (i, result) in results.iter().enumerate() {
        assert_eq!(result.index, i);
        assert!(result.result.is_ok());
        assert_eq!(result.result.as_ref().unwrap(), &((i + 1) * 2));
    }
}

#[tokio::test]
async fn test_async_batch_executor_with_failures() {
    let executor = AsyncBatchExecutor::new(AsyncBatchConfig::new().with_concurrency(2));

    let items = vec![1, 2, 3, 4, 5];

    let results = executor
        .execute(items, |n| async move {
            if n == 3 {
                Err(GatewayError::BadRequest("Test error".to_string()))
            } else {
                Ok::<_, GatewayError>(n * 2)
            }
        })
        .await;

    assert_eq!(results.len(), 5);

    // Check that index 2 (value 3) failed
    let failed = results.iter().find(|r| r.index == 2).unwrap();
    assert!(failed.result.is_err());

    // Others should succeed
    let succeeded: Vec<_> = results.iter().filter(|r| r.result.is_ok()).collect();
    assert_eq!(succeeded.len(), 4);
}

#[tokio::test]
async fn test_async_batch_executor_with_summary() {
    let executor = AsyncBatchExecutor::new(AsyncBatchConfig::new().with_concurrency(3));

    let items = vec![1, 2, 3, 4, 5];

    let (results, summary) = executor
        .execute_with_summary(items, |n| async move {
            if n % 2 == 0 {
                Err(GatewayError::BadRequest("Even number".to_string()))
            } else {
                Ok::<_, GatewayError>(n)
            }
        })
        .await;

    assert_eq!(results.len(), 5);
    assert_eq!(summary.total, 5);
    assert_eq!(summary.succeeded, 3); // 1, 3, 5
    assert_eq!(summary.failed, 2); // 2, 4
}

#[tokio::test]
async fn test_batch_execute_convenience_fn() {
    let items = vec![10, 20, 30];

    let results = batch_execute(
        items,
        |n| async move { Ok::<_, GatewayError>(n + 1) },
        Some(AsyncBatchConfig::new().with_concurrency(2)),
    )
    .await;

    assert_eq!(results.len(), 3);
    assert_eq!(results[0].result.as_ref().unwrap(), &11);
    assert_eq!(results[1].result.as_ref().unwrap(), &21);
    assert_eq!(results[2].result.as_ref().unwrap(), &31);
}

#[test]
fn test_async_batch_error_from_gateway_error() {
    let timeout_err = GatewayError::Timeout("timeout".to_string());
    let batch_err: AsyncBatchError = timeout_err.into();
    assert!(batch_err.retryable);

    let invalid_err = GatewayError::BadRequest("invalid".to_string());
    let batch_err: AsyncBatchError = invalid_err.into();
    assert!(!batch_err.retryable);
}
