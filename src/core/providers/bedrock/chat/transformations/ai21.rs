//! AI21 Labs Model Transformations

use crate::core::providers::bedrock::model_config::ModelConfig;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use serde_json::{Value, json};

/// Transform request for AI21 models
pub fn transform_request(
    request: &ChatRequest,
    _model_config: &ModelConfig,
) -> Result<Value, ProviderError> {
    // AI21 Jamba models use their own format
    if request.model.contains("jamba") {
        transform_jamba_request(request)
    } else {
        // Older Jurassic models
        transform_jurassic_request(request)
    }
}

/// Transform request for Jamba models
fn transform_jamba_request(request: &ChatRequest) -> Result<Value, ProviderError> {
    use crate::core::types::{message::MessageContent, message::MessageRole};

    let mut messages = Vec::new();

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

        let role = match msg.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            _ => continue,
        };

        messages.push(json!({
            "role": role,
            "content": content
        }));
    }

    let mut body = json!({
        "messages": messages,
        "max_tokens": request.max_tokens.unwrap_or(4096),
    });

    if let Some(temp) = request.temperature {
        body["temperature"] = json!(temp);
    }

    if let Some(top_p) = request.top_p {
        body["top_p"] = json!(top_p);
    }

    if let Some(stop) = &request.stop {
        body["stop"] = json!(stop);
    }

    Ok(body)
}

/// Transform request for Jurassic models
fn transform_jurassic_request(request: &ChatRequest) -> Result<Value, ProviderError> {
    let prompt = super::messages_to_prompt(&request.messages);

    let mut body = json!({
        "prompt": prompt,
        "maxTokens": request.max_tokens.unwrap_or(4096),
    });

    if let Some(temp) = request.temperature {
        body["temperature"] = json!(temp);
    }

    if let Some(top_p) = request.top_p {
        body["topP"] = json!(top_p);
    }

    if let Some(stop) = &request.stop {
        body["stopSequences"] = json!(stop);
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
            family: BedrockModelFamily::AI21,
            api_type: BedrockApiType::Invoke,
            supports_streaming: true,
            supports_function_calling: false,
            supports_multimodal: false,
            max_context_length: 256000,
            max_output_length: Some(4096),
            input_cost_per_1k: 0.0,
            output_cost_per_1k: 0.0,
        }
    }

    #[test]
    fn test_transform_request_jamba() {
        let request = create_test_request("ai21.jamba-instruct-v1");
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        // Jamba uses messages format
        assert!(value["messages"].is_array());
        assert_eq!(value["max_tokens"], 4096);
    }

    #[test]
    fn test_transform_request_jurassic() {
        let request = create_test_request("ai21.j2-ultra-v1");
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        // Jurassic uses prompt format
        assert!(value["prompt"].is_string());
        assert_eq!(value["maxTokens"], 4096);
    }

    #[test]
    fn test_transform_jamba_with_system() {
        let request = ChatRequest {
            model: "ai21.jamba-instruct-v1".to_string(),
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

        let result = transform_jamba_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        let messages = value["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
    }

    #[test]
    fn test_transform_jamba_with_temperature() {
        let mut request = create_test_request("ai21.jamba-instruct-v1");
        request.temperature = Some(0.5);

        let result = transform_jamba_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["temperature"], 0.5);
    }

    #[test]
    fn test_transform_jamba_with_top_p() {
        let mut request = create_test_request("ai21.jamba-instruct-v1");
        request.top_p = Some(0.5);

        let result = transform_jamba_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["top_p"], 0.5);
    }

    #[test]
    fn test_transform_jamba_with_stop() {
        let mut request = create_test_request("ai21.jamba-instruct-v1");
        request.stop = Some(vec!["STOP".to_string()]);

        let result = transform_jamba_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["stop"].is_array());
    }

    #[test]
    fn test_transform_jurassic_with_temperature() {
        let mut request = create_test_request("ai21.j2-ultra-v1");
        request.temperature = Some(0.5);

        let result = transform_jurassic_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["temperature"], 0.5);
    }

    #[test]
    fn test_transform_jurassic_with_top_p() {
        let mut request = create_test_request("ai21.j2-ultra-v1");
        request.top_p = Some(0.5);

        let result = transform_jurassic_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["topP"], 0.5);
    }

    #[test]
    fn test_transform_jurassic_with_stop() {
        let mut request = create_test_request("ai21.j2-ultra-v1");
        request.stop = Some(vec!["STOP".to_string()]);

        let result = transform_jurassic_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["stopSequences"].is_array());
    }

    #[test]
    fn test_transform_jurassic_with_max_tokens() {
        let mut request = create_test_request("ai21.j2-ultra-v1");
        request.max_tokens = Some(100);

        let result = transform_jurassic_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["maxTokens"], 100);
    }
}
