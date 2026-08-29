//! Chat response types

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::super::{chat::ChatMessage, message::MessageContent, tools::ToolCall};
use super::delta::ChatDelta;
use super::logprobs::{FinishReason, LogProbs};
use super::usage::Usage;

pub(crate) const PROVIDER_ANNOTATION_CHUNK_OBJECT: &str = "litellm.provider.annotation.chunk";

/// Chat completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Response ID
    pub id: String,

    /// Object type
    pub object: String,

    /// Creation timestamp
    pub created: i64,

    /// Model used
    pub model: String,

    /// Choice list
    pub choices: Vec<ChatChoice>,

    /// Usage statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,

    /// System fingerprint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

/// Chat choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    /// Choice index
    pub index: u32,

    /// Response message
    pub message: ChatMessage,

    /// Completion reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,

    /// Log probabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<LogProbs>,
}

/// Streaming chat chunk response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChunk {
    /// Response ID
    pub id: String,

    /// Object type
    pub object: String,

    /// Creation timestamp
    pub created: i64,

    /// Model used
    pub model: String,

    /// Choice list
    pub choices: Vec<ChatStreamChoice>,

    /// Usage (usually in last chunk)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,

    /// System fingerprint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

/// Streaming choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatStreamChoice {
    /// Choice index
    pub index: u32,

    /// Delta content
    pub delta: ChatDelta,

    /// Finish reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,

    /// Log probabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<LogProbs>,
}

impl ChatChunk {
    pub(crate) fn is_provider_annotation(&self) -> bool {
        self.object == PROVIDER_ANNOTATION_CHUNK_OBJECT
    }

    /// Build a crate-internal provider annotation event without widening the
    /// stable public response structs. The HTTP boundary recognizes the exact
    /// object discriminator and projects the payload into `delta.annotations`.
    pub(crate) fn provider_annotation(
        id: String,
        created: i64,
        model: String,
        choice_index: u32,
        annotation: Value,
    ) -> Self {
        Self {
            id,
            object: PROVIDER_ANNOTATION_CHUNK_OBJECT.to_string(),
            created,
            model,
            choices: vec![ChatStreamChoice {
                index: choice_index,
                delta: ChatDelta {
                    content: Some(annotation.to_string()),
                    ..Default::default()
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        }
    }
}

impl ChatResponse {
    /// Get first message content
    pub fn first_content(&self) -> Option<&str> {
        self.choices
            .first()
            .and_then(|choice| match &choice.message.content {
                Some(MessageContent::Text(text)) => Some(text.as_str()),
                _ => None,
            })
    }

    /// Get all message contents
    pub fn all_content(&self) -> Vec<&str> {
        self.choices
            .iter()
            .filter_map(|choice| match &choice.message.content {
                Some(MessageContent::Text(text)) => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    /// Check if response has tool calls
    pub fn has_tool_calls(&self) -> bool {
        self.choices
            .iter()
            .any(|choice| choice.message.tool_calls.is_some())
    }

    /// Get first tool calls
    pub fn first_tool_calls(&self) -> Option<&[ToolCall]> {
        self.choices
            .first()
            .and_then(|choice| choice.message.tool_calls.as_ref())
            .map(|calls| calls.as_slice())
    }

    /// Calculate total cost (requires pricing information)
    pub fn calculate_cost(&self, input_cost_per_1k: f64, output_cost_per_1k: f64) -> f64 {
        if let Some(usage) = &self.usage {
            let input_cost = (usage.prompt_tokens as f64 / 1000.0) * input_cost_per_1k;
            let output_cost = (usage.completion_tokens as f64 / 1000.0) * output_cost_per_1k;
            input_cost + output_cost
        } else {
            0.0
        }
    }
}

impl Default for ChatResponse {
    fn default() -> Self {
        Self {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4().simple()),
            object: "chat.completion".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: String::new(),
            choices: Vec::new(),
            usage: None,
            system_fingerprint: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::message::MessageRole;

    fn create_test_message(content: &str) -> ChatMessage {
        ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text(content.to_string())),
            thinking: None,
            audio: None,
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
        }
    }

    fn create_test_response(content: &str) -> ChatResponse {
        ChatResponse {
            id: "chatcmpl-test123".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: create_test_message(content),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }),
            system_fingerprint: None,
        }
    }

    #[test]
    fn test_chat_response_default() {
        let response = ChatResponse::default();
        assert!(response.id.starts_with("chatcmpl-"));
        assert_eq!(response.object, "chat.completion");
        assert!(response.model.is_empty());
        assert!(response.choices.is_empty());
        assert!(response.usage.is_none());
    }

    #[test]
    fn test_chat_response_first_content() {
        let response = create_test_response("Hello, world!");
        assert_eq!(response.first_content(), Some("Hello, world!"));
    }

    #[test]
    fn test_chat_response_first_content_empty() {
        let response = ChatResponse::default();
        assert_eq!(response.first_content(), None);
    }

    #[test]
    fn test_chat_response_all_content() {
        let response = ChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![
                ChatChoice {
                    index: 0,
                    message: create_test_message("First"),
                    finish_reason: None,
                    logprobs: None,
                },
                ChatChoice {
                    index: 1,
                    message: create_test_message("Second"),
                    finish_reason: None,
                    logprobs: None,
                },
            ],
            usage: None,
            system_fingerprint: None,
        };

        let contents = response.all_content();
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0], "First");
        assert_eq!(contents[1], "Second");
    }

    #[test]
    fn test_chat_response_has_tool_calls_false() {
        let response = create_test_response("Hello");
        assert!(!response.has_tool_calls());
    }

    #[test]
    fn test_chat_response_has_tool_calls_true() {
        let response = ChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: None,
                    thinking: None,
                    audio: None,
                    name: None,
                    tool_calls: Some(vec![ToolCall {
                        id: "call_123".to_string(),
                        tool_type: "function".to_string(),
                        function: crate::core::types::tools::FunctionCall {
                            name: "get_weather".to_string(),
                            arguments: "{}".to_string(),
                        },
                    }]),
                    tool_call_id: None,
                    function_call: None,
                },
                finish_reason: Some(FinishReason::ToolCalls),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        assert!(response.has_tool_calls());
    }

    #[test]
    fn test_chat_response_first_tool_calls() {
        let response = ChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: ChatMessage {
                    role: MessageRole::Assistant,
                    content: None,
                    thinking: None,
                    audio: None,
                    name: None,
                    tool_calls: Some(vec![
                        ToolCall {
                            id: "call_1".to_string(),
                            tool_type: "function".to_string(),
                            function: crate::core::types::tools::FunctionCall {
                                name: "func1".to_string(),
                                arguments: "{}".to_string(),
                            },
                        },
                        ToolCall {
                            id: "call_2".to_string(),
                            tool_type: "function".to_string(),
                            function: crate::core::types::tools::FunctionCall {
                                name: "func2".to_string(),
                                arguments: "{}".to_string(),
                            },
                        },
                    ]),
                    tool_call_id: None,
                    function_call: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        let tool_calls = response.first_tool_calls();
        assert!(tool_calls.is_some());
        assert_eq!(tool_calls.unwrap().len(), 2);
    }

    #[test]
    fn test_chat_response_calculate_cost() {
        let response = create_test_response("Hello");
        // 10 prompt tokens at $0.01/1k, 20 completion at $0.03/1k
        let cost = response.calculate_cost(0.01, 0.03);
        let expected = (10.0 / 1000.0) * 0.01 + (20.0 / 1000.0) * 0.03;
        assert!((cost - expected).abs() < f64::EPSILON);
    }

    #[test]
    fn test_chat_response_calculate_cost_no_usage() {
        let response = ChatResponse::default();
        let cost = response.calculate_cost(0.01, 0.03);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_chat_response_serialization() {
        let response = create_test_response("Hello");
        let json = serde_json::to_value(&response).unwrap();

        assert_eq!(json["id"], "chatcmpl-test123");
        assert_eq!(json["object"], "chat.completion");
        assert_eq!(json["model"], "gpt-4");
        assert!(json["choices"].is_array());
        assert!(json["usage"].is_object());
    }

    #[test]
    fn test_chat_choice_clone() {
        let choice = ChatChoice {
            index: 0,
            message: create_test_message("Test"),
            finish_reason: Some(FinishReason::Stop),
            logprobs: None,
        };

        let cloned = choice.clone();
        assert_eq!(choice.index, cloned.index);
    }

    #[test]
    fn test_chat_chunk_structure() {
        let chunk = ChatChunk {
            id: "chunk_123".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![ChatStreamChoice {
                index: 0,
                delta: ChatDelta {
                    role: Some(MessageRole::Assistant),
                    content: Some("Hello".to_string()),
                    thinking: None,
                    tool_calls: None,
                    function_call: None,
                    audio: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        assert_eq!(chunk.id, "chunk_123");
        assert_eq!(chunk.object, "chat.completion.chunk");
        assert_eq!(chunk.choices.len(), 1);
    }

    #[test]
    fn test_chat_stream_choice() {
        let choice = ChatStreamChoice {
            index: 0,
            delta: ChatDelta {
                role: None,
                content: Some("world".to_string()),
                thinking: None,
                tool_calls: None,
                function_call: None,
                audio: None,
            },
            finish_reason: Some(FinishReason::Stop),
            logprobs: None,
        };

        assert_eq!(choice.index, 0);
        assert_eq!(choice.delta.content, Some("world".to_string()));
        assert_eq!(choice.finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn test_chat_chunk_serialization() {
        let chunk = ChatChunk {
            id: "chunk_123".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![],
            usage: None,
            system_fingerprint: None,
        };

        let json = serde_json::to_value(&chunk).unwrap();
        assert_eq!(json["object"], "chat.completion.chunk");
    }
}
