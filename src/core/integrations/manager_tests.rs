use super::*;
use crate::core::traits::integration::Integration;
use std::sync::atomic::{AtomicU32, Ordering};

/// Mock integration for testing
struct MockIntegration {
    name: &'static str,
    enabled: bool,
    start_count: AtomicU32,
    end_count: AtomicU32,
    error_count: AtomicU32,
    embedding_error_count: AtomicU32,
    flush_count: AtomicU32,
    should_fail: bool,
    embedding_error_delay_ms: u64,
}

impl MockIntegration {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            enabled: true,
            start_count: AtomicU32::new(0),
            end_count: AtomicU32::new(0),
            error_count: AtomicU32::new(0),
            embedding_error_count: AtomicU32::new(0),
            flush_count: AtomicU32::new(0),
            should_fail: false,
            embedding_error_delay_ms: 0,
        }
    }

    fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    fn failing(mut self) -> Self {
        self.should_fail = true;
        self
    }

    fn slow_embedding_error(mut self, delay_ms: u64) -> Self {
        self.embedding_error_delay_ms = delay_ms;
        self
    }
}

#[async_trait::async_trait]
impl Integration for MockIntegration {
    fn name(&self) -> &'static str {
        self.name
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    async fn on_llm_start(&self, _event: &LlmStartEvent) -> IntegrationResult<()> {
        self.start_count.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            Err(IntegrationError::other("Mock failure"))
        } else {
            Ok(())
        }
    }

    async fn on_llm_end(&self, _event: &LlmEndEvent) -> IntegrationResult<()> {
        self.end_count.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            Err(IntegrationError::other("Mock failure"))
        } else {
            Ok(())
        }
    }

    async fn on_llm_error(&self, _event: &LlmErrorEvent) -> IntegrationResult<()> {
        self.error_count.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            Err(IntegrationError::other("Mock failure"))
        } else {
            Ok(())
        }
    }

    async fn on_embedding_error(&self, _event: &EmbeddingErrorEvent) -> IntegrationResult<()> {
        if self.embedding_error_delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(
                self.embedding_error_delay_ms,
            ))
            .await;
        }
        self.embedding_error_count.fetch_add(1, Ordering::SeqCst);
        if self.should_fail {
            Err(IntegrationError::other("Mock failure"))
        } else {
            Ok(())
        }
    }

    async fn flush(&self) -> IntegrationResult<()> {
        self.flush_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn shutdown(&self) -> IntegrationResult<()> {
        Ok(())
    }
}

#[tokio::test]
async fn test_register_integration() {
    let manager = IntegrationManager::with_defaults();
    let integration = Arc::new(MockIntegration::new("test"));

    manager.register(integration).await;

    assert_eq!(manager.count().await, 1);
    assert!(manager.has_integration("test").await);
}

#[tokio::test]
async fn test_register_disabled_integration() {
    let manager = IntegrationManager::with_defaults();
    let integration = Arc::new(MockIntegration::new("disabled").disabled());

    manager.register(integration).await;

    assert_eq!(manager.count().await, 0);
    assert!(!manager.has_integration("disabled").await);
}

#[tokio::test]
async fn test_unregister_integration() {
    let manager = IntegrationManager::with_defaults();
    let integration = Arc::new(MockIntegration::new("test"));

    manager.register(integration).await;
    assert_eq!(manager.count().await, 1);

    let removed = manager.unregister("test").await;
    assert!(removed);
    assert_eq!(manager.count().await, 0);
}

#[tokio::test]
async fn test_list_integrations() {
    let manager = IntegrationManager::with_defaults();
    manager
        .register(Arc::new(MockIntegration::new("integration1")))
        .await;
    manager
        .register(Arc::new(MockIntegration::new("integration2")))
        .await;

    let names = manager.list_integrations().await;
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"integration1"));
    assert!(names.contains(&"integration2"));
}

#[tokio::test]
async fn test_on_llm_start() {
    let manager = IntegrationManager::with_defaults();
    let integration = Arc::new(MockIntegration::new("test"));
    let integration_ref = Arc::clone(&integration);

    manager.register(integration).await;

    let event = LlmStartEvent::new("req-1", "gpt-4");
    manager.on_llm_start(&event).await.unwrap();

    assert_eq!(integration_ref.start_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_on_llm_end() {
    let manager = IntegrationManager::with_defaults();
    let integration = Arc::new(MockIntegration::new("test"));
    let integration_ref = Arc::clone(&integration);

    manager.register(integration).await;

    let event = LlmEndEvent::new("req-1", "gpt-4");
    manager.on_llm_end(&event).await.unwrap();

    assert_eq!(integration_ref.end_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_on_llm_error() {
    let manager = IntegrationManager::with_defaults();
    let integration = Arc::new(MockIntegration::new("test"));
    let integration_ref = Arc::clone(&integration);

    manager.register(integration).await;

    let event = LlmErrorEvent::new("req-1", "gpt-4", "Test error");
    manager.on_llm_error(&event).await.unwrap();

    assert_eq!(integration_ref.error_count.load(Ordering::SeqCst), 1);
}

fn embedding_error_event() -> EmbeddingErrorEvent {
    EmbeddingErrorEvent {
        request_id: "embedding-error".to_string(),
        model: "embedding-model".to_string(),
        provider: Some("provider".to_string()),
        error_message: "failed".to_string(),
        error_type: Some("provider_error".to_string()),
        latency_ms: 25,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
    }
}

#[tokio::test]
async fn embedding_error_parallel_dispatch_times_out_slow_integration() {
    let manager = IntegrationManager::new(
        IntegrationManagerConfig::new()
            .parallel(true)
            .timeout_ms(100)
            .log_errors(false),
    );
    let fast = Arc::new(MockIntegration::new("fast"));
    let fast_ref = Arc::clone(&fast);
    for name in ["slow-1", "slow-2", "slow-3"] {
        manager
            .register(Arc::new(
                MockIntegration::new(name).slow_embedding_error(500),
            ))
            .await;
    }
    manager.register(fast).await;

    tokio::time::timeout(
        std::time::Duration::from_millis(200),
        manager.on_embedding_error(&embedding_error_event()),
    )
    .await
    .expect("manager timeout must bound the slow exporter")
    .unwrap();
    assert_eq!(fast_ref.embedding_error_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn embedding_error_sequential_timeout_continues_when_not_fail_fast() {
    let manager = IntegrationManager::new(
        IntegrationManagerConfig::new()
            .parallel(false)
            .timeout_ms(25)
            .log_errors(false),
    );
    let fast = Arc::new(MockIntegration::new("fast"));
    let fast_ref = Arc::clone(&fast);
    manager
        .register(Arc::new(
            MockIntegration::new("slow").slow_embedding_error(500),
        ))
        .await;
    manager.register(fast).await;

    tokio::time::timeout(
        std::time::Duration::from_millis(150),
        manager.on_embedding_error(&embedding_error_event()),
    )
    .await
    .expect("sequential dispatch must apply the per-integration timeout")
    .unwrap();
    assert_eq!(fast_ref.embedding_error_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_flush() {
    let manager = IntegrationManager::with_defaults();
    let integration = Arc::new(MockIntegration::new("test"));
    let integration_ref = Arc::clone(&integration);

    manager.register(integration).await;
    manager.flush().await.unwrap();

    assert_eq!(integration_ref.flush_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_multiple_integrations() {
    let manager = IntegrationManager::with_defaults();
    let int1 = Arc::new(MockIntegration::new("int1"));
    let int2 = Arc::new(MockIntegration::new("int2"));
    let int1_ref = Arc::clone(&int1);
    let int2_ref = Arc::clone(&int2);

    manager.register(int1).await;
    manager.register(int2).await;

    let event = LlmStartEvent::new("req-1", "gpt-4");
    manager.on_llm_start(&event).await.unwrap();

    assert_eq!(int1_ref.start_count.load(Ordering::SeqCst), 1);
    assert_eq!(int2_ref.start_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_fail_fast_disabled() {
    let config = IntegrationManagerConfig::new()
        .fail_fast(false)
        .log_errors(false);
    let manager = IntegrationManager::new(config);

    manager
        .register(Arc::new(MockIntegration::new("failing").failing()))
        .await;
    manager
        .register(Arc::new(MockIntegration::new("working")))
        .await;

    let event = LlmStartEvent::new("req-1", "gpt-4");
    // Should not fail even though one integration fails
    let result = manager.on_llm_start(&event).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_fail_fast_enabled() {
    let config = IntegrationManagerConfig::new()
        .fail_fast(true)
        .log_errors(false);
    let manager = IntegrationManager::new(config);

    manager
        .register(Arc::new(MockIntegration::new("failing").failing()))
        .await;

    let event = LlmStartEvent::new("req-1", "gpt-4");
    let result = manager.on_llm_start(&event).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_sequential_dispatch() {
    let config = IntegrationManagerConfig::new().parallel(false);
    let manager = IntegrationManager::new(config);

    let int1 = Arc::new(MockIntegration::new("int1"));
    let int2 = Arc::new(MockIntegration::new("int2"));
    let int1_ref = Arc::clone(&int1);
    let int2_ref = Arc::clone(&int2);

    manager.register(int1).await;
    manager.register(int2).await;

    let event = LlmStartEvent::new("req-1", "gpt-4");
    manager.on_llm_start(&event).await.unwrap();

    assert_eq!(int1_ref.start_count.load(Ordering::SeqCst), 1);
    assert_eq!(int2_ref.start_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_empty_manager() {
    let manager = IntegrationManager::with_defaults();

    let event = LlmStartEvent::new("req-1", "gpt-4");
    let result = manager.on_llm_start(&event).await;
    assert!(result.is_ok());
}
