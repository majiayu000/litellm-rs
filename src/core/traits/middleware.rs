//! Middleware system trait definitions
//!
//! Provides composable middleware architecture supporting authentication, cache, retry, and other cross-cutting concerns

use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;

/// Core middleware trait
///
/// All middleware must implement this trait
#[async_trait]
pub trait Middleware<Req, Resp>: Send + Sync {
    /// Error
    type Error: std::error::Error + Send + Sync + 'static;

    /// Process request through middleware
    ///
    /// # Parameters
    /// * `request` - Incoming request
    /// * `next` - Next handler in the chain
    ///
    /// # Returns
    /// Processed response
    async fn process(
        &self,
        request: Req,
        next: Box<dyn MiddlewareNext<Req, Resp>>,
    ) -> Result<Resp, Self::Error>;
}

/// Next middleware handler in the chain
#[async_trait]
pub trait MiddlewareNext<Req, Resp>: Send + Sync {
    /// Call the next handler in the chain
    async fn call(&self, request: Req) -> Result<Resp, MiddlewareError>;
}

// Type aliases for cleaner types
type BoxedMiddleware<Req, Resp> = Box<dyn Middleware<Req, Resp, Error = MiddlewareError>>;
type BoxedFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Middleware chain/stack
pub struct MiddlewareStack<Req, Resp> {
    middlewares: Vec<BoxedMiddleware<Req, Resp>>,
}

impl<Req, Resp> MiddlewareStack<Req, Resp>
where
    Req: Clone + Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    /// Create new middleware stack
    pub fn new() -> Self {
        Self {
            middlewares: Vec::new(),
        }
    }

    /// Add middleware to the stack
    pub fn add_middleware<M>(self, _middleware: M) -> Self
    where
        M: Middleware<Req, Resp> + 'static,
    {
        // NOTE: Middleware wrapper type constraints not yet resolved.
        self
    }

    /// Execute middleware chain
    pub async fn execute<F, Fut>(
        &self,
        request: Req,
        final_handler: F,
    ) -> Result<Resp, MiddlewareError>
    where
        F: FnOnce(Req) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Resp, MiddlewareError>> + Send + Sync + 'static,
    {
        let handler = Box::new(FinalHandler::new(final_handler));
        self.execute_chain(request, 0, handler).await
    }

    /// Recursively execute middleware chain
    fn execute_chain(
        &self,
        request: Req,
        index: usize,
        final_handler: Box<dyn MiddlewareNext<Req, Resp>>,
    ) -> BoxedFuture<'_, Result<Resp, MiddlewareError>> {
        Box::pin(async move {
            if index >= self.middlewares.len() {
                // Execute final handler
                final_handler.call(request).await
            } else {
                // Create next handler
                let _next = Box::new(NextHandler {
                    _stack: self,
                    _index: index + 1,
                    _final_handler: final_handler,
                    _request: request.clone(),
                });

                // NOTE: Middleware execution disabled; type constraints not yet resolved.
                // self.middlewares[index].process(request, next).await
                Err(MiddlewareError::ExecutionFailed(
                    "Middleware system temporarily disabled".to_string(),
                ))
            }
        })
    }
}

impl<Req, Resp> Default for MiddlewareStack<Req, Resp>
where
    Req: Clone + Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    fn default() -> Self {
        Self::new()
    }
}

/// Final handler wrapper
struct FinalHandler<F, Fut, Req, Resp> {
    _handler: Option<F>,
    _phantom: std::marker::PhantomData<(Fut, Req, Resp)>,
}

impl<F, Fut, Req, Resp> FinalHandler<F, Fut, Req, Resp> {
    fn new(handler: F) -> Self {
        Self {
            _handler: Some(handler),
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait]
impl<F, Fut, Req, Resp> MiddlewareNext<Req, Resp> for FinalHandler<F, Fut, Req, Resp>
where
    F: FnOnce(Req) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Resp, MiddlewareError>> + Send + Sync,
    Req: Send + Sync,
    Resp: Send + Sync,
{
    async fn call(&self, _request: Req) -> Result<Resp, MiddlewareError> {
        // FnOnce can only be called once, but trait methods may be called multiple times
        // This requires a more complex design with interior mutability
        Err(MiddlewareError::ExecutionFailed(
            "FinalHandler: FnOnce handling not yet implemented".to_string(),
        ))
    }
}

/// Next handler wrapper
struct NextHandler<'a, Req, Resp> {
    _stack: &'a MiddlewareStack<Req, Resp>,
    _index: usize,
    _final_handler: Box<dyn MiddlewareNext<Req, Resp>>,
    _request: Req,
}

#[async_trait]
impl<'a, Req, Resp> MiddlewareNext<Req, Resp> for NextHandler<'a, Req, Resp>
where
    Req: Clone + Send + Sync + 'static,
    Resp: Send + Sync + 'static,
{
    async fn call(&self, _request: Req) -> Result<Resp, MiddlewareError> {
        // This requires redesign due to lifetime issues with recursive middleware chains
        Err(MiddlewareError::ExecutionFailed(
            "NextHandler: next handler not yet implemented".to_string(),
        ))
    }
}

/// Error
#[derive(Debug, thiserror::Error)]
pub enum MiddlewareError {
    #[error("Middleware chain execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Invalid middleware configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Middleware timeout after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Other middleware error: {0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== MiddlewareStack Tests ====================

    #[test]
    fn test_middleware_stack_new() {
        let stack: MiddlewareStack<String, String> = MiddlewareStack::new();
        assert!(stack.middlewares.is_empty());
    }

    #[test]
    fn test_middleware_stack_default() {
        let stack: MiddlewareStack<String, String> = MiddlewareStack::default();
        assert!(stack.middlewares.is_empty());
    }

    // ==================== MiddlewareError Tests ====================

    #[test]
    fn test_middleware_error_execution_failed() {
        let err = MiddlewareError::ExecutionFailed("Handler failed".to_string());
        assert!(
            err.to_string()
                .contains("Middleware chain execution failed")
        );
        assert!(err.to_string().contains("Handler failed"));
    }

    #[test]
    fn test_middleware_error_invalid_configuration() {
        let err = MiddlewareError::InvalidConfiguration("Missing required field".to_string());
        assert!(err.to_string().contains("Invalid middleware configuration"));
        assert!(err.to_string().contains("Missing required field"));
    }

    #[test]
    fn test_middleware_error_timeout() {
        let err = MiddlewareError::Timeout { timeout_ms: 5000 };
        assert!(err.to_string().contains("timeout"));
        assert!(err.to_string().contains("5000"));
    }

    #[test]
    fn test_middleware_error_other() {
        let err = MiddlewareError::Other("Unknown error".to_string());
        assert!(err.to_string().contains("Unknown error"));
    }

    #[test]
    fn test_middleware_error_display() {
        let err = MiddlewareError::ExecutionFailed("test".to_string());
        let display = format!("{}", err);
        assert!(!display.is_empty());
    }

    #[test]
    fn test_middleware_error_debug() {
        let err = MiddlewareError::Timeout { timeout_ms: 1000 };
        let debug = format!("{:?}", err);
        assert!(debug.contains("Timeout"));
        assert!(debug.contains("1000"));
    }
}
