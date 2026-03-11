//! Chat completion functionality for Meta Llama provider
//!
//! This module handles chat completions for Llama models through the OpenAI-compatible API.

pub mod transformation;

use serde_json::Value;
use std::collections::HashMap;
use tracing::debug;

// Use the transformation module
use crate::core::providers::meta_llama::LlamaConfig;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{chat::ChatRequest, responses::ChatResponse};
pub use transformation::LlamaChatTransformation;

/// Llama chat handler - simplified implementation using new type system
#[derive(Debug, Clone)]
pub struct LlamaChatHandler {
    transformation: LlamaChatTransformation,
}

impl LlamaChatHandler {
    /// Create a new chat handler
    pub fn new(_config: LlamaConfig) -> Result<Self, ProviderError> {
        Ok(Self {
            transformation: LlamaChatTransformation::new(),
        })
    }

    /// Transform a standard chat request to Llama format
    pub fn transform_request(&self, request: ChatRequest) -> Result<Value, ProviderError> {
        debug!("Transforming chat request for model: {}", request.model);
        self.transformation.transform_request(request)
    }

    /// Transform a Llama response to standard format
    pub fn transform_response(&self, response: Value) -> Result<ChatResponse, ProviderError> {
        debug!("Transforming Llama response");
        self.transformation.transform_response(response)
    }

    /// Get supported OpenAI parameters
    pub fn get_supported_openai_params(&self) -> Vec<String> {
        self.transformation.get_supported_params()
    }

    /// Validate request parameters
    pub fn validate_request(&self, request: &ChatRequest) -> Result<(), ProviderError> {
        // Validate model
        if request.model.is_empty() {
            return Err(ProviderError::invalid_request("meta", "Model is required"));
        }

        // Validate messages
        if request.messages.is_empty() {
            return Err(ProviderError::invalid_request(
                "meta",
                "Messages cannot be empty",
            ));
        }

        // Validate temperature
        if let Some(temp) = request.temperature
            && !(0.0..=2.0).contains(&temp)
        {
            return Err(ProviderError::invalid_request(
                "meta",
                format!("Temperature must be between 0 and 2, got {}", temp),
            ));
        }

        // Validate top_p
        if let Some(top_p) = request.top_p
            && !(0.0..=1.0).contains(&top_p)
        {
            return Err(ProviderError::invalid_request(
                "meta",
                format!("top_p must be between 0 and 1, got {}", top_p),
            ));
        }

        Ok(())
    }

    /// Map OpenAI parameters to Llama-specific ones
    pub fn map_openai_params(
        &self,
        params: HashMap<String, Value>,
        model: &str,
    ) -> HashMap<String, Value> {
        self.transformation.map_openai_params(params, model)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    #[test]
    fn test_handler_creation() {
        let config = LlamaConfig::default();
        let handler = LlamaChatHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_supported_params() {
        let config = LlamaConfig {
            api_key: "test".to_string(),
            ..Default::default()
        };
        let handler = LlamaChatHandler::new(config).unwrap();
        let params = handler.get_supported_openai_params();

        assert!(params.contains(&"messages".to_string()));
        assert!(params.contains(&"model".to_string()));
        assert!(params.contains(&"temperature".to_string()));
        assert!(params.contains(&"stream".to_string()));
    }

    #[test]
    fn test_request_validation() {
        let config = LlamaConfig {
            api_key: "test".to_string(),
            ..Default::default()
        };
        let handler = LlamaChatHandler::new(config).unwrap();

        // Test valid request
        let valid_request = ChatRequest {
            model: "llama3.1-8b".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            temperature: Some(0.8),
            top_p: Some(0.9),
            ..Default::default()
        };

        assert!(handler.validate_request(&valid_request).is_ok());

        // Test invalid temperature
        let mut invalid_request = valid_request.clone();
        invalid_request.temperature = Some(3.0);
        assert!(handler.validate_request(&invalid_request).is_err());

        // Test invalid top_p
        let mut invalid_request = valid_request.clone();
        invalid_request.top_p = Some(1.5);
        assert!(handler.validate_request(&invalid_request).is_err());

        // Test empty messages
        let mut invalid_request = valid_request.clone();
        invalid_request.messages.clear();
        assert!(handler.validate_request(&invalid_request).is_err());
    }
}
