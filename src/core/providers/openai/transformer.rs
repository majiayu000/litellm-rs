//! OpenAI Request and Response Transformers
//!
//! Unified transformation layer for converting between unified LiteLLM types and OpenAI-specific formats

use crate::core::traits::transformer::Transform;
use crate::core::types::responses::{
    ChatChoice, ChatChunk, ChatDelta, ChatResponse, ChatStreamChoice, FinishReason, LogProbs,
    TokenLogProb, TopLogProb, Usage,
};
use crate::core::types::thinking::ThinkingContent;
use crate::core::types::{
    chat::ChatMessage, chat::ChatRequest, content::ContentPart, message::MessageContent,
    message::MessageRole, tools::ResponseFormat, tools::Tool, tools::ToolChoice,
};
use serde_json;

use super::error::OpenAIError;
use super::models::*;

#[cfg(test)]
use crate::core::types::{content::ImageUrl, tools::FunctionCall, tools::ToolCall};

/// OpenAI Request Transformer
pub struct OpenAIRequestTransformer;

impl OpenAIRequestTransformer {
    /// Transform ChatRequest to OpenAIChatRequest
    pub fn transform(request: ChatRequest) -> Result<OpenAIChatRequest, OpenAIError> {
        let messages = request
            .messages
            .into_iter()
            .map(Self::transform_message)
            .collect::<Result<Vec<_>, _>>()?;

        let tools = request
            .tools
            .map(|tools| tools.into_iter().map(Self::transform_tool).collect());

        let tool_choice = request
            .tool_choice
            .map(Self::transform_tool_choice)
            .and_then(|tc| serde_json::to_value(tc).ok());

        let response_format = request.response_format.map(Self::transform_response_format);

        Ok(OpenAIChatRequest {
            model: request.model,
            messages,
            temperature: request.temperature,
            top_p: request.top_p,
            n: request.n,
            stream: None, // Set by caller
            stop: request.stop,
            max_tokens: request.max_tokens,
            max_completion_tokens: request.max_completion_tokens,
            presence_penalty: request.presence_penalty,
            frequency_penalty: request.frequency_penalty,
            logit_bias: request.logit_bias,
            logprobs: request.logprobs,
            top_logprobs: request.top_logprobs,
            user: request.user,
            tools,
            tool_choice,
            parallel_tool_calls: request.parallel_tool_calls,
            response_format,
            seed: request.seed,
        })
    }

    /// Transform Message
    fn transform_message(message: ChatMessage) -> Result<OpenAIMessage, OpenAIError> {
        if let Some(MessageContent::Parts(parts)) = message.content.as_ref() {
            for part in parts {
                match part {
                    ContentPart::Document { .. } => {
                        return Err(OpenAIError::InvalidRequest {
                            provider: "openai",
                            message: "Document content not supported by OpenAI".to_string(),
                        });
                    }
                    ContentPart::ToolResult { .. } => {
                        return Err(OpenAIError::InvalidRequest {
                            provider: "openai",
                            message: "ToolResult should be handled separately".to_string(),
                        });
                    }
                    ContentPart::ToolUse { .. } => {
                        return Err(OpenAIError::InvalidRequest {
                            provider: "openai",
                            message: "ToolUse should be handled separately".to_string(),
                        });
                    }
                    _ => {}
                }
            }
        }

        let compatible_message: crate::core::models::openai::ChatMessage = message.into();
        OpenAIMessage::from_compatible_message(compatible_message).map_err(|message| {
            OpenAIError::Serialization {
                provider: "openai",
                message,
            }
        })
    }

    /// Transform tool definition
    fn transform_tool(tool: Tool) -> OpenAITool {
        OpenAITool {
            tool_type: "function".to_string(),
            function: Some(OpenAIFunction {
                name: tool.function.name,
                description: tool.function.description,
                parameters: tool.function.parameters,
            }),
        }
    }

    /// Transform tool choice
    fn transform_tool_choice(choice: ToolChoice) -> OpenAIToolChoice {
        match choice {
            ToolChoice::String(s) => match s.as_str() {
                "none" => OpenAIToolChoice::none(),
                "auto" => OpenAIToolChoice::auto(),
                "required" => OpenAIToolChoice::required(),
                _ => OpenAIToolChoice::auto(),
            },
            ToolChoice::Specific {
                choice_type,
                function,
            } => {
                if choice_type == "function" {
                    if let Some(func) = function {
                        OpenAIToolChoice::Function {
                            r#type: "function".to_string(),
                            function: OpenAIFunctionChoice { name: func.name },
                        }
                    } else {
                        OpenAIToolChoice::auto()
                    }
                } else {
                    OpenAIToolChoice::auto()
                }
            }
        }
    }

    /// Transform response format
    fn transform_response_format(format: ResponseFormat) -> OpenAIResponseFormat {
        OpenAIResponseFormat {
            format_type: format.format_type,
            json_schema: format.json_schema,
        }
    }
}

/// OpenAI Response Transformer
pub struct OpenAIResponseTransformer;

impl OpenAIResponseTransformer {
    /// Transform OpenAIChatResponse to ChatResponse
    pub fn transform(response: OpenAIChatResponse) -> Result<ChatResponse, OpenAIError> {
        let choices = response
            .choices
            .into_iter()
            .map(Self::transform_choice)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ChatResponse {
            id: response.id,
            object: response.object,
            created: response.created,
            model: response.model,
            choices,
            usage: response.usage.map(Self::transform_usage),
            system_fingerprint: response.system_fingerprint,
        })
    }

    /// Transform stream chunk
    pub fn transform_stream_chunk(chunk: OpenAIStreamChunk) -> Result<ChatChunk, OpenAIError> {
        let choices = chunk
            .choices
            .into_iter()
            .map(Self::transform_stream_choice)
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ChatChunk {
            id: chunk.id,
            object: chunk.object,
            created: chunk.created,
            model: chunk.model,
            choices,
            usage: chunk.usage.map(Self::transform_usage),
            system_fingerprint: chunk.system_fingerprint,
        })
    }

    /// Transform choice
    fn transform_choice(choice: OpenAIChoice) -> Result<ChatChoice, OpenAIError> {
        Ok(ChatChoice {
            index: choice.index,
            message: Self::transform_message_response(choice.message)?,
            logprobs: choice.logprobs.and_then(|lp| {
                serde_json::from_value::<OpenAILogprobs>(lp)
                    .ok()
                    .map(Self::transform_logprobs)
            }),
            finish_reason: choice.finish_reason.map(Self::transform_finish_reason),
        })
    }

    /// Transform stream choice
    fn transform_stream_choice(
        choice: OpenAIStreamChoice,
    ) -> Result<ChatStreamChoice, OpenAIError> {
        Ok(ChatStreamChoice {
            index: choice.index,
            delta: Self::transform_delta(choice.delta)?,
            logprobs: choice.logprobs.and_then(|lp| {
                serde_json::from_value::<OpenAILogprobs>(lp)
                    .ok()
                    .map(Self::transform_logprobs)
            }),
            finish_reason: choice.finish_reason.map(Self::transform_finish_reason),
        })
    }

    /// Transform message response
    fn transform_message_response(message: OpenAIMessage) -> Result<ChatMessage, OpenAIError> {
        // Extract thinking content from reasoning fields
        // Priority: reasoning_content (DeepSeek) > reasoning (OpenAI)
        let thinking = message
            .reasoning_content
            .as_ref()
            .filter(|s| !s.is_empty())
            .or(message.reasoning.as_ref().filter(|s| !s.is_empty()))
            .map(|text| ThinkingContent::Text {
                text: text.clone(),
                signature: None,
            });
        let compatible_message =
            message
                .into_compatible_message()
                .map_err(|message| OpenAIError::ResponseParsing {
                    provider: "openai",
                    message,
                })?;
        let mut core_message: ChatMessage = compatible_message.into();
        core_message.thinking = thinking;
        Ok(core_message)
    }

    /// Transform delta
    fn transform_delta(delta: OpenAIDelta) -> Result<ChatDelta, OpenAIError> {
        Ok(ChatDelta {
            role: delta.role.map(|r| match r.as_str() {
                "system" => MessageRole::System,
                "user" => MessageRole::User,
                "assistant" => MessageRole::Assistant,
                "tool" => MessageRole::Tool,
                "function" => MessageRole::Function,
                _ => MessageRole::Assistant,
            }),
            content: delta.content,
            thinking: None,
            tool_calls: None,
            function_call: None,
        })
    }

    /// Transform usage
    fn transform_usage(usage: OpenAIUsage) -> Usage {
        Usage {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            thinking_usage: None,
            prompt_tokens_details: usage.prompt_tokens_details.map(|details| {
                crate::core::types::responses::PromptTokensDetails {
                    cached_tokens: details.cached_tokens,
                    audio_tokens: details.audio_tokens,
                }
            }),
            completion_tokens_details: usage.completion_tokens_details.map(|details| {
                crate::core::types::responses::CompletionTokensDetails {
                    reasoning_tokens: details.reasoning_tokens,
                    audio_tokens: details.audio_tokens,
                }
            }),
        }
    }

    /// Transform logprobs
    fn transform_logprobs(logprobs: OpenAILogprobs) -> LogProbs {
        LogProbs {
            content: logprobs
                .content
                .map(|content| {
                    content
                        .into_iter()
                        .map(|token| TokenLogProb {
                            token: token.token,
                            logprob: token.logprob,
                            bytes: token.bytes,
                            top_logprobs: Some(
                                token
                                    .top_logprobs
                                    .into_iter()
                                    .map(|top| TopLogProb {
                                        token: top.token,
                                        logprob: top.logprob,
                                        bytes: top.bytes,
                                    })
                                    .collect(),
                            ),
                        })
                        .collect()
                })
                .unwrap_or_default(),
            refusal: logprobs.refusal.map(|_| "filtered".to_string()),
        }
    }

    /// Transform finish reason
    fn transform_finish_reason(reason: String) -> FinishReason {
        match reason.as_str() {
            "stop" => FinishReason::Stop,
            "length" => FinishReason::Length,
            "function_call" => FinishReason::FunctionCall,
            "tool_calls" => FinishReason::ToolCalls,
            "content_filter" => FinishReason::ContentFilter,
            _ => FinishReason::Stop,
        }
    }
}

/// OpenAI Transformer (compatible with old interface)
pub struct OpenAITransformer;

impl Transform<ChatRequest, OpenAIChatRequest> for OpenAITransformer {
    type Error = OpenAIError;

    fn transform(input: ChatRequest) -> Result<OpenAIChatRequest, Self::Error> {
        OpenAIRequestTransformer::transform(input)
    }
}

impl Transform<OpenAIChatResponse, ChatResponse> for OpenAITransformer {
    type Error = OpenAIError;

    fn transform(input: OpenAIChatResponse) -> Result<ChatResponse, Self::Error> {
        OpenAIResponseTransformer::transform(input)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{
        content::AudioData, content::DocumentSource, content::ImageSource,
        tools::FunctionDefinition, tools::ToolType,
    };

    // ==================== Request Transformer Tests ====================

    #[test]
    fn test_transform_basic_request() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request);
        assert!(result.is_ok());

        let openai_request = result.unwrap();
        assert_eq!(openai_request.model, "gpt-4");
        assert_eq!(openai_request.messages.len(), 1);
        assert_eq!(openai_request.messages.first().unwrap().role, "user");
    }

    #[test]
    fn test_transform_request_with_temperature() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            temperature: Some(0.7),
            top_p: Some(0.9),
            max_tokens: Some(100),
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert_eq!(result.temperature, Some(0.7));
        assert_eq!(result.top_p, Some(0.9));
        assert_eq!(result.max_tokens, Some(100));
    }

    #[test]
    fn test_transform_message_roles() {
        let messages = vec![
            ChatMessage {
                role: MessageRole::System,
                content: Some(MessageContent::Text("You are helpful".to_string())),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::Assistant,
                content: Some(MessageContent::Text("Hi there".to_string())),
                ..Default::default()
            },
            ChatMessage {
                role: MessageRole::Tool,
                content: Some(MessageContent::Text("result".to_string())),
                tool_call_id: Some("tool-123".to_string()),
                ..Default::default()
            },
        ];

        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages,
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert_eq!(result.messages.first().unwrap().role, "system");
        assert_eq!(result.messages[1].role, "user");
        assert_eq!(result.messages[2].role, "assistant");
        assert_eq!(result.messages[3].role, "tool");
    }

    #[test]
    fn test_transform_content_parts_text() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Parts(vec![ContentPart::Text {
                    text: "Hello world".to_string(),
                }])),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert!(result.messages.first().unwrap().content.is_some());
    }

    #[test]
    fn test_transform_content_parts_image_url() {
        let request = ChatRequest {
            model: "gpt-4-vision".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Parts(vec![
                    ContentPart::Text {
                        text: "What's in this image?".to_string(),
                    },
                    ContentPart::ImageUrl {
                        image_url: ImageUrl {
                            url: "https://example.com/image.png".to_string(),
                            detail: Some("high".to_string()),
                        },
                    },
                ])),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert!(result.messages.first().unwrap().content.is_some());
    }

    #[test]
    fn test_transform_content_parts_audio() {
        let request = ChatRequest {
            model: "gpt-4o-audio".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Parts(vec![ContentPart::Audio {
                    audio: AudioData {
                        data: "base64data".to_string(),
                        format: Some("mp3".to_string()),
                    },
                }])),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert!(result.messages.first().unwrap().content.is_some());
    }

    #[test]
    fn test_transform_content_parts_image_source() {
        let request = ChatRequest {
            model: "gpt-4-vision".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Parts(vec![ContentPart::Image {
                    source: ImageSource {
                        media_type: "image/png".to_string(),
                        data: "base64imagedata".to_string(),
                    },
                    detail: Some("high".to_string()),
                    image_url: None,
                }])),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert!(result.messages.first().unwrap().content.is_some());
    }

    #[test]
    fn test_transform_document_content_error() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Parts(vec![ContentPart::Document {
                    source: DocumentSource {
                        media_type: "application/pdf".to_string(),
                        data: "base64pdfdata".to_string(),
                    },
                    cache_control: None,
                }])),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_tool_call() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::Assistant,
                content: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call_123".to_string(),
                    tool_type: "function".to_string(),
                    function: FunctionCall {
                        name: "get_weather".to_string(),
                        arguments: r#"{"location":"NYC"}"#.to_string(),
                    },
                }]),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        let tool_calls = result
            .messages
            .first()
            .unwrap()
            .tool_calls
            .as_ref()
            .unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls.first().unwrap().id, "call_123");
        assert_eq!(tool_calls.first().unwrap().function.name, "get_weather");
    }

    #[test]
    fn test_transform_tools() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            tools: Some(vec![Tool {
                tool_type: ToolType::Function,
                function: FunctionDefinition {
                    name: "get_weather".to_string(),
                    description: Some("Get weather info".to_string()),
                    parameters: Some(serde_json::json!({
                        "type": "object",
                        "properties": {
                            "location": {"type": "string"}
                        }
                    })),
                },
            }]),
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        let tools = result.tools.unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(
            tools.first().unwrap().function.as_ref().unwrap().name,
            "get_weather"
        );
    }

    #[test]
    fn test_transform_tool_choice_none() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            tool_choice: Some(ToolChoice::String("none".to_string())),
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert!(result.tool_choice.is_some());
    }

    #[test]
    fn test_transform_tool_choice_auto() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            tool_choice: Some(ToolChoice::String("auto".to_string())),
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert!(result.tool_choice.is_some());
    }

    #[test]
    fn test_transform_tool_choice_required() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            tool_choice: Some(ToolChoice::String("required".to_string())),
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert!(result.tool_choice.is_some());
    }

    #[test]
    fn test_transform_tool_choice_specific() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            tool_choice: Some(ToolChoice::Specific {
                choice_type: "function".to_string(),
                function: Some(crate::core::types::tools::FunctionChoice {
                    name: "get_weather".to_string(),
                }),
            }),
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert!(result.tool_choice.is_some());
    }

    #[test]
    fn test_transform_response_format() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
                json_schema: None,
                response_type: None,
            }),
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        let format = result.response_format.unwrap();
        assert_eq!(format.format_type, "json_object");
    }

    #[test]
    fn test_transform_response_format_with_schema() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            response_format: Some(ResponseFormat {
                format_type: "json_schema".to_string(),
                json_schema: Some(serde_json::json!({
                    "name": "response",
                    "schema": {"type": "object"}
                })),
                response_type: None,
            }),
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        let format = result.response_format.unwrap();
        assert_eq!(format.format_type, "json_schema");
        assert!(format.json_schema.is_some());
    }

    // ==================== Response Transformer Tests ====================

    #[test]
    fn test_transform_basic_response() {
        let response = OpenAIChatResponse {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1677652288,
            model: "gpt-4".to_string(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: Some(serde_json::json!("Hello!")),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                    reasoning: None,
                    reasoning_details: None,
                    reasoning_content: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: Some(OpenAIUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            }),
            system_fingerprint: Some("fp_123".to_string()),
        };

        let result = OpenAIResponseTransformer::transform(response).unwrap();
        assert_eq!(result.id, "chatcmpl-123");
        assert_eq!(result.model, "gpt-4");
        assert_eq!(result.choices.len(), 1);
        assert!(matches!(
            result.choices.first().unwrap().finish_reason,
            Some(FinishReason::Stop)
        ));
    }

    #[test]
    fn test_transform_response_with_usage_details() {
        let response = OpenAIChatResponse {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion".to_string(),
            created: 1677652288,
            model: "gpt-4".to_string(),
            choices: vec![],
            usage: Some(OpenAIUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                prompt_tokens_details: Some(OpenAITokenDetails {
                    cached_tokens: Some(20),
                    audio_tokens: Some(5),
                    reasoning_tokens: None,
                }),
                completion_tokens_details: Some(OpenAITokenDetails {
                    cached_tokens: None,
                    audio_tokens: Some(10),
                    reasoning_tokens: Some(15),
                }),
            }),
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform(response).unwrap();
        let usage = result.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(
            usage.prompt_tokens_details.as_ref().unwrap().cached_tokens,
            Some(20)
        );
        assert_eq!(
            usage
                .completion_tokens_details
                .as_ref()
                .unwrap()
                .reasoning_tokens,
            Some(15)
        );
    }

    #[test]
    fn test_transform_response_role_mapping() {
        let roles = vec!["system", "user", "assistant", "tool", "function", "unknown"];

        for role in roles {
            let response = OpenAIChatResponse {
                id: "test".to_string(),
                object: "chat.completion".to_string(),
                created: 0,
                model: "gpt-4".to_string(),
                choices: vec![OpenAIChoice {
                    index: 0,
                    message: OpenAIMessage {
                        role: role.to_string(),
                        content: Some(serde_json::json!("test")),
                        name: None,
                        tool_calls: None,
                        tool_call_id: None,
                        function_call: None,
                        reasoning: None,
                        reasoning_details: None,
                        reasoning_content: None,
                    },
                    finish_reason: None,
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            };

            let result = OpenAIResponseTransformer::transform(response);
            assert!(result.is_ok());
        }
    }

    #[test]
    fn test_transform_finish_reasons() {
        let reasons = vec![
            ("stop", FinishReason::Stop),
            ("length", FinishReason::Length),
            ("function_call", FinishReason::FunctionCall),
            ("tool_calls", FinishReason::ToolCalls),
            ("content_filter", FinishReason::ContentFilter),
            ("unknown", FinishReason::Stop), // Default fallback
        ];

        for (reason_str, expected) in reasons {
            let response = OpenAIChatResponse {
                id: "test".to_string(),
                object: "chat.completion".to_string(),
                created: 0,
                model: "gpt-4".to_string(),
                choices: vec![OpenAIChoice {
                    index: 0,
                    message: OpenAIMessage {
                        role: "assistant".to_string(),
                        content: None,
                        name: None,
                        tool_calls: None,
                        tool_call_id: None,
                        function_call: None,
                        reasoning: None,
                        reasoning_details: None,
                        reasoning_content: None,
                    },
                    finish_reason: Some(reason_str.to_string()),
                    logprobs: None,
                }],
                usage: None,
                system_fingerprint: None,
            };

            let result = OpenAIResponseTransformer::transform(response).unwrap();
            assert_eq!(
                result.choices.first().unwrap().finish_reason,
                Some(expected)
            );
        }
    }

    #[test]
    fn test_transform_response_with_tool_calls() {
        let response = OpenAIChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_calls: Some(vec![OpenAIToolCall {
                        id: "call_abc".to_string(),
                        tool_type: "function".to_string(),
                        function: OpenAIFunctionCall {
                            name: "get_weather".to_string(),
                            arguments: r#"{"location":"NYC"}"#.to_string(),
                        },
                    }]),
                    tool_call_id: None,
                    function_call: None,
                    reasoning: None,
                    reasoning_details: None,
                    reasoning_content: None,
                },
                finish_reason: Some("tool_calls".to_string()),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform(response).unwrap();
        let tool_calls = result
            .choices
            .first()
            .unwrap()
            .message
            .tool_calls
            .as_ref()
            .unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls.first().unwrap().id, "call_abc");
        assert_eq!(tool_calls.first().unwrap().function.name, "get_weather");
    }

    #[test]
    fn test_transform_response_with_reasoning() {
        let response = OpenAIChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "o1-preview".to_string(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: Some(serde_json::json!("The answer is 42")),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                    reasoning: Some("Let me think about this...".to_string()),
                    reasoning_details: None,
                    reasoning_content: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform(response).unwrap();
        assert!(result.choices.first().unwrap().message.thinking.is_some());
    }

    #[test]
    fn test_transform_response_with_deepseek_reasoning() {
        let response = OpenAIChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "deepseek-chat".to_string(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: Some(serde_json::json!("Result")),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                    reasoning: None,
                    reasoning_details: None,
                    reasoning_content: Some("DeepSeek thinking process...".to_string()),
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform(response).unwrap();
        assert!(result.choices.first().unwrap().message.thinking.is_some());
    }

    #[test]
    fn test_transform_response_null_content() {
        let response = OpenAIChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: Some(serde_json::Value::Null),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                    reasoning: None,
                    reasoning_details: None,
                    reasoning_content: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform(response).unwrap();
        assert!(result.choices.first().unwrap().message.content.is_none());
    }

    #[test]
    fn test_transform_response_empty_content() {
        let response = OpenAIChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: Some(serde_json::json!("")),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                    reasoning: None,
                    reasoning_details: None,
                    reasoning_content: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform(response).unwrap();
        assert!(result.choices.first().unwrap().message.content.is_none());
    }

    // ==================== Stream Transformer Tests ====================

    #[test]
    fn test_transform_stream_chunk() {
        let chunk = OpenAIStreamChunk {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1677652288,
            model: "gpt-4".to_string(),
            choices: vec![OpenAIStreamChoice {
                index: 0,
                delta: OpenAIDelta {
                    role: Some("assistant".to_string()),
                    content: Some("Hello".to_string()),
                    tool_calls: None,
                    function_call: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform_stream_chunk(chunk).unwrap();
        assert_eq!(result.id, "chatcmpl-123");
        assert_eq!(result.choices.len(), 1);
        assert_eq!(
            result.choices.first().unwrap().delta.content,
            Some("Hello".to_string())
        );
    }

    #[test]
    fn test_transform_stream_chunk_with_finish() {
        let chunk = OpenAIStreamChunk {
            id: "chatcmpl-123".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: 1677652288,
            model: "gpt-4".to_string(),
            choices: vec![OpenAIStreamChoice {
                index: 0,
                delta: OpenAIDelta {
                    role: None,
                    content: None,
                    tool_calls: None,
                    function_call: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: None,
            }],
            usage: Some(OpenAIUsage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
                prompt_tokens_details: None,
                completion_tokens_details: None,
            }),
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform_stream_chunk(chunk).unwrap();
        assert!(matches!(
            result.choices.first().unwrap().finish_reason,
            Some(FinishReason::Stop)
        ));
        assert!(result.usage.is_some());
    }

    #[test]
    fn test_transform_delta_roles() {
        let roles = vec!["system", "user", "assistant", "tool", "function", "unknown"];

        for role in roles {
            let delta = OpenAIDelta {
                role: Some(role.to_string()),
                content: None,
                tool_calls: None,
                function_call: None,
            };

            let result = OpenAIResponseTransformer::transform_delta(delta);
            assert!(result.is_ok());
        }
    }

    // ==================== Trait Implementation Tests ====================

    #[test]
    fn test_transform_trait_request() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            ..Default::default()
        };

        let result =
            <OpenAITransformer as Transform<ChatRequest, OpenAIChatRequest>>::transform(request);
        assert!(result.is_ok());
    }

    #[test]
    fn test_transform_trait_response() {
        let response = OpenAIChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![],
            usage: None,
            system_fingerprint: None,
        };

        let result =
            <OpenAITransformer as Transform<OpenAIChatResponse, ChatResponse>>::transform(response);
        assert!(result.is_ok());
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_transform_request_with_all_optional_fields() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("test".to_string())),
                name: Some("test_user".to_string()),
                ..Default::default()
            }],
            temperature: Some(0.5),
            top_p: Some(0.9),
            n: Some(2),
            stop: Some(vec!["END".to_string()]),
            max_tokens: Some(500),
            max_completion_tokens: Some(400),
            presence_penalty: Some(0.5),
            frequency_penalty: Some(0.3),
            logprobs: Some(true),
            top_logprobs: Some(5),
            user: Some("user123".to_string()),
            seed: Some(42),
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert_eq!(result.temperature, Some(0.5));
        assert_eq!(result.n, Some(2));
        assert_eq!(result.seed, Some(42));
        assert_eq!(result.user, Some("user123".to_string()));
    }

    #[test]
    fn test_transform_response_with_logprobs() {
        let response = OpenAIChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: Some(serde_json::json!("test")),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                    reasoning: None,
                    reasoning_details: None,
                    reasoning_content: None,
                },
                finish_reason: Some("stop".to_string()),
                logprobs: Some(serde_json::json!({
                    "content": [{
                        "token": "test",
                        "logprob": -0.5,
                        "bytes": [116, 101, 115, 116],
                        "top_logprobs": [{
                            "token": "test",
                            "logprob": -0.5,
                            "bytes": [116, 101, 115, 116]
                        }]
                    }]
                })),
            }],
            usage: None,
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform(response).unwrap();
        assert!(result.choices.first().unwrap().logprobs.is_some());
    }

    #[test]
    fn test_transform_response_content_array() {
        let response = OpenAIChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: Some(serde_json::json!([
                        {"type": "text", "text": "Hello"}
                    ])),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                    reasoning: None,
                    reasoning_details: None,
                    reasoning_content: None,
                },
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform(response).unwrap();
        assert!(result.choices.first().unwrap().message.content.is_some());
    }

    #[test]
    fn test_transform_message_with_function_call() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::Assistant,
                content: None,
                function_call: Some(FunctionCall {
                    name: "get_weather".to_string(),
                    arguments: r#"{"location":"NYC"}"#.to_string(),
                }),
                ..Default::default()
            }],
            ..Default::default()
        };

        let result = OpenAIRequestTransformer::transform(request).unwrap();
        assert!(result.messages.first().unwrap().function_call.is_some());
    }

    #[test]
    fn test_transform_response_with_function_call() {
        let response = OpenAIChatResponse {
            id: "test".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "gpt-4".to_string(),
            choices: vec![OpenAIChoice {
                index: 0,
                message: OpenAIMessage {
                    role: "assistant".to_string(),
                    content: None,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: Some(OpenAIFunctionCall {
                        name: "get_weather".to_string(),
                        arguments: r#"{"location":"NYC"}"#.to_string(),
                    }),
                    reasoning: None,
                    reasoning_details: None,
                    reasoning_content: None,
                },
                finish_reason: Some("function_call".to_string()),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };

        let result = OpenAIResponseTransformer::transform(response).unwrap();
        let func_call = result
            .choices
            .first()
            .unwrap()
            .message
            .function_call
            .as_ref()
            .unwrap();
        assert_eq!(func_call.name, "get_weather");
    }
}
