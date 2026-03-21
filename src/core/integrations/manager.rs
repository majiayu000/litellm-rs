//! Integration Manager
//!
//! Manages registration and invocation of all integrations.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, warn};

use crate::core::traits::integration::{
    BoxedIntegration, CacheHitEvent, EmbeddingEndEvent, EmbeddingStartEvent, IntegrationError,
    IntegrationResult, LlmEndEvent, LlmErrorEvent, LlmStartEvent, LlmStreamEvent,
};

/// Configuration for the integration manager
#[derive(Debug, Clone)]
pub struct IntegrationManagerConfig {
    /// Whether to fail fast on integration errors (default: false)
    pub fail_fast: bool,
    /// Whether to run integrations in parallel (default: true)
    pub parallel: bool,
    /// Timeout for each integration call in milliseconds (default: 5000)
    pub timeout_ms: u64,
    /// Whether to log integration errors (default: true)
    pub log_errors: bool,
}

impl Default for IntegrationManagerConfig {
    fn default() -> Self {
        Self {
            fail_fast: false,
            parallel: true,
            timeout_ms: 5000,
            log_errors: true,
        }
    }
}

impl IntegrationManagerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    pub fn parallel(mut self, parallel: bool) -> Self {
        self.parallel = parallel;
        self
    }

    pub fn timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    pub fn log_errors(mut self, log_errors: bool) -> Self {
        self.log_errors = log_errors;
        self
    }
}

/// Integration Manager - manages all registered integrations
pub struct IntegrationManager {
    /// Registered integrations
    integrations: RwLock<Vec<BoxedIntegration>>,
    /// Configuration
    config: IntegrationManagerConfig,
}

impl IntegrationManager {
    /// Create a new integration manager
    pub fn new(config: IntegrationManagerConfig) -> Self {
        Self {
            integrations: RwLock::new(Vec::new()),
            config,
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(IntegrationManagerConfig::default())
    }

    /// Register an integration
    pub async fn register(&self, integration: BoxedIntegration) {
        if integration.is_enabled() {
            debug!("Registering integration: {}", integration.name());
            let mut integrations = self.integrations.write().await;
            integrations.push(integration);
        } else {
            debug!("Skipping disabled integration: {}", integration.name());
        }
    }

    /// Register multiple integrations
    pub async fn register_all(&self, integrations: Vec<BoxedIntegration>) {
        for integration in integrations {
            self.register(integration).await;
        }
    }

    /// Unregister an integration by name
    pub async fn unregister(&self, name: &str) -> bool {
        let mut integrations = self.integrations.write().await;
        let initial_len = integrations.len();
        integrations.retain(|i| i.name() != name);
        let removed = integrations.len() < initial_len;
        if removed {
            debug!("Unregistered integration: {}", name);
        }
        removed
    }

    /// Get list of registered integration names
    pub async fn list_integrations(&self) -> Vec<&'static str> {
        let integrations = self.integrations.read().await;
        integrations.iter().map(|i| i.name()).collect()
    }

    /// Get count of registered integrations
    pub async fn count(&self) -> usize {
        self.integrations.read().await.len()
    }

    /// Check if an integration is registered
    pub async fn has_integration(&self, name: &str) -> bool {
        let integrations = self.integrations.read().await;
        integrations.iter().any(|i| i.name() == name)
    }

    /// Notify all integrations of LLM start
    pub async fn on_llm_start(&self, event: &LlmStartEvent) -> IntegrationResult<()> {
        let integrations = self.integrations.read().await;
        if integrations.is_empty() {
            return Ok(());
        }

        let event = event.clone();
        if self.config.parallel {
            self.dispatch_parallel_start(&integrations, event).await
        } else {
            self.dispatch_sequential_start(&integrations, event).await
        }
    }

    /// Notify all integrations of LLM end
    pub async fn on_llm_end(&self, event: &LlmEndEvent) -> IntegrationResult<()> {
        let integrations = self.integrations.read().await;
        if integrations.is_empty() {
            return Ok(());
        }

        let event = event.clone();
        if self.config.parallel {
            self.dispatch_parallel_end(&integrations, event).await
        } else {
            self.dispatch_sequential_end(&integrations, event).await
        }
    }

    /// Notify all integrations of LLM error
    pub async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()> {
        let integrations = self.integrations.read().await;
        if integrations.is_empty() {
            return Ok(());
        }

        let event = event.clone();
        if self.config.parallel {
            self.dispatch_parallel_error(&integrations, event).await
        } else {
            self.dispatch_sequential_error(&integrations, event).await
        }
    }

    /// Notify all integrations of LLM stream chunk
    pub async fn on_llm_stream(&self, event: &LlmStreamEvent) -> IntegrationResult<()> {
        let integrations = self.integrations.read().await;
        for integration in integrations.iter() {
            if integration.is_enabled()
                && let Err(e) = integration.on_llm_stream(event).await
            {
                if self.config.log_errors {
                    warn!("Integration {} stream error: {}", integration.name(), e);
                }
                if self.config.fail_fast {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Notify all integrations of embedding start
    pub async fn on_embedding_start(&self, event: &EmbeddingStartEvent) -> IntegrationResult<()> {
        let integrations = self.integrations.read().await;
        for integration in integrations.iter() {
            if integration.is_enabled()
                && let Err(e) = integration.on_embedding_start(event).await
            {
                if self.config.log_errors {
                    warn!(
                        "Integration {} embedding start error: {}",
                        integration.name(),
                        e
                    );
                }
                if self.config.fail_fast {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Notify all integrations of embedding end
    pub async fn on_embedding_end(&self, event: &EmbeddingEndEvent) -> IntegrationResult<()> {
        let integrations = self.integrations.read().await;
        for integration in integrations.iter() {
            if integration.is_enabled()
                && let Err(e) = integration.on_embedding_end(event).await
            {
                if self.config.log_errors {
                    warn!(
                        "Integration {} embedding end error: {}",
                        integration.name(),
                        e
                    );
                }
                if self.config.fail_fast {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Notify all integrations of cache hit
    pub async fn on_cache_hit(&self, event: &CacheHitEvent) -> IntegrationResult<()> {
        let integrations = self.integrations.read().await;
        for integration in integrations.iter() {
            if integration.is_enabled()
                && let Err(e) = integration.on_cache_hit(event).await
            {
                if self.config.log_errors {
                    warn!("Integration {} cache hit error: {}", integration.name(), e);
                }
                if self.config.fail_fast {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Flush all integrations
    pub async fn flush(&self) -> IntegrationResult<()> {
        let integrations = self.integrations.read().await;
        for integration in integrations.iter() {
            if integration.is_enabled()
                && let Err(e) = integration.flush().await
            {
                if self.config.log_errors {
                    warn!("Integration {} flush error: {}", integration.name(), e);
                }
                if self.config.fail_fast {
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Shutdown all integrations
    pub async fn shutdown(&self) -> IntegrationResult<()> {
        debug!("Shutting down all integrations");
        let integrations = self.integrations.read().await;

        let mut errors = Vec::new();
        for integration in integrations.iter() {
            if let Err(e) = integration.shutdown().await {
                if self.config.log_errors {
                    error!(
                        "Error shutting down integration {}: {}",
                        integration.name(),
                        e
                    );
                }
                errors.push((integration.name(), e));
            }
        }

        if self.config.fail_fast
            && let Some((name, err)) = errors.into_iter().next()
        {
            return Err(IntegrationError::Other(format!(
                "Integration {} shutdown failed: {}",
                name, err
            )));
        }

        Ok(())
    }

    /// Dispatch LLM start events in parallel
    async fn dispatch_parallel_start(
        &self,
        integrations: &[BoxedIntegration],
        event: LlmStartEvent,
    ) -> IntegrationResult<()> {
        let timeout = std::time::Duration::from_millis(self.config.timeout_ms);
        let mut handles = Vec::with_capacity(integrations.len());

        for integration in integrations.iter() {
            if !integration.is_enabled() {
                continue;
            }

            let integration = Arc::clone(integration);
            let event = event.clone();
            let name = integration.name();

            handles.push(tokio::spawn(async move {
                match tokio::time::timeout(timeout, integration.on_llm_start(&event)).await {
                    Ok(result) => (name, result),
                    Err(_) => (
                        name,
                        Err(IntegrationError::Timeout {
                            timeout_ms: timeout.as_millis() as u64,
                        }),
                    ),
                }
            }));
        }

        self.collect_results(handles).await
    }

    /// Dispatch LLM start events sequentially
    async fn dispatch_sequential_start(
        &self,
        integrations: &[BoxedIntegration],
        event: LlmStartEvent,
    ) -> IntegrationResult<()> {
        let timeout = std::time::Duration::from_millis(self.config.timeout_ms);

        for integration in integrations.iter() {
            if !integration.is_enabled() {
                continue;
            }

            let result = tokio::time::timeout(timeout, integration.on_llm_start(&event)).await;
            self.handle_sequential_result(integration.name(), result)?;
        }

        Ok(())
    }

    /// Dispatch LLM end events in parallel
    async fn dispatch_parallel_end(
        &self,
        integrations: &[BoxedIntegration],
        event: LlmEndEvent,
    ) -> IntegrationResult<()> {
        let timeout = std::time::Duration::from_millis(self.config.timeout_ms);
        let mut handles = Vec::with_capacity(integrations.len());

        for integration in integrations.iter() {
            if !integration.is_enabled() {
                continue;
            }

            let integration = Arc::clone(integration);
            let event = event.clone();
            let name = integration.name();

            handles.push(tokio::spawn(async move {
                match tokio::time::timeout(timeout, integration.on_llm_end(&event)).await {
                    Ok(result) => (name, result),
                    Err(_) => (
                        name,
                        Err(IntegrationError::Timeout {
                            timeout_ms: timeout.as_millis() as u64,
                        }),
                    ),
                }
            }));
        }

        self.collect_results(handles).await
    }

    /// Dispatch LLM end events sequentially
    async fn dispatch_sequential_end(
        &self,
        integrations: &[BoxedIntegration],
        event: LlmEndEvent,
    ) -> IntegrationResult<()> {
        let timeout = std::time::Duration::from_millis(self.config.timeout_ms);

        for integration in integrations.iter() {
            if !integration.is_enabled() {
                continue;
            }

            let result = tokio::time::timeout(timeout, integration.on_llm_end(&event)).await;
            self.handle_sequential_result(integration.name(), result)?;
        }

        Ok(())
    }

    /// Dispatch LLM error events in parallel
    async fn dispatch_parallel_error(
        &self,
        integrations: &[BoxedIntegration],
        event: LlmErrorEvent,
    ) -> IntegrationResult<()> {
        let timeout = std::time::Duration::from_millis(self.config.timeout_ms);
        let mut handles = Vec::with_capacity(integrations.len());

        for integration in integrations.iter() {
            if !integration.is_enabled() {
                continue;
            }

            let integration = Arc::clone(integration);
            let event = event.clone();
            let name = integration.name();

            handles.push(tokio::spawn(async move {
                match tokio::time::timeout(timeout, integration.on_llm_error(&event)).await {
                    Ok(result) => (name, result),
                    Err(_) => (
                        name,
                        Err(IntegrationError::Timeout {
                            timeout_ms: timeout.as_millis() as u64,
                        }),
                    ),
                }
            }));
        }

        self.collect_results(handles).await
    }

    /// Dispatch LLM error events sequentially
    async fn dispatch_sequential_error(
        &self,
        integrations: &[BoxedIntegration],
        event: LlmErrorEvent,
    ) -> IntegrationResult<()> {
        let timeout = std::time::Duration::from_millis(self.config.timeout_ms);

        for integration in integrations.iter() {
            if !integration.is_enabled() {
                continue;
            }

            let result = tokio::time::timeout(timeout, integration.on_llm_error(&event)).await;
            self.handle_sequential_result(integration.name(), result)?;
        }

        Ok(())
    }

    /// Collect results from parallel dispatch
    async fn collect_results(
        &self,
        handles: Vec<tokio::task::JoinHandle<(&'static str, IntegrationResult<()>)>>,
    ) -> IntegrationResult<()> {
        let mut errors = Vec::new();

        for handle in handles {
            match handle.await {
                Ok((name, Err(e))) => {
                    if self.config.log_errors {
                        warn!("Integration {} error: {}", name, e);
                    }
                    errors.push((name, e));
                }
                Err(e) => {
                    if self.config.log_errors {
                        error!("Integration task panicked: {}", e);
                    }
                }
                Ok((_, Ok(()))) => {}
            }
        }

        if self.config.fail_fast
            && let Some((name, err)) = errors.into_iter().next()
        {
            return Err(IntegrationError::Other(format!(
                "Integration {} failed: {}",
                name, err
            )));
        }

        Ok(())
    }

    /// Handle result from sequential dispatch
    fn handle_sequential_result(
        &self,
        name: &'static str,
        result: Result<IntegrationResult<()>, tokio::time::error::Elapsed>,
    ) -> IntegrationResult<()> {
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => {
                if self.config.log_errors {
                    warn!("Integration {} error: {}", name, e);
                }
                if self.config.fail_fast {
                    Err(e)
                } else {
                    Ok(())
                }
            }
            Err(_) => {
                let err = IntegrationError::Timeout {
                    timeout_ms: self.config.timeout_ms,
                };
                if self.config.log_errors {
                    warn!("Integration {} timeout", name);
                }
                if self.config.fail_fast {
                    Err(err)
                } else {
                    Ok(())
                }
            }
        }
    }
}

impl Default for IntegrationManager {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
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
}
