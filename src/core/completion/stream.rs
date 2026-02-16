//! Completion streaming types

#[cfg(test)]
use crate::core::streaming::types::ChatCompletionChunk;
use crate::core::types::responses::FinishReason;
use futures::stream::BoxStream;

/// Streaming completion response
pub type CompletionStream =
    BoxStream<'static, Result<CompletionChunk, crate::utils::error::gateway_error::GatewayError>>;

/// Chunk in a streaming completion response
#[derive(Debug, Clone)]
pub struct CompletionChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<StreamChoice>,
}

/// Choice in a streaming chunk
#[derive(Debug, Clone)]
pub struct StreamChoice {
    pub index: u32,
    pub delta: StreamDelta,
    pub finish_reason: Option<FinishReason>,
}

/// Delta content in streaming response
#[derive(Debug, Clone, Default)]
pub struct StreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<crate::core::completion::types::ToolCall>>,
}

/// Convert internal stream chunk to completion chunk
#[cfg(test)]
pub fn convert_stream_chunk(chunk: ChatCompletionChunk) -> CompletionChunk {
    CompletionChunk {
        id: chunk.id,
        object: chunk.object,
        created: chunk.created as i64,
        model: chunk.model,
        choices: chunk
            .choices
            .into_iter()
            .map(|c| StreamChoice {
                index: c.index,
                delta: StreamDelta {
                    role: c.delta.role.map(|r| r.to_string()),
                    content: c.delta.content,
                    tool_calls: None,
                },
                finish_reason: c.finish_reason.and_then(|s| parse_finish_reason(&s)),
            })
            .collect(),
    }
}

/// Parse finish reason string to FinishReason enum
#[cfg(test)]
fn parse_finish_reason(s: &str) -> Option<FinishReason> {
    match s.to_lowercase().as_str() {
        "stop" => Some(FinishReason::Stop),
        "length" => Some(FinishReason::Length),
        "tool_calls" | "function_call" => Some(FinishReason::ToolCalls),
        "content_filter" => Some(FinishReason::ContentFilter),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::streaming::types::{ChatCompletionChunkChoice, ChatCompletionDelta};

    // ==================== CompletionChunk Tests ====================

    #[test]
    fn test_completion_chunk_creation() {
        let chunk = CompletionChunk {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![],
        };

        assert_eq!(chunk.id, "chatcmpl-123");
        assert_eq!(chunk.object, "chat.completion.chunk");
        assert_eq!(chunk.created, 1234567890);
        assert_eq!(chunk.model, "gpt-4");
        assert!(chunk.choices.is_empty());
    }

    #[test]
    fn test_completion_chunk_with_choices() {
        let chunk = CompletionChunk {
            id: "chatcmpl-456".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: Some("assistant".to_string()),
                    content: Some("Hello".to_string()),
                    tool_calls: None,
                },
                finish_reason: None,
            }],
        };

        assert_eq!(chunk.choices.len(), 1);
        assert_eq!(chunk.choices[0].index, 0);
    }

    #[test]
    fn test_completion_chunk_clone() {
        let chunk = CompletionChunk {
            id: "chatcmpl-789".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![],
        };

        let cloned = chunk.clone();
        assert_eq!(chunk.id, cloned.id);
        assert_eq!(chunk.model, cloned.model);
    }

    #[test]
    fn test_completion_chunk_debug() {
        let chunk = CompletionChunk {
            id: "test".to_string(),
            object: "test".to_string(),
            created: 0,
            model: "test".to_string(),
            choices: vec![],
        };

        let debug_str = format!("{:?}", chunk);
        assert!(debug_str.contains("CompletionChunk"));
    }

    // ==================== StreamChoice Tests ====================

    #[test]
    fn test_stream_choice_creation() {
        let choice = StreamChoice {
            index: 0,
            delta: StreamDelta::default(),
            finish_reason: None,
        };

        assert_eq!(choice.index, 0);
        assert!(choice.finish_reason.is_none());
    }

    #[test]
    fn test_stream_choice_with_finish_reason() {
        let choice = StreamChoice {
            index: 1,
            delta: StreamDelta::default(),
            finish_reason: Some(FinishReason::Stop),
        };

        assert_eq!(choice.index, 1);
        assert_eq!(choice.finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn test_stream_choice_with_content() {
        let choice = StreamChoice {
            index: 0,
            delta: StreamDelta {
                role: Some("assistant".to_string()),
                content: Some("Hello, world!".to_string()),
                tool_calls: None,
            },
            finish_reason: None,
        };

        assert_eq!(choice.delta.content, Some("Hello, world!".to_string()));
        assert_eq!(choice.delta.role, Some("assistant".to_string()));
    }

    #[test]
    fn test_stream_choice_clone() {
        let choice = StreamChoice {
            index: 5,
            delta: StreamDelta {
                role: Some("user".to_string()),
                content: Some("Test".to_string()),
                tool_calls: None,
            },
            finish_reason: Some(FinishReason::Length),
        };

        let cloned = choice.clone();
        assert_eq!(choice.index, cloned.index);
        assert_eq!(choice.finish_reason, cloned.finish_reason);
    }

    // ==================== StreamDelta Tests ====================

    #[test]
    fn test_stream_delta_default() {
        let delta = StreamDelta::default();

        assert!(delta.role.is_none());
        assert!(delta.content.is_none());
        assert!(delta.tool_calls.is_none());
    }

    #[test]
    fn test_stream_delta_with_role() {
        let delta = StreamDelta {
            role: Some("assistant".to_string()),
            content: None,
            tool_calls: None,
        };

        assert_eq!(delta.role, Some("assistant".to_string()));
    }

    #[test]
    fn test_stream_delta_with_content() {
        let delta = StreamDelta {
            role: None,
            content: Some("Hello".to_string()),
            tool_calls: None,
        };

        assert_eq!(delta.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_stream_delta_full() {
        let delta = StreamDelta {
            role: Some("assistant".to_string()),
            content: Some("Test content".to_string()),
            tool_calls: None,
        };

        assert!(delta.role.is_some());
        assert!(delta.content.is_some());
        assert!(delta.tool_calls.is_none());
    }

    #[test]
    fn test_stream_delta_clone() {
        let delta = StreamDelta {
            role: Some("user".to_string()),
            content: Some("Clone test".to_string()),
            tool_calls: None,
        };

        let cloned = delta.clone();
        assert_eq!(delta.role, cloned.role);
        assert_eq!(delta.content, cloned.content);
    }

    // ==================== parse_finish_reason Tests ====================

    #[test]
    fn test_parse_finish_reason_stop() {
        assert_eq!(parse_finish_reason("stop"), Some(FinishReason::Stop));
        assert_eq!(parse_finish_reason("Stop"), Some(FinishReason::Stop));
        assert_eq!(parse_finish_reason("STOP"), Some(FinishReason::Stop));
    }

    #[test]
    fn test_parse_finish_reason_length() {
        assert_eq!(parse_finish_reason("length"), Some(FinishReason::Length));
        assert_eq!(parse_finish_reason("Length"), Some(FinishReason::Length));
        assert_eq!(parse_finish_reason("LENGTH"), Some(FinishReason::Length));
    }

    #[test]
    fn test_parse_finish_reason_tool_calls() {
        assert_eq!(
            parse_finish_reason("tool_calls"),
            Some(FinishReason::ToolCalls)
        );
        assert_eq!(
            parse_finish_reason("Tool_Calls"),
            Some(FinishReason::ToolCalls)
        );
        assert_eq!(
            parse_finish_reason("TOOL_CALLS"),
            Some(FinishReason::ToolCalls)
        );
    }

    #[test]
    fn test_parse_finish_reason_function_call() {
        assert_eq!(
            parse_finish_reason("function_call"),
            Some(FinishReason::ToolCalls)
        );
        assert_eq!(
            parse_finish_reason("Function_Call"),
            Some(FinishReason::ToolCalls)
        );
    }

    #[test]
    fn test_parse_finish_reason_content_filter() {
        assert_eq!(
            parse_finish_reason("content_filter"),
            Some(FinishReason::ContentFilter)
        );
        assert_eq!(
            parse_finish_reason("Content_Filter"),
            Some(FinishReason::ContentFilter)
        );
    }

    #[test]
    fn test_parse_finish_reason_unknown() {
        assert_eq!(parse_finish_reason("unknown"), None);
        assert_eq!(parse_finish_reason(""), None);
        assert_eq!(parse_finish_reason("error"), None);
        assert_eq!(parse_finish_reason("cancelled"), None);
    }

    // ==================== convert_stream_chunk Tests ====================

    #[test]
    fn test_convert_stream_chunk_basic() {
        let input_chunk = ChatCompletionChunk {
            id: "chatcmpl-abc".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![],
            system_fingerprint: None,
            usage: None,
        };

        let result = convert_stream_chunk(input_chunk);

        assert_eq!(result.id, "chatcmpl-abc");
        assert_eq!(result.object, "chat.completion.chunk");
        assert_eq!(result.created, 1234567890);
        assert_eq!(result.model, "gpt-4");
        assert!(result.choices.is_empty());
    }

    #[test]
    fn test_convert_stream_chunk_with_choice() {
        let input_chunk = ChatCompletionChunk {
            id: "chatcmpl-xyz".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1000000000,
            model: "gpt-3.5-turbo".to_string(),
            choices: vec![ChatCompletionChunkChoice {
                index: 0,
                delta: ChatCompletionDelta {
                    role: Some(crate::core::types::message::MessageRole::Assistant),
                    content: Some("Hello".to_string()),
                    tool_calls: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            system_fingerprint: None,
            usage: None,
        };

        let result = convert_stream_chunk(input_chunk);

        assert_eq!(result.choices.len(), 1);
        assert_eq!(result.choices[0].index, 0);
        assert_eq!(result.choices[0].delta.content, Some("Hello".to_string()));
        assert_eq!(result.choices[0].delta.role, Some("assistant".to_string()));
    }

    #[test]
    fn test_convert_stream_chunk_with_finish_reason() {
        let input_chunk = ChatCompletionChunk {
            id: "chatcmpl-finish".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![ChatCompletionChunkChoice {
                index: 0,
                delta: ChatCompletionDelta {
                    role: None,
                    content: None,
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            system_fingerprint: None,
            usage: None,
        };

        let result = convert_stream_chunk(input_chunk);

        assert_eq!(result.choices[0].finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn test_convert_stream_chunk_multiple_choices() {
        let input_chunk = ChatCompletionChunk {
            id: "chatcmpl-multi".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1234567890,
            model: "gpt-4".to_string(),
            choices: vec![
                ChatCompletionChunkChoice {
                    index: 0,
                    delta: ChatCompletionDelta {
                        role: None,
                        content: Some("First".to_string()),
                        tool_calls: None,
                    },
                    finish_reason: None,
                    logprobs: None,
                },
                ChatCompletionChunkChoice {
                    index: 1,
                    delta: ChatCompletionDelta {
                        role: None,
                        content: Some("Second".to_string()),
                        tool_calls: None,
                    },
                    finish_reason: None,
                    logprobs: None,
                },
            ],
            system_fingerprint: None,
            usage: None,
        };

        let result = convert_stream_chunk(input_chunk);

        assert_eq!(result.choices.len(), 2);
        assert_eq!(result.choices[0].delta.content, Some("First".to_string()));
        assert_eq!(result.choices[1].delta.content, Some("Second".to_string()));
    }

    #[test]
    fn test_convert_stream_chunk_created_conversion() {
        // Test that u64 created is properly converted to i64
        let input_chunk = ChatCompletionChunk {
            id: "test".to_string(),
            object: "test".to_string(),
            created: u64::MAX / 2, // Large but safe value
            model: "test".to_string(),
            choices: vec![],
            system_fingerprint: None,
            usage: None,
        };

        let result = convert_stream_chunk(input_chunk);

        assert_eq!(result.created, (u64::MAX / 2) as i64);
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_streaming_sequence() {
        // Simulate a typical streaming sequence
        let chunks = vec![
            // First chunk with role
            ChatCompletionChunk {
                id: "chatcmpl-seq".to_string(),
                object: "chat.completion.chunk".to_string(),
                created: 1234567890,
                model: "gpt-4".to_string(),
                choices: vec![ChatCompletionChunkChoice {
                    index: 0,
                    delta: ChatCompletionDelta {
                        role: Some(crate::core::types::message::MessageRole::Assistant),
                        content: None,
                        tool_calls: None,
                    },
                    finish_reason: None,
                    logprobs: None,
                }],
                system_fingerprint: None,
                usage: None,
            },
            // Content chunk
            ChatCompletionChunk {
                id: "chatcmpl-seq".to_string(),
                object: "chat.completion.chunk".to_string(),
                created: 1234567890,
                model: "gpt-4".to_string(),
                choices: vec![ChatCompletionChunkChoice {
                    index: 0,
                    delta: ChatCompletionDelta {
                        role: None,
                        content: Some("Hello".to_string()),
                        tool_calls: None,
                    },
                    finish_reason: None,
                    logprobs: None,
                }],
                system_fingerprint: None,
                usage: None,
            },
            // Final chunk with finish reason
            ChatCompletionChunk {
                id: "chatcmpl-seq".to_string(),
                object: "chat.completion.chunk".to_string(),
                created: 1234567890,
                model: "gpt-4".to_string(),
                choices: vec![ChatCompletionChunkChoice {
                    index: 0,
                    delta: ChatCompletionDelta {
                        role: None,
                        content: None,
                        tool_calls: None,
                    },
                    finish_reason: Some("stop".to_string()),
                    logprobs: None,
                }],
                system_fingerprint: None,
                usage: None,
            },
        ];

        let results: Vec<_> = chunks.into_iter().map(convert_stream_chunk).collect();

        assert_eq!(results.len(), 3);
        assert!(results[0].choices[0].delta.role.is_some());
        assert_eq!(
            results[1].choices[0].delta.content,
            Some("Hello".to_string())
        );
        assert_eq!(
            results[2].choices[0].finish_reason,
            Some(FinishReason::Stop)
        );
    }
}
