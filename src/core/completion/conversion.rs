//! Type conversion functions

use super::types::{Choice, CompletionOptions, CompletionResponse};
use crate::core::types::responses::ChatResponse;
#[cfg(test)]
use crate::core::types::responses::Usage;
use crate::core::types::{chat::ChatMessage, chat::ChatRequest};
use crate::utils::error::gateway_error::Result;

/// Convert to chat completion request
pub fn convert_to_chat_completion_request(
    model: &str,
    messages: Vec<ChatMessage>,
    options: CompletionOptions,
) -> Result<ChatRequest> {
    Ok(ChatRequest {
        model: model.to_string(),
        messages,
        temperature: options.temperature,
        max_tokens: options.max_tokens,
        max_completion_tokens: None,
        top_p: options.top_p,
        frequency_penalty: options.frequency_penalty,
        presence_penalty: options.presence_penalty,
        stop: options.stop,
        stream: options.stream,
        stream_options: None,
        tools: None,
        tool_choice: None,
        parallel_tool_calls: None,
        response_format: None,
        user: options.user,
        seed: options.seed,
        n: options.n,
        logit_bias: None,
        functions: None,
        function_call: None,
        logprobs: options.logprobs,
        top_logprobs: options.top_logprobs,
        thinking: None,
        reasoning_effort: None,
        extra_params: options.extra_params,
    })
}

/// Convert from chat completion response
pub fn convert_from_chat_completion_response(response: ChatResponse) -> Result<CompletionResponse> {
    let choices = response
        .choices
        .into_iter()
        .map(|choice| Choice {
            index: choice.index,
            message: choice.message,
            finish_reason: choice.finish_reason,
        })
        .collect();

    Ok(CompletionResponse {
        id: response.id,
        object: response.object,
        created: response.created,
        model: response.model,
        choices,
        usage: response.usage,
    })
}

/// Convert from usage response
#[cfg(test)]
pub fn convert_usage(usage: &crate::core::types::responses::Usage) -> Usage {
    Usage {
        prompt_tokens: usage.prompt_tokens,
        completion_tokens: usage.completion_tokens,
        total_tokens: usage.total_tokens,
        prompt_tokens_details: None,
        completion_tokens_details: None,
        thinking_usage: usage.thinking_usage.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::responses::{ChatChoice, FinishReason};
    use crate::core::types::{
        message::MessageContent, message::MessageRole, thinking::ThinkingUsage,
    };
    use std::collections::HashMap;

    // ==================== Helper Functions ====================

    fn create_test_message(role: MessageRole, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: Some(MessageContent::Text(content.to_string())),
            ..Default::default()
        }
    }

    fn create_test_usage() -> crate::core::types::responses::Usage {
        crate::core::types::responses::Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        }
    }

    fn create_test_chat_response() -> ChatResponse {
        ChatResponse {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: create_test_message(MessageRole::Assistant, "Hello, world!"),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: Some(create_test_usage()),
            system_fingerprint: None,
        }
    }

    // ==================== convert_to_chat_completion_request Tests ====================

    #[test]
    fn test_convert_to_chat_completion_request_basic() {
        let messages = vec![create_test_message(MessageRole::User, "Hello")];
        let options = CompletionOptions::default();

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert_eq!(result.model, "gpt-4");
        assert_eq!(result.messages.len(), 1);
        assert!(!result.stream);
    }

    #[test]
    fn test_convert_to_chat_completion_request_with_temperature() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let options = CompletionOptions {
            temperature: Some(0.7),
            ..Default::default()
        };

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert_eq!(result.temperature, Some(0.7));
    }

    #[test]
    fn test_convert_to_chat_completion_request_with_max_tokens() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let options = CompletionOptions {
            max_tokens: Some(1000),
            ..Default::default()
        };

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert_eq!(result.max_tokens, Some(1000));
    }

    #[test]
    fn test_convert_to_chat_completion_request_with_top_p() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let options = CompletionOptions {
            top_p: Some(0.9),
            ..Default::default()
        };

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert_eq!(result.top_p, Some(0.9));
    }

    #[test]
    fn test_convert_to_chat_completion_request_with_penalties() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let options = CompletionOptions {
            frequency_penalty: Some(0.5),
            presence_penalty: Some(0.3),
            ..Default::default()
        };

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert_eq!(result.frequency_penalty, Some(0.5));
        assert_eq!(result.presence_penalty, Some(0.3));
    }

    #[test]
    fn test_convert_to_chat_completion_request_with_stop() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let options = CompletionOptions {
            stop: Some(vec!["END".to_string(), "STOP".to_string()]),
            ..Default::default()
        };

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert_eq!(
            result.stop,
            Some(vec!["END".to_string(), "STOP".to_string()])
        );
    }

    #[test]
    fn test_convert_to_chat_completion_request_with_stream() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let options = CompletionOptions {
            stream: true,
            ..Default::default()
        };

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert!(result.stream);
    }

    #[test]
    fn test_convert_to_chat_completion_request_with_user() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let options = CompletionOptions {
            user: Some("user_123".to_string()),
            ..Default::default()
        };

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert_eq!(result.user, Some("user_123".to_string()));
    }

    #[test]
    fn test_convert_to_chat_completion_request_with_seed() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let options = CompletionOptions {
            seed: Some(42),
            ..Default::default()
        };

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert_eq!(result.seed, Some(42));
    }

    #[test]
    fn test_convert_to_chat_completion_request_with_n() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let options = CompletionOptions {
            n: Some(3),
            ..Default::default()
        };

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert_eq!(result.n, Some(3));
    }

    #[test]
    fn test_convert_to_chat_completion_request_with_logprobs() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let options = CompletionOptions {
            logprobs: Some(true),
            top_logprobs: Some(5),
            ..Default::default()
        };

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert_eq!(result.logprobs, Some(true));
        assert_eq!(result.top_logprobs, Some(5));
    }

    #[test]
    fn test_convert_to_chat_completion_request_with_extra_params() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let mut options = CompletionOptions::default();
        let mut extra = HashMap::new();
        extra.insert(
            "custom_field".to_string(),
            serde_json::json!("custom_value"),
        );
        options.extra_params = extra;

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert!(result.extra_params.contains_key("custom_field"));
    }

    #[test]
    fn test_convert_to_chat_completion_request_multiple_messages() {
        let messages = vec![
            create_test_message(MessageRole::System, "You are helpful"),
            create_test_message(MessageRole::User, "Hello"),
            create_test_message(MessageRole::Assistant, "Hi there!"),
            create_test_message(MessageRole::User, "How are you?"),
        ];
        let options = CompletionOptions::default();

        let result = convert_to_chat_completion_request("gpt-4", messages, options).unwrap();

        assert_eq!(result.messages.len(), 4);
        assert_eq!(result.messages[0].role, MessageRole::System);
        assert_eq!(result.messages[1].role, MessageRole::User);
        assert_eq!(result.messages[2].role, MessageRole::Assistant);
        assert_eq!(result.messages[3].role, MessageRole::User);
    }

    #[test]
    fn test_convert_to_chat_completion_request_all_options() {
        let messages = vec![create_test_message(MessageRole::User, "Test")];
        let options = CompletionOptions {
            temperature: Some(0.8),
            max_tokens: Some(500),
            top_p: Some(0.95),
            frequency_penalty: Some(0.2),
            presence_penalty: Some(0.1),
            stop: Some(vec!["END".to_string()]),
            stream: true,
            user: Some("user_456".to_string()),
            seed: Some(123),
            n: Some(2),
            ..Default::default()
        };

        let result = convert_to_chat_completion_request("claude-3", messages, options).unwrap();

        assert_eq!(result.model, "claude-3");
        assert_eq!(result.temperature, Some(0.8));
        assert_eq!(result.max_tokens, Some(500));
        assert_eq!(result.top_p, Some(0.95));
        assert_eq!(result.frequency_penalty, Some(0.2));
        assert_eq!(result.presence_penalty, Some(0.1));
        assert!(result.stream);
        assert_eq!(result.user, Some("user_456".to_string()));
        assert_eq!(result.seed, Some(123));
        assert_eq!(result.n, Some(2));
    }

    // ==================== convert_from_chat_completion_response Tests ====================

    #[test]
    fn test_convert_from_chat_completion_response_basic() {
        let response = create_test_chat_response();

        let result = convert_from_chat_completion_response(response).unwrap();

        assert_eq!(result.id, "chatcmpl-123");
        assert_eq!(result.object, "chat.completion");
        assert_eq!(result.created, 1234567890);
        assert_eq!(result.model, "gpt-4");
        assert_eq!(result.choices.len(), 1);
    }

    #[test]
    fn test_convert_from_chat_completion_response_choice_fields() {
        let response = create_test_chat_response();

        let result = convert_from_chat_completion_response(response).unwrap();

        let choice = &result.choices[0];
        assert_eq!(choice.index, 0);
        assert_eq!(choice.message.role, MessageRole::Assistant);
        assert_eq!(choice.finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn test_convert_from_chat_completion_response_with_usage() {
        let response = create_test_chat_response();

        let result = convert_from_chat_completion_response(response).unwrap();

        assert!(result.usage.is_some());
        let usage = result.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_convert_from_chat_completion_response_multiple_choices() {
        let mut response = create_test_chat_response();
        response.choices = vec![
            ChatChoice {
                index: 0,
                message: create_test_message(MessageRole::Assistant, "Response 1"),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            },
            ChatChoice {
                index: 1,
                message: create_test_message(MessageRole::Assistant, "Response 2"),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            },
            ChatChoice {
                index: 2,
                message: create_test_message(MessageRole::Assistant, "Response 3"),
                finish_reason: Some(FinishReason::Length),
                logprobs: None,
            },
        ];

        let result = convert_from_chat_completion_response(response).unwrap();

        assert_eq!(result.choices.len(), 3);
        assert_eq!(result.choices[0].index, 0);
        assert_eq!(result.choices[1].index, 1);
        assert_eq!(result.choices[2].index, 2);
        assert_eq!(result.choices[2].finish_reason, Some(FinishReason::Length));
    }

    #[test]
    fn test_convert_from_chat_completion_response_without_usage() {
        let mut response = create_test_chat_response();
        response.usage = None;

        let result = convert_from_chat_completion_response(response).unwrap();

        assert!(result.usage.is_none());
    }

    #[test]
    fn test_convert_from_chat_completion_response_empty_choices() {
        let mut response = create_test_chat_response();
        response.choices = vec![];

        let result = convert_from_chat_completion_response(response).unwrap();

        assert!(result.choices.is_empty());
    }

    #[test]
    fn test_convert_from_chat_completion_response_different_finish_reasons() {
        let finish_reasons = vec![
            FinishReason::Stop,
            FinishReason::Length,
            FinishReason::ToolCalls,
            FinishReason::ContentFilter,
        ];

        for (i, reason) in finish_reasons.into_iter().enumerate() {
            let mut response = create_test_chat_response();
            response.choices[0].finish_reason = Some(reason.clone());

            let result = convert_from_chat_completion_response(response).unwrap();

            assert_eq!(
                result.choices[0].finish_reason,
                Some(reason),
                "Failed at index {}",
                i
            );
        }
    }

    // ==================== convert_usage Tests ====================

    #[test]
    fn test_convert_usage_basic() {
        let usage = create_test_usage();

        let result = convert_usage(&usage);

        assert_eq!(result.prompt_tokens, 100);
        assert_eq!(result.completion_tokens, 50);
        assert_eq!(result.total_tokens, 150);
    }

    #[test]
    fn test_convert_usage_clears_details() {
        let usage = create_test_usage();

        let result = convert_usage(&usage);

        assert!(result.prompt_tokens_details.is_none());
        assert!(result.completion_tokens_details.is_none());
    }

    #[test]
    fn test_convert_usage_with_thinking_usage() {
        let mut usage = create_test_usage();
        usage.thinking_usage = Some(ThinkingUsage {
            thinking_tokens: Some(25),
            budget_tokens: None,
            thinking_cost: None,
            provider: None,
        });

        let result = convert_usage(&usage);

        assert!(result.thinking_usage.is_some());
        assert_eq!(result.thinking_usage.unwrap().thinking_tokens, Some(25));
    }

    #[test]
    fn test_convert_usage_zero_tokens() {
        let usage = crate::core::types::responses::Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        };

        let result = convert_usage(&usage);

        assert_eq!(result.prompt_tokens, 0);
        assert_eq!(result.completion_tokens, 0);
        assert_eq!(result.total_tokens, 0);
    }

    #[test]
    fn test_convert_usage_large_values() {
        let usage = crate::core::types::responses::Usage {
            prompt_tokens: 100000,
            completion_tokens: 50000,
            total_tokens: 150000,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        };

        let result = convert_usage(&usage);

        assert_eq!(result.prompt_tokens, 100000);
        assert_eq!(result.completion_tokens, 50000);
        assert_eq!(result.total_tokens, 150000);
    }
}
