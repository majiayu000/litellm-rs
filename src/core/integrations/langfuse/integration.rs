//! Adapter from the canonical integration lifecycle to Langfuse logging.

use async_trait::async_trait;

use super::{LangfuseConfig, LangfuseError, LangfuseLogger, LlmError, LlmRequest, LlmResponse};
use crate::core::traits::integration::{
    Integration, IntegrationError, IntegrationResult, LlmEndEvent, LlmErrorEvent, LlmStartEvent,
};

/// Canonical callback adapter for the existing Langfuse logger.
pub struct LangfuseIntegration {
    logger: LangfuseLogger,
}

impl LangfuseIntegration {
    /// Build a Langfuse callback integration.
    pub fn new(config: LangfuseConfig) -> Result<Self, LangfuseError> {
        LangfuseLogger::new(config).map(|logger| Self { logger })
    }
}

#[async_trait]
impl Integration for LangfuseIntegration {
    fn name(&self) -> &'static str {
        "langfuse"
    }

    fn is_enabled(&self) -> bool {
        true
    }

    async fn on_llm_start(&self, event: &LlmStartEvent) -> IntegrationResult<()> {
        let mut request = LlmRequest::new(&event.request_id, &event.model);
        request.provider = event.provider.clone();
        request.user_id = event.user_id.clone();
        request.session_id = event.session_id.clone();
        request.metadata = event.metadata.clone();
        request.tags = event.tags.clone();
        request.parameters = event.parameters.clone();
        self.logger.on_llm_start(request);
        Ok(())
    }

    async fn on_llm_end(&self, event: &LlmEndEvent) -> IntegrationResult<()> {
        let mut response = LlmResponse::new(&event.request_id);
        response.input_tokens = event.input_tokens;
        response.output_tokens = event.output_tokens;
        response.cost = event.cost_usd;
        response.metadata = event.metadata.clone();
        if let Some(provider) = &event.provider {
            response
                .metadata
                .insert("provider".to_string(), serde_json::json!(provider));
        }
        response.metadata.insert(
            "latency_ms".to_string(),
            serde_json::json!(event.latency_ms),
        );
        self.logger.on_llm_end(response);
        Ok(())
    }

    async fn on_llm_error(&self, event: &LlmErrorEvent) -> IntegrationResult<()> {
        let mut error = LlmError::new(&event.request_id, &event.error_message);
        error.error_type = event.error_type.clone();
        error.metadata = event.metadata.clone();
        if let Some(provider) = &event.provider {
            error
                .metadata
                .insert("provider".to_string(), serde_json::json!(provider));
        }
        self.logger.on_llm_error(error);
        Ok(())
    }

    async fn flush(&self) -> IntegrationResult<()> {
        self.logger
            .flush()
            .await
            .map_err(|error| IntegrationError::other(format!("Langfuse flush failed: {error}")))
    }

    async fn shutdown(&self) -> IntegrationResult<()> {
        self.flush().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn debug_config() -> LangfuseConfig {
        LangfuseConfig {
            public_key: Some("pk-test".to_string()),
            secret_key: Some("sk-test".to_string()),
            debug: true,
            flush_interval_ms: 60_000,
            ..LangfuseConfig::default()
        }
    }

    #[tokio::test]
    async fn adapter_tracks_canonical_start_and_end_without_content() {
        let integration = match LangfuseIntegration::new(debug_config()) {
            Ok(integration) => integration,
            Err(error) => panic!("Langfuse adapter should initialize: {error}"),
        };
        let start = LlmStartEvent::new("req-1", "model").provider("provider");
        assert!(integration.on_llm_start(&start).await.is_ok());
        assert_eq!(integration.logger.active_count(), 1);

        let end = LlmEndEvent::new("req-1", "model")
            .provider("provider")
            .tokens(10, 5)
            .cost(0.25)
            .latency(20);
        assert!(integration.on_llm_end(&end).await.is_ok());
        assert_eq!(integration.logger.active_count(), 0);
    }
}
