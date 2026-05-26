//! Model-specific Request Transformations
//!
//! Handles transformation of OpenAI-style requests to provider-specific formats

pub mod ai21;
pub mod amazon;
pub mod anthropic;
pub mod cohere;
pub mod meta;
pub mod mistral;

use crate::core::providers::bedrock::model_config::{BedrockModelFamily, ModelConfig};
use crate::core::providers::bedrock::model_id::is_runtime_resolved_invoke_model_id;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use serde_json::{Value, json};

/// Transform request based on model family
pub fn transform_for_model(
    request: &ChatRequest,
    model_config: &ModelConfig,
) -> Result<Value, ProviderError> {
    if is_runtime_resolved_invoke_model_id(&request.model) {
        return transform_runtime_invoke_request(request);
    }

    match model_config.family {
        BedrockModelFamily::Claude => anthropic::transform_request(request, model_config),
        BedrockModelFamily::TitanText => amazon::transform_titan_request(request, model_config),
        BedrockModelFamily::Nova => amazon::transform_nova_request(request, model_config),
        BedrockModelFamily::Llama => meta::transform_request(request, model_config),
        BedrockModelFamily::Mistral => mistral::transform_request(request, model_config),
        BedrockModelFamily::Cohere => cohere::transform_request(request, model_config),
        BedrockModelFamily::AI21 => ai21::transform_request(request, model_config),
        BedrockModelFamily::DeepSeek => {
            // DeepSeek uses similar format to Mistral
            mistral::transform_request(request, model_config)
        }
        _ => Err(ProviderError::not_supported(
            "bedrock",
            format!(
                "Model family {:?} not supported for chat",
                model_config.family
            ),
        )),
    }
}

pub(in crate::core::providers::bedrock) fn transform_runtime_invoke_request(
    request: &ChatRequest,
) -> Result<Value, ProviderError> {
    if request
        .extra_params
        .get("bedrock_invoke_schema")
        .and_then(Value::as_str)
        == Some("openai_chat")
    {
        return transform_openai_compatible_request(request);
    }

    transform_generic_invoke_request(request)
}

fn transform_openai_compatible_request(request: &ChatRequest) -> Result<Value, ProviderError> {
    let messages = serde_json::to_value(&request.messages)
        .map_err(|e| ProviderError::serialization("bedrock", e.to_string()))?;
    let mut body = json!({ "messages": messages });

    add_sampling_invoke_params(request, &mut body);

    if let Some(tools) = &request.tools {
        body["tools"] = serde_json::to_value(tools)
            .map_err(|e| ProviderError::serialization("bedrock", e.to_string()))?;
    }
    if let Some(tool_choice) = &request.tool_choice {
        body["tool_choice"] = serde_json::to_value(tool_choice)
            .map_err(|e| ProviderError::serialization("bedrock", e.to_string()))?;
    }

    Ok(body)
}

fn transform_generic_invoke_request(request: &ChatRequest) -> Result<Value, ProviderError> {
    let mut body = json!({
        "prompt": messages_to_prompt(&request.messages),
    });

    add_sampling_invoke_params(request, &mut body);
    if request
        .max_completion_tokens
        .or(request.max_tokens)
        .is_none()
    {
        body["max_tokens"] = json!(4096);
    }

    Ok(body)
}

fn add_sampling_invoke_params(request: &ChatRequest, body: &mut Value) {
    if let Some(max_tokens) = request.max_completion_tokens.or(request.max_tokens) {
        body["max_tokens"] = json!(max_tokens);
    }
    if let Some(temperature) = request.temperature {
        body["temperature"] = json!(temperature);
    }
    if let Some(top_p) = request.top_p {
        body["top_p"] = json!(top_p);
    }
    if let Some(stop) = &request.stop {
        body["stop"] = json!(stop);
    }
}

/// Common utility to convert messages to prompt format
pub fn messages_to_prompt(messages: &[crate::core::types::chat::ChatMessage]) -> String {
    use crate::core::types::{message::MessageContent, message::MessageRole};

    let mut prompt = String::new();

    for message in messages {
        let content = match &message.content {
            Some(MessageContent::Text(text)) => text.clone(),
            Some(MessageContent::Parts(parts)) => {
                // Extract text from parts
                parts
                    .iter()
                    .filter_map(|part| {
                        if let crate::core::types::content::ContentPart::Text { text } = part {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            None => continue,
        };

        match message.role {
            MessageRole::System | MessageRole::Developer => {
                prompt.push_str(&format!("System: {}\n\n", content))
            }
            MessageRole::User => prompt.push_str(&format!("Human: {}\n\n", content)),
            MessageRole::Assistant => prompt.push_str(&format!("Assistant: {}\n\n", content)),
            MessageRole::Function | MessageRole::Tool => {
                prompt.push_str(&format!("Tool: {}\n\n", content));
            }
        }
    }

    // Add Assistant prompt at the end for completion
    prompt.push_str("Assistant:");
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    fn create_user_message(text: &str) -> ChatMessage {
        ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text(text.to_string())),
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    fn create_assistant_message(text: &str) -> ChatMessage {
        ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text(text.to_string())),
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    fn create_system_message(text: &str) -> ChatMessage {
        ChatMessage {
            role: MessageRole::System,
            content: Some(MessageContent::Text(text.to_string())),
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    #[test]
    fn test_messages_to_prompt_user_only() {
        let messages = vec![create_user_message("Hello")];
        let prompt = messages_to_prompt(&messages);
        assert!(prompt.contains("Human: Hello"));
        assert!(prompt.ends_with("Assistant:"));
    }

    #[test]
    fn test_messages_to_prompt_conversation() {
        let messages = vec![
            create_user_message("Hi"),
            create_assistant_message("Hello!"),
            create_user_message("How are you?"),
        ];
        let prompt = messages_to_prompt(&messages);
        assert!(prompt.contains("Human: Hi"));
        assert!(prompt.contains("Assistant: Hello!"));
        assert!(prompt.contains("Human: How are you?"));
    }

    #[test]
    fn test_messages_to_prompt_with_system() {
        let messages = vec![
            create_system_message("You are a helpful assistant"),
            create_user_message("Hello"),
        ];
        let prompt = messages_to_prompt(&messages);
        assert!(prompt.contains("System: You are a helpful assistant"));
        assert!(prompt.contains("Human: Hello"));
    }

    #[test]
    fn test_messages_to_prompt_tool_role() {
        let messages = vec![ChatMessage {
            role: MessageRole::Tool,
            content: Some(MessageContent::Text("Tool result".to_string())),
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: Some("call_123".to_string()),
        }];
        let prompt = messages_to_prompt(&messages);
        assert!(prompt.contains("Tool: Tool result"));
    }

    #[test]
    fn test_messages_to_prompt_empty_content() {
        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: None,
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        }];
        let prompt = messages_to_prompt(&messages);
        // Empty content messages should be skipped
        assert_eq!(prompt, "Assistant:");
    }

    #[test]
    fn runtime_resolved_invoke_uses_generic_request_shape_by_default() {
        let mut request =
            ChatRequest::new("arn:aws:bedrock:us-east-1:123456789012:imported-model/ABC123")
                .add_user_message("Hello");
        request.max_tokens = Some(64);
        request.temperature = Some(0.2);

        let config = crate::core::providers::bedrock::get_model_config_for_model_id(&request.model)
            .unwrap_or_else(|err| panic!("runtime config should resolve: {err}"));
        let body = transform_for_model(&request, config)
            .unwrap_or_else(|err| panic!("request should transform: {err}"));

        assert!(
            body["prompt"]
                .as_str()
                .unwrap_or_default()
                .contains("Hello")
        );
        assert_eq!(body["max_tokens"], 64);
        let temperature = body["temperature"]
            .as_f64()
            .unwrap_or_else(|| panic!("temperature should be numeric"));
        assert!((temperature - 0.2).abs() < 0.000001);
        assert!(body.get("messages").is_none());
        assert!(body.get("inferenceConfig").is_none());
    }

    #[test]
    fn runtime_resolved_invoke_uses_openai_request_shape_when_requested() {
        let mut request =
            ChatRequest::new("arn:aws:bedrock:us-east-1:123456789012:imported-model/ABC123")
                .add_user_message("Hello");
        request.max_tokens = Some(64);
        request.extra_params.insert(
            "bedrock_invoke_schema".to_string(),
            serde_json::json!("openai_chat"),
        );

        let config = crate::core::providers::bedrock::get_model_config_for_model_id(&request.model)
            .unwrap_or_else(|err| panic!("runtime config should resolve: {err}"));
        let body = transform_for_model(&request, config)
            .unwrap_or_else(|err| panic!("request should transform: {err}"));

        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "Hello");
        assert_eq!(body["max_tokens"], 64);
        assert!(body.get("prompt").is_none());
    }
}
