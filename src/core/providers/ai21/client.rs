//! AI21 Client
//!
//! Request transformation, response processing, and model definitions for AI21 Labs

use serde_json::{Value, json};

use super::models::get_ai21_registry;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{chat::ChatRequest, model::ModelInfo, responses::ChatResponse};

/// AI21 API client logic
pub struct AI21Client;

impl AI21Client {
    /// Transform chat request to AI21 format (OpenAI compatible)
    pub fn transform_chat_request(request: ChatRequest) -> Value {
        json!({
            "model": request.model,
            "messages": request.messages,
            "temperature": request.temperature,
            "max_tokens": request.max_tokens,
            "top_p": request.top_p,
            "stream": request.stream,
            "tools": request.tools,
            "tool_choice": request.tool_choice,
            "n": request.n,
            "stop": request.stop,
            "seed": request.seed,
            "response_format": request.response_format,
        })
    }

    /// Transform AI21 response to standard format
    pub fn transform_chat_response(response: Value) -> Result<ChatResponse, ProviderError> {
        // Check for error in response
        if let Some(error) = response.get("error") {
            let error_message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error from AI21 API");

            let error_code = error
                .get("code")
                .and_then(|c| c.as_str())
                .unwrap_or("unknown_error");

            return Err(match error_code {
                "authentication_error" | "invalid_request_error" => {
                    ProviderError::authentication("ai21", error_message)
                }
                "rate_limit_exceeded" => ProviderError::rate_limit("ai21", None),
                _ => ProviderError::api_error("ai21", 400, error_message),
            });
        }

        // First try direct deserialization
        if let Ok(chat_response) = serde_json::from_value::<ChatResponse>(response.clone()) {
            return Ok(chat_response);
        }

        // Build response with defaults for missing fields
        let mut response_obj = response
            .as_object()
            .ok_or_else(|| {
                ProviderError::response_parsing("ai21", "Response is not an object".to_string())
            })?
            .clone();

        // If no id field, generate one
        if !response_obj.contains_key("id") {
            use std::time::{SystemTime, UNIX_EPOCH};
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            response_obj.insert(
                "id".to_string(),
                Value::String(format!("chatcmpl-ai21-{}", timestamp)),
            );
        }

        // Default object type
        if !response_obj.contains_key("object") {
            response_obj.insert(
                "object".to_string(),
                Value::String("chat.completion".to_string()),
            );
        }

        // If no created field, add current timestamp
        if !response_obj.contains_key("created") {
            use std::time::{SystemTime, UNIX_EPOCH};
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            response_obj.insert(
                "created".to_string(),
                Value::Number(serde_json::Number::from(timestamp)),
            );
        }

        // Default model
        if !response_obj.contains_key("model") {
            response_obj.insert(
                "model".to_string(),
                Value::String("jamba-1.5-large".to_string()),
            );
        }

        // Try deserialization again
        serde_json::from_value(Value::Object(response_obj))
            .map_err(|e| ProviderError::response_parsing("ai21", e.to_string()))
    }

    /// Get supported models
    pub fn supported_models() -> Vec<ModelInfo> {
        get_ai21_registry().get_all_models()
    }

    /// Get supported OpenAI-compatible parameters
    pub fn supported_openai_params() -> &'static [&'static str] {
        &[
            "temperature",
            "max_tokens",
            "max_completion_tokens",
            "top_p",
            "stream",
            "tools",
            "tool_choice",
            "n",
            "stop",
            "seed",
            "response_format",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};
    use std::collections::HashMap;

    #[test]
    fn test_transform_request() {
        let request = ChatRequest {
            model: "jamba-1.5-large".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                thinking: None,
                name: None,
                tool_calls: None,
                tool_call_id: None,
                function_call: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stream: false,
            stream_options: None,
            tools: None,
            tool_choice: None,
            user: None,
            response_format: None,
            seed: None,
            max_completion_tokens: None,
            stop: None,
            parallel_tool_calls: None,
            n: None,
            logit_bias: None,
            functions: None,
            function_call: None,
            logprobs: None,
            top_logprobs: None,
            reasoning_effort: None,
            store: None,
            metadata: None,
            service_tier: None,
            thinking: None,
            extra_params: HashMap::new(),
        };

        let transformed = AI21Client::transform_chat_request(request);
        assert_eq!(transformed["model"], "jamba-1.5-large");
        let temp = transformed["temperature"].as_f64().unwrap();
        assert!((temp - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_supported_models() {
        let models = AI21Client::supported_models();
        assert!(models.len() >= 2);

        let model_ids: Vec<String> = models.iter().map(|m| m.id.clone()).collect();
        assert!(model_ids.contains(&"jamba-1.5-large".to_string()));
        assert!(model_ids.contains(&"jamba-1.5-mini".to_string()));
    }

    #[test]
    fn test_supported_openai_params() {
        let params = AI21Client::supported_openai_params();
        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"stream"));
    }

    #[test]
    fn test_transform_response_with_error() {
        let error_response = json!({
            "error": {
                "message": "Invalid API key",
                "code": "authentication_error"
            }
        });

        let result = AI21Client::transform_chat_response(error_response);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_response_rate_limit() {
        let error_response = json!({
            "error": {
                "message": "Rate limit exceeded",
                "code": "rate_limit_exceeded"
            }
        });

        let result = AI21Client::transform_chat_response(error_response);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ProviderError::RateLimit { .. }
        ));
    }

    #[test]
    fn test_transform_response_not_object() {
        let invalid_response = json!("not an object");

        let result = AI21Client::transform_chat_response(invalid_response);
        assert!(result.is_err());
    }
}
