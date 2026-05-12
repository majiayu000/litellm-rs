//! Cohere Model Transformations

use crate::core::providers::bedrock::model_config::ModelConfig;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use serde_json::{Value, json};

/// Transform request for Cohere models
pub fn transform_request(
    request: &ChatRequest,
    _model_config: &ModelConfig,
) -> Result<Value, ProviderError> {
    // Newer Cohere models (Command R) support chat format
    if request.model.contains("command-r") {
        transform_command_r_request(request)
    } else {
        // Older Cohere models use prompt format
        transform_command_request(request)
    }
}

/// Transform request for Command R models (chat format)
fn transform_command_r_request(request: &ChatRequest) -> Result<Value, ProviderError> {
    use crate::core::types::{message::MessageContent, message::MessageRole};

    let mut chat_history = Vec::new();
    let mut message = String::new();
    let mut preamble = None;

    for msg in &request.messages {
        let content = match &msg.content {
            Some(MessageContent::Text(text)) => text.clone(),
            Some(MessageContent::Parts(parts)) => parts
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
            None => continue,
        };

        match msg.role {
            MessageRole::System => {
                preamble = Some(content);
            }
            MessageRole::User => {
                // If there's a previous message, add it to history
                if !message.is_empty() {
                    chat_history.push(json!({
                        "role": "USER",
                        "message": message.clone()
                    }));
                }
                message = content;
            }
            MessageRole::Assistant => {
                // Add user message to history if exists
                if !message.is_empty() {
                    chat_history.push(json!({
                        "role": "USER",
                        "message": message.clone()
                    }));
                    message.clear();
                }
                // Add assistant message
                chat_history.push(json!({
                    "role": "CHATBOT",
                    "message": content
                }));
            }
            _ => {}
        }
    }

    let mut body = json!({
        "message": message,
        "max_tokens": request.max_tokens.unwrap_or(4096),
    });

    if !chat_history.is_empty() {
        body["chat_history"] = json!(chat_history);
    }

    if let Some(preamble_text) = preamble {
        body["preamble"] = json!(preamble_text);
    }

    if let Some(temp) = request.temperature {
        body["temperature"] = json!(temp);
    }

    if let Some(top_p) = request.top_p {
        body["p"] = json!(top_p);
    }

    if let Some(stop) = &request.stop {
        body["stop_sequences"] = json!(stop);
    }

    Ok(body)
}

/// Transform request for older Command models (prompt format)
fn transform_command_request(request: &ChatRequest) -> Result<Value, ProviderError> {
    let prompt = super::messages_to_prompt(&request.messages);

    let mut body = json!({
        "prompt": prompt,
        "max_tokens": request.max_tokens.unwrap_or(4096),
    });

    if let Some(temp) = request.temperature {
        body["temperature"] = json!(temp);
    }

    if let Some(top_p) = request.top_p {
        body["p"] = json!(top_p);
    }

    if let Some(stop) = &request.stop {
        body["stop_sequences"] = json!(stop);
    }

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::bedrock::model_config::{BedrockApiType, BedrockModelFamily};
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    fn create_test_request(model: &str) -> ChatRequest {
        ChatRequest {
            model: model.to_string(),
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
            family: BedrockModelFamily::Cohere,
            api_type: BedrockApiType::Invoke,
            supports_streaming: true,
            supports_function_calling: false,
            supports_multimodal: false,
            max_context_length: 128000,
            max_output_length: Some(4096),
            input_cost_per_1k: 0.0,
            output_cost_per_1k: 0.0,
        }
    }

    #[test]
    fn test_transform_request_command_r() {
        let request = create_test_request("cohere.command-r-v1");
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        // Command R uses chat format
        assert!(value["message"].is_string());
        assert_eq!(value["max_tokens"], 4096);
    }

    #[test]
    fn test_transform_request_command() {
        let request = create_test_request("cohere.command-text-v14");
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        // Older Command uses prompt format
        assert!(value["prompt"].is_string());
        assert_eq!(value["max_tokens"], 4096);
    }

    #[test]
    fn test_transform_command_r_with_system() {
        let request = ChatRequest {
            model: "cohere.command-r-v1".to_string(),
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

        let result = transform_command_r_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["preamble"], "You are helpful");
        assert_eq!(value["message"], "Hello");
    }

    #[test]
    fn test_transform_command_r_with_chat_history() {
        let request = ChatRequest {
            model: "cohere.command-r-v1".to_string(),
            messages: vec![
                ChatMessage {
                    role: MessageRole::User,
                    content: Some(MessageContent::Text("Hi".to_string())),
                    thinking: None,
                    audio: None,
                    name: None,
                    function_call: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text("Hello!".to_string())),
                    thinking: None,
                    audio: None,
                    name: None,
                    function_call: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                ChatMessage {
                    role: MessageRole::User,
                    content: Some(MessageContent::Text("How are you?".to_string())),
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

        let result = transform_command_r_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["chat_history"].is_array());
        assert_eq!(value["message"], "How are you?");
    }

    #[test]
    fn test_transform_command_r_with_temperature() {
        let mut request = create_test_request("cohere.command-r-v1");
        request.temperature = Some(0.5);

        let result = transform_command_r_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["temperature"], 0.5);
    }

    #[test]
    fn test_transform_command_r_with_top_p() {
        let mut request = create_test_request("cohere.command-r-v1");
        request.top_p = Some(0.5);

        let result = transform_command_r_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["p"], 0.5);
    }

    #[test]
    fn test_transform_command_r_with_stop() {
        let mut request = create_test_request("cohere.command-r-v1");
        request.stop = Some(vec!["STOP".to_string()]);

        let result = transform_command_r_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["stop_sequences"].is_array());
    }

    #[test]
    fn test_transform_command_with_temperature() {
        let mut request = create_test_request("cohere.command-text-v14");
        request.temperature = Some(0.5);

        let result = transform_command_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["temperature"], 0.5);
    }

    #[test]
    fn test_transform_command_with_top_p() {
        let mut request = create_test_request("cohere.command-text-v14");
        request.top_p = Some(0.5);

        let result = transform_command_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["p"], 0.5);
    }
}
