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
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use serde_json::Value;

/// Transform request based on model family
pub fn transform_for_model(
    request: &ChatRequest,
    model_config: &ModelConfig,
) -> Result<Value, ProviderError> {
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
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        }];
        let prompt = messages_to_prompt(&messages);
        // Empty content messages should be skipped
        assert_eq!(prompt, "Assistant:");
    }
}
