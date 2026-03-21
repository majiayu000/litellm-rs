//! OpenAI Request Transformer
//!
//! Converts unified ChatRequest types into OpenAI-specific request formats.

use crate::core::types::{
    chat::ChatMessage, chat::ChatRequest, content::ContentPart, message::MessageContent,
    tools::ResponseFormat, tools::Tool, tools::ToolChoice,
};
use serde_json;

use super::super::error::OpenAIError;
use super::super::models::*;

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
            reasoning_effort: request.reasoning_effort,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{
        content::AudioData, content::DocumentSource, content::ImageSource, message::MessageRole,
        tools::FunctionDefinition, tools::ToolType,
    };

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
}
