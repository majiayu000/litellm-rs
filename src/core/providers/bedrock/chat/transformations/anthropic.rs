//! Anthropic Claude Model Transformations

use crate::core::providers::bedrock::model_config::ModelConfig;
use crate::core::providers::bedrock::parameter_policy::serialize_bedrock_chat_parameters;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use serde_json::{Value, json};

/// Transform request for Anthropic Claude models
pub fn transform_request(
    request: &ChatRequest,
    _model_config: &ModelConfig,
) -> Result<Value, ProviderError> {
    let parameter_fields = serialize_bedrock_chat_parameters(request)?;

    // Claude models on Bedrock use anthropic messages format
    let mut body = json!({
        "messages": request.messages,
        "max_tokens": parameter_fields.max_tokens.unwrap_or(4096),
        "anthropic_version": "bedrock-2023-05-31"
    });

    if let Some(temp) = parameter_fields.temperature {
        body["temperature"] = json!(temp);
    }

    if let Some(top_p) = parameter_fields.top_p {
        body["top_p"] = json!(top_p);
    }

    if let Some(stop) = parameter_fields.stop_sequences {
        body["stop_sequences"] = json!(stop);
    }

    if let Some(top_k) = parameter_fields.top_k {
        body["top_k"] = top_k;
    }

    if let Some(thinking) = parameter_fields.thinking {
        body["thinking"] = thinking;
    }

    if let Some(system) = extract_system_message(request) {
        body["system"] = json!(system);
    }

    Ok(body)
}

/// Extract system message from chat messages
fn extract_system_message(request: &ChatRequest) -> Option<String> {
    use crate::core::types::{message::MessageContent, message::MessageRole};

    request
        .messages
        .iter()
        .find(|msg| msg.role == MessageRole::System)
        .and_then(|msg| msg.content.as_ref())
        .map(|content| match content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Parts(parts) => parts
                .iter()
                .filter_map(|part| {
                    if let crate::core::types::content::ContentPart::Text { text } = part {
                        Some(text.clone())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::bedrock::model_config::{BedrockApiType, BedrockModelFamily};
    use crate::core::types::{
        chat::ChatMessage,
        message::MessageContent,
        message::MessageRole,
        thinking::{ThinkingConfig, ThinkingEffort},
    };

    fn create_test_request() -> ChatRequest {
        ChatRequest {
            model: "anthropic.claude-3-sonnet".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                thinking: None,
                audio: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            ..Default::default()
        }
    }

    fn create_test_model_config() -> ModelConfig {
        ModelConfig {
            family: BedrockModelFamily::Claude,
            api_type: BedrockApiType::Converse,
            supports_streaming: true,
            supports_function_calling: true,
            supports_multimodal: true,
            max_context_length: 200000,
            max_output_length: Some(4096),
            input_cost_per_1k: 0.003,
            output_cost_per_1k: 0.015,
        }
    }

    #[test]
    fn test_transform_request_basic() {
        let request = create_test_request();
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["messages"].is_array());
        assert_eq!(value["max_tokens"], 4096);
        assert_eq!(value["anthropic_version"], "bedrock-2023-05-31");
    }

    #[test]
    fn test_transform_request_with_temperature() {
        let mut request = create_test_request();
        request.temperature = Some(0.5);
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["temperature"], 0.5);
    }

    #[test]
    fn test_transform_request_with_top_p() {
        let mut request = create_test_request();
        request.top_p = Some(0.5);
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["top_p"], 0.5);
    }

    #[test]
    fn bedrock_parameter_policy_serializes_top_k() {
        let mut request = create_test_request();
        request
            .extra_params
            .insert("top_k".to_string(), serde_json::json!(64));
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["top_k"], 64);
    }

    #[test]
    fn bedrock_parameter_policy_rejects_opus_47_top_p() {
        let mut request = create_test_request();
        request.model = "anthropic.claude-opus-4-7".to_string();
        request.top_p = Some(0.5);
        let model_config = create_test_model_config();

        let error = transform_request(&request, &model_config)
            .expect_err("Claude Opus 4.7 should reject non-default top_p locally");
        let message = error.to_string();
        assert!(message.contains("claude-opus-4-7"));
        assert!(message.contains("top_p"));
    }

    #[test]
    fn bedrock_parameter_policy_serializes_opus_47_adaptive_thinking() {
        let mut request = create_test_request();
        request.model = "anthropic.claude-opus-4-7".to_string();
        request.thinking = Some(
            ThinkingConfig::new()
                .enabled()
                .with_effort(ThinkingEffort::High),
        );
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(
            value["thinking"],
            serde_json::json!({
                "type": "adaptive",
                "effort": "high"
            })
        );
    }

    #[test]
    fn test_transform_request_with_stop() {
        let mut request = create_test_request();
        request.stop = Some(vec!["STOP".to_string()]);
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["stop_sequences"].is_array());
    }

    #[test]
    fn test_transform_request_with_max_tokens() {
        let mut request = create_test_request();
        request.max_tokens = Some(100);
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["max_tokens"], 100);
    }

    #[test]
    fn test_transform_request_with_system() {
        let request = ChatRequest {
            model: "anthropic.claude-3-sonnet".to_string(),
            messages: vec![
                ChatMessage {
                    role: MessageRole::System,
                    content: Some(MessageContent::Text("You are helpful".to_string())),
                    thinking: None,
                    audio: None,
                    name: None,
                    function_call: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                ChatMessage {
                    role: MessageRole::User,
                    content: Some(MessageContent::Text("Hello".to_string())),
                    thinking: None,
                    audio: None,
                    name: None,
                    function_call: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
            ],
            ..Default::default()
        };
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["system"], "You are helpful");
    }

    #[test]
    fn test_extract_system_message_text() {
        let request = ChatRequest {
            model: "anthropic.claude-3-sonnet".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::System,
                content: Some(MessageContent::Text("System prompt".to_string())),
                thinking: None,
                audio: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            ..Default::default()
        };

        let system = extract_system_message(&request);
        assert!(system.is_some());
        assert_eq!(system.unwrap(), "System prompt");
    }

    #[test]
    fn test_extract_system_message_none() {
        let request = create_test_request();

        let system = extract_system_message(&request);
        assert!(system.is_none());
    }

    #[test]
    fn test_extract_system_message_user_only() {
        let request = ChatRequest {
            model: "anthropic.claude-3-sonnet".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                thinking: None,
                audio: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            ..Default::default()
        };

        let system = extract_system_message(&request);
        assert!(system.is_none());
    }
}
