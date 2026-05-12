//! Mistral Model Transformations

use crate::core::providers::bedrock::model_config::ModelConfig;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use serde_json::{Value, json};

/// Transform request for Mistral models
pub fn transform_request(
    request: &ChatRequest,
    _model_config: &ModelConfig,
) -> Result<Value, ProviderError> {
    let prompt = format_mistral_prompt(&request.messages);

    let mut body = json!({
        "prompt": prompt,
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

/// Format messages for Mistral prompt format
fn format_mistral_prompt(messages: &[crate::core::types::chat::ChatMessage]) -> String {
    use crate::core::types::{message::MessageContent, message::MessageRole};

    let mut prompt = String::new();
    let mut system_prompt = None;

    for message in messages {
        let content = match &message.content {
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

        match message.role {
            MessageRole::System => {
                system_prompt = Some(content);
            }
            MessageRole::User => {
                if let Some(sys) = &system_prompt {
                    prompt.push_str(&format!("<s>[INST] {}\n\n{} [/INST]", sys, content));
                    system_prompt = None; // Use system prompt only once
                } else {
                    prompt.push_str(&format!("<s>[INST] {} [/INST]", content));
                }
            }
            MessageRole::Assistant => {
                prompt.push_str(&format!("{}</s>", content));
            }
            _ => {}
        }
    }

    // Add instruction tag for the model to continue
    if !prompt.is_empty() && !prompt.ends_with("[/INST]") {
        prompt.push_str("<s>[INST]");
    }

    prompt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::bedrock::model_config::{BedrockApiType, BedrockModelFamily};
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    fn create_test_request() -> ChatRequest {
        ChatRequest {
            model: "mistral.mistral-7b-instruct-v0".to_string(),
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
            family: BedrockModelFamily::Mistral,
            api_type: BedrockApiType::Invoke,
            supports_streaming: true,
            supports_function_calling: false,
            supports_multimodal: false,
            max_context_length: 32000,
            max_output_length: Some(8000),
            input_cost_per_1k: 0.0,
            output_cost_per_1k: 0.0,
        }
    }

    #[test]
    fn test_transform_request_basic() {
        let request = create_test_request();
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["prompt"].is_string());
        assert_eq!(value["max_tokens"], 4096);
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
    fn test_transform_request_with_stop() {
        let mut request = create_test_request();
        request.stop = Some(vec!["STOP".to_string()]);
        let model_config = create_test_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert!(value["stop"].is_array());
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
    fn test_format_mistral_prompt_user_only() {
        let messages = vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        }];

        let prompt = format_mistral_prompt(&messages);
        assert!(prompt.contains("[INST] Hello [/INST]"));
    }

    #[test]
    fn test_format_mistral_prompt_with_system() {
        let messages = vec![
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
        ];

        let prompt = format_mistral_prompt(&messages);
        assert!(prompt.contains("You are helpful"));
        assert!(prompt.contains("Hello"));
        assert!(prompt.contains("[INST]"));
    }

    #[test]
    fn test_format_mistral_prompt_conversation() {
        let messages = vec![
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
        ];

        let prompt = format_mistral_prompt(&messages);
        assert!(prompt.contains("[INST] Hi [/INST]"));
        assert!(prompt.contains("Hello!</s>"));
        assert!(prompt.contains("[INST] How are you? [/INST]"));
    }

    #[test]
    fn test_format_mistral_prompt_empty_content() {
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

        let prompt = format_mistral_prompt(&messages);
        assert!(prompt.is_empty() || prompt == "<s>[INST]");
    }
}
