//! Meta Llama Model Transformations

use crate::core::providers::bedrock::model_config::ModelConfig;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use serde_json::{Value, json};

/// Transform request for Meta Llama models
pub fn transform_request(
    request: &ChatRequest,
    model_config: &ModelConfig,
) -> Result<Value, ProviderError> {
    // Newer Llama models support message format, older ones need prompt format
    if model_config.family
        == crate::core::providers::bedrock::model_config::BedrockModelFamily::Llama
        && request.model.contains("llama3")
    {
        // Llama 3 uses message format similar to Claude
        transform_llama3_request(request)
    } else {
        // Llama 2 uses prompt format
        transform_llama2_request(request)
    }
}

/// Transform request for Llama 3 models (message format)
fn transform_llama3_request(request: &ChatRequest) -> Result<Value, ProviderError> {
    let mut body = json!({
        "messages": request.messages,
        "max_tokens": request.max_tokens.unwrap_or(4096),
    });

    if let Some(temp) = request.temperature {
        body["temperature"] = json!(temp);
    }

    if let Some(top_p) = request.top_p {
        body["top_p"] = json!(top_p);
    }

    Ok(body)
}

/// Transform request for Llama 2 models (prompt format)
fn transform_llama2_request(request: &ChatRequest) -> Result<Value, ProviderError> {
    let prompt = format_llama2_prompt(&request.messages);

    let mut body = json!({
        "prompt": prompt,
        "max_gen_len": request.max_tokens.unwrap_or(512),
    });

    if let Some(temp) = request.temperature {
        body["temperature"] = json!(temp);
    }

    if let Some(top_p) = request.top_p {
        body["top_p"] = json!(top_p);
    }

    Ok(body)
}

/// Format messages for Llama 2 prompt format
fn format_llama2_prompt(messages: &[crate::core::types::chat::ChatMessage]) -> String {
    use crate::core::types::{message::MessageContent, message::MessageRole};

    let mut prompt = String::from("<s>");
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
                    prompt.push_str(&format!(
                        "[INST] <<SYS>>\n{}\n<</SYS>>\n\n{} [/INST]",
                        sys, content
                    ));
                    system_prompt = None; // Use system prompt only once
                } else {
                    prompt.push_str(&format!("[INST] {} [/INST]", content));
                }
            }
            MessageRole::Assistant => {
                prompt.push_str(&format!(" {} </s><s>", content));
            }
            _ => {}
        }
    }

    prompt
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

    fn create_llama3_model_config() -> ModelConfig {
        ModelConfig {
            family: BedrockModelFamily::Llama,
            api_type: BedrockApiType::Converse,
            supports_streaming: true,
            supports_function_calling: true,
            supports_multimodal: false,
            max_context_length: 128000,
            max_output_length: Some(4096),
            input_cost_per_1k: 0.0,
            output_cost_per_1k: 0.0,
        }
    }

    fn create_llama2_model_config() -> ModelConfig {
        ModelConfig {
            family: BedrockModelFamily::Llama,
            api_type: BedrockApiType::Invoke,
            supports_streaming: true,
            supports_function_calling: false,
            supports_multimodal: false,
            max_context_length: 4096,
            max_output_length: Some(4096),
            input_cost_per_1k: 0.0,
            output_cost_per_1k: 0.0,
        }
    }

    #[test]
    fn test_transform_request_llama3() {
        let request = create_test_request("meta.llama3-70b-instruct-v1");
        let model_config = create_llama3_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        // Llama 3 uses message format
        assert!(value["messages"].is_array());
        assert_eq!(value["max_tokens"], 4096);
    }

    #[test]
    fn test_transform_request_llama2() {
        let request = create_test_request("meta.llama2-70b-chat-v1");
        let model_config = create_llama2_model_config();

        let result = transform_request(&request, &model_config);
        assert!(result.is_ok());
        let value = result.unwrap();
        // Llama 2 uses prompt format
        assert!(value["prompt"].is_string());
        assert_eq!(value["max_gen_len"], 512);
    }

    #[test]
    fn test_transform_llama3_request_with_temperature() {
        let mut request = create_test_request("meta.llama3-70b-instruct-v1");
        request.temperature = Some(0.5);

        let result = transform_llama3_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["temperature"], 0.5);
    }

    #[test]
    fn test_transform_llama3_request_with_top_p() {
        let mut request = create_test_request("meta.llama3-70b-instruct-v1");
        request.top_p = Some(0.5);

        let result = transform_llama3_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["top_p"], 0.5);
    }

    #[test]
    fn test_transform_llama2_request_with_temperature() {
        let mut request = create_test_request("meta.llama2-70b-chat-v1");
        request.temperature = Some(0.5);

        let result = transform_llama2_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["temperature"], 0.5);
    }

    #[test]
    fn test_transform_llama2_request_with_max_tokens() {
        let mut request = create_test_request("meta.llama2-70b-chat-v1");
        request.max_tokens = Some(100);

        let result = transform_llama2_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["max_gen_len"], 100);
    }

    #[test]
    fn test_format_llama2_prompt_user_only() {
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

        let prompt = format_llama2_prompt(&messages);
        assert!(prompt.starts_with("<s>"));
        assert!(prompt.contains("[INST] Hello [/INST]"));
    }

    #[test]
    fn test_format_llama2_prompt_with_system() {
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

        let prompt = format_llama2_prompt(&messages);
        assert!(prompt.contains("<<SYS>>"));
        assert!(prompt.contains("You are helpful"));
        assert!(prompt.contains("<</SYS>>"));
        assert!(prompt.contains("Hello"));
    }

    #[test]
    fn test_format_llama2_prompt_conversation() {
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

        let prompt = format_llama2_prompt(&messages);
        assert!(prompt.contains("[INST] Hi [/INST]"));
        assert!(prompt.contains("Hello!"));
        assert!(prompt.contains("[INST] How are you? [/INST]"));
    }

    #[test]
    fn test_format_llama2_prompt_empty_content() {
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

        let prompt = format_llama2_prompt(&messages);
        assert_eq!(prompt, "<s>");
    }
}
