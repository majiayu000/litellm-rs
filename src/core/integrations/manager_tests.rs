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
    flush_count: AtomicU32,
    should_fail: bool,
}

impl MockIntegration {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            enabled: true,
            start_count: AtomicU32::new(0),
            end_count: AtomicU32::new(0),
            error_count: AtomicU32::new(0),
            flush_count: AtomicU32::new(0),
            should_fail: false,
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
