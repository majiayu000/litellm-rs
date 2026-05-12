//! Amazon Titan and Nova Model Transformations

use crate::core::providers::bedrock::model_config::ModelConfig;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use serde_json::{Value, json};

#[cfg(test)]
use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

/// Transform request for Amazon Titan models
pub fn transform_titan_request(
    request: &ChatRequest,
    _model_config: &ModelConfig,
) -> Result<Value, ProviderError> {
    let prompt = super::messages_to_prompt(&request.messages);

    let mut text_generation_config = json!({
        "maxTokenCount": request.max_tokens.unwrap_or(4096),
    });

    if let Some(temp) = request.temperature {
        text_generation_config["temperature"] = json!(temp);
    }

    if let Some(top_p) = request.top_p {
        text_generation_config["topP"] = json!(top_p);
    }

    if let Some(stop) = &request.stop {
        text_generation_config["stopSequences"] = json!(stop);
    }

    Ok(json!({
        "inputText": prompt,
        "textGenerationConfig": text_generation_config
    }))
}

/// Transform request for Amazon Nova models
pub fn transform_nova_request(
    request: &ChatRequest,
    _model_config: &ModelConfig,
) -> Result<Value, ProviderError> {
    // Nova models use a format similar to Claude but with some differences
    let mut messages = Vec::new();
    let mut system = None;

    use crate::core::types::{message::MessageContent, message::MessageRole};

    for msg in &request.messages {
        match msg.role {
            MessageRole::System => {
                // Extract system message
                if let Some(content) = &msg.content {
                    system = Some(match content {
                        MessageContent::Text(text) => text.clone(),
                        MessageContent::Parts(parts) => parts
                            .iter()
                            .filter_map(|part| {
                                if let crate::core::types::content::ContentPart::Text { text } =
                                    part
                                {
                                    Some(text.clone())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" "),
                    });
                }
            }
            MessageRole::User | MessageRole::Assistant => {
                messages.push(json!({
                    "role": match msg.role {
                        MessageRole::User => "user",
                        MessageRole::Assistant => "assistant",
                        _ => continue,
                    },
                    "content": match &msg.content {
                        Some(MessageContent::Text(text)) => vec![json!({
                            "text": text
                        })],
                        Some(MessageContent::Parts(parts)) => {
                            parts.iter().filter_map(|part| {
                                match part {
                                    crate::core::types::content::ContentPart::Text { text } => {
                                        Some(json!({"text": text}))
                                    }
                                    crate::core::types::content::ContentPart::Image { .. } => {
                                        // NOTE: image content for Nova Canvas not yet handled
                                        None
                                    }
                                    crate::core::types::content::ContentPart::ImageUrl { .. } => {
                                        // NOTE: image URL content not yet handled
                                        None
                                    }
                                    crate::core::types::content::ContentPart::Audio { .. } => {
                                        // NOTE: audio content not yet handled
                                        None
                                    }
                                    crate::core::types::content::ContentPart::Document { .. } => {
                                        // NOTE: document content not yet handled
                                        None
                                    }
                                    crate::core::types::content::ContentPart::ToolResult { .. } => {
                                        // NOTE: tool result content not yet handled
                                        None
                                    }
                                    crate::core::types::content::ContentPart::ToolUse { .. } => {
                                        // NOTE: tool use content not yet handled
                                        None
                                    }
                                }
                            }).collect()
                        }
                        None => vec![],
                    }
                }));
            }
            _ => {
                // Skip function/tool messages for now
            }
        }
    }

    let mut body = json!({
        "messages": messages,
    });

    if let Some(system_text) = system {
        body["system"] = json!([{
            "text": system_text
        }]);
    }

    // Add inference configuration
    let mut inference_config = json!({});

    if let Some(max_tokens) = request.max_tokens {
        inference_config["maxTokens"] = json!(max_tokens);
    }

    if let Some(temp) = request.temperature {
        inference_config["temperature"] = json!(temp);
    }

    if let Some(top_p) = request.top_p {
        inference_config["topP"] = json!(top_p);
    }

    if let Some(stop) = &request.stop {
        inference_config["stopSequences"] = json!(stop);
    }

    if inference_config
        .as_object()
        .map(|o| !o.is_empty())
        .unwrap_or(false)
    {
        body["inferenceConfig"] = inference_config;
    }

    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::bedrock::model_config::{BedrockApiType, BedrockModelFamily};

    fn create_test_request() -> ChatRequest {
        ChatRequest {
            model: "amazon.titan-text-express-v1".to_string(),
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
            family: BedrockModelFamily::TitanText,
            api_type: BedrockApiType::Invoke,
            supports_streaming: true,
            supports_function_calling: false,
            supports_multimodal: false,
            max_context_length: 8192,
            max_output_length: Some(4096),
            input_cost_per_1k: 0.0,
            output_cost_per_1k: 0.0,
        }
    }

    #[test]
    fn test_transform_titan_request_basic() {
        let request = create_test_request();
        let model_config = create_test_model_config();

        let result = transform_titan_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["inputText"].is_string());
        assert!(value["textGenerationConfig"].is_object());
        assert_eq!(value["textGenerationConfig"]["maxTokenCount"], 4096);
    }

    #[test]
    fn test_transform_titan_request_with_temperature() {
        let mut request = create_test_request();
        request.temperature = Some(0.5);
        let model_config = create_test_model_config();

        let result = transform_titan_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["textGenerationConfig"]["temperature"], 0.5);
    }

    #[test]
    fn test_transform_titan_request_with_top_p() {
        let mut request = create_test_request();
        request.top_p = Some(0.5);
        let model_config = create_test_model_config();

        let result = transform_titan_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["textGenerationConfig"]["topP"], 0.5);
    }

    #[test]
    fn test_transform_titan_request_with_max_tokens() {
        let mut request = create_test_request();
        request.max_tokens = Some(100);
        let model_config = create_test_model_config();

        let result = transform_titan_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["textGenerationConfig"]["maxTokenCount"], 100);
    }

    #[test]
    fn test_transform_titan_request_with_stop() {
        let mut request = create_test_request();
        request.stop = Some(vec!["STOP".to_string()]);
        let model_config = create_test_model_config();

        let result = transform_titan_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["textGenerationConfig"]["stopSequences"].is_array());
    }

    fn create_nova_model_config() -> ModelConfig {
        ModelConfig {
            family: BedrockModelFamily::Nova,
            api_type: BedrockApiType::Converse,
            supports_streaming: true,
            supports_function_calling: true,
            supports_multimodal: true,
            max_context_length: 128000,
            max_output_length: Some(4096),
            input_cost_per_1k: 0.0,
            output_cost_per_1k: 0.0,
        }
    }

    #[test]
    fn test_transform_nova_request_basic() {
        let request = create_test_request();
        let model_config = create_nova_model_config();

        let result = transform_nova_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["messages"].is_array());
    }

    #[test]
    fn test_transform_nova_request_with_system() {
        let request = ChatRequest {
            model: "amazon.nova-pro-v1".to_string(),
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
        let model_config = create_nova_model_config();

        let result = transform_nova_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["system"].is_array());
        assert!(value["messages"].is_array());
    }

    #[test]
    fn test_transform_nova_request_with_inference_config() {
        let mut request = create_test_request();
        request.max_tokens = Some(100);
        request.temperature = Some(0.5);
        request.top_p = Some(0.5);
        let model_config = create_nova_model_config();

        let result = transform_nova_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["inferenceConfig"].is_object());
        assert_eq!(value["inferenceConfig"]["maxTokens"], 100);
        assert_eq!(value["inferenceConfig"]["temperature"], 0.5);
        assert_eq!(value["inferenceConfig"]["topP"], 0.5);
    }
}
