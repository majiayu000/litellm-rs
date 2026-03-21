//! OpenAI API Request/Response Types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OpenAI Chat Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIChatRequest {
    pub model: String,
    pub messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parallel_tool_calls: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<OpenAIResponseFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<String, f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<u32>,
    /// Reasoning effort for o-series and GPT-5.x models ("low", "medium", "high")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

/// OpenAI Message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<OpenAIFunctionCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_details: Option<serde_json::Value>,
    /// DeepSeek reasoning content field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
}

impl OpenAIMessage {
    /// Convert from canonical OpenAI-compatible message model.
    pub fn from_compatible_message(
        message: crate::core::models::openai::ChatMessage,
    ) -> Result<Self, String> {
        let role = match message.role {
            crate::core::models::openai::MessageRole::System => "system",
            crate::core::models::openai::MessageRole::Developer => "developer",
            crate::core::models::openai::MessageRole::User => "user",
            crate::core::models::openai::MessageRole::Assistant => "assistant",
            crate::core::models::openai::MessageRole::Tool => "tool",
            crate::core::models::openai::MessageRole::Function => "function",
        }
        .to_string();

        let content = match message.content {
            Some(crate::core::models::openai::MessageContent::Text(text)) => {
                Some(serde_json::json!(text))
            }
            Some(crate::core::models::openai::MessageContent::Parts(parts)) => {
                let provider_parts: Vec<OpenAIContentPart> = parts
                    .into_iter()
                    .map(OpenAIContentPart::from_compatible_part)
                    .collect();
                Some(
                    serde_json::to_value(provider_parts)
                        .map_err(|e| format!("Failed to serialize OpenAI content parts: {}", e))?,
                )
            }
            None => None,
        };

        Ok(Self {
            role,
            content,
            name: message.name,
            tool_calls: message.tool_calls.map(|calls| {
                calls
                    .into_iter()
                    .map(|call| OpenAIToolCall {
                        id: call.id,
                        tool_type: call.tool_type,
                        function: OpenAIFunctionCall {
                            name: call.function.name,
                            arguments: call.function.arguments,
                        },
                    })
                    .collect()
            }),
            tool_call_id: message.tool_call_id,
            function_call: message.function_call.map(|call| OpenAIFunctionCall {
                name: call.name,
                arguments: call.arguments,
            }),
            reasoning: None,
            reasoning_details: None,
            reasoning_content: None,
        })
    }

    /// Convert to canonical OpenAI-compatible message model.
    pub fn into_compatible_message(
        self,
    ) -> Result<crate::core::models::openai::ChatMessage, String> {
        let role = match self.role.as_str() {
            "system" => crate::core::models::openai::MessageRole::System,
            "user" => crate::core::models::openai::MessageRole::User,
            "assistant" => crate::core::models::openai::MessageRole::Assistant,
            "tool" => crate::core::models::openai::MessageRole::Tool,
            "function" => crate::core::models::openai::MessageRole::Function,
            _ => crate::core::models::openai::MessageRole::User,
        };

        let content = match self.content {
            Some(value) if value.is_null() => None,
            Some(value) => {
                if let Some(text) = value.as_str() {
                    if text.is_empty() {
                        None
                    } else {
                        Some(crate::core::models::openai::MessageContent::Text(
                            text.to_string(),
                        ))
                    }
                } else if let Some(array) = value.as_array() {
                    let parts: Vec<OpenAIContentPart> =
                        serde_json::from_value(serde_json::Value::Array(array.clone()))
                            .map_err(|e| format!("Failed to parse OpenAI content parts: {}", e))?;
                    Some(crate::core::models::openai::MessageContent::Parts(
                        parts
                            .into_iter()
                            .map(OpenAIContentPart::into_compatible_part)
                            .collect(),
                    ))
                } else {
                    None
                }
            }
            None => None,
        };

        Ok(crate::core::models::openai::ChatMessage {
            role,
            content,
            name: self.name,
            function_call: self.function_call.map(|call| {
                crate::core::models::openai::FunctionCall {
                    name: call.name,
                    arguments: call.arguments,
                }
            }),
            tool_calls: self.tool_calls.map(|calls| {
                calls
                    .into_iter()
                    .map(|call| crate::core::models::openai::ToolCall {
                        id: call.id,
                        tool_type: call.tool_type,
                        function: crate::core::models::openai::FunctionCall {
                            name: call.function.name,
                            arguments: call.function.arguments,
                        },
                    })
                    .collect()
            }),
            tool_call_id: self.tool_call_id,
            audio: None,
        })
    }
}

/// OpenAI Tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAITool {
    #[serde(rename = "type")]
    pub tool_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<OpenAIFunction>,
}

/// OpenAI Function
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIFunction {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// OpenAI Tool Call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAIFunctionCall,
}

/// OpenAI Function Call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIFunctionCall {
    pub name: String,
    pub arguments: String,
}

/// OpenAI Response Format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIResponseFormat {
    #[serde(rename = "type")]
    pub format_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_schema: Option<serde_json::Value>,
}

/// OpenAI Chat Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAIUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

/// OpenAI Choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIChoice {
    pub index: u32,
    pub message: OpenAIMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

/// OpenAI Usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<OpenAITokenDetails>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<OpenAITokenDetails>,
}

/// OpenAI Token Details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAITokenDetails {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_tokens: Option<u32>,
}

/// OpenAI Stream Chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIStreamChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIStreamChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAIUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

/// OpenAI Stream Choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIStreamChoice {
    pub index: u32,
    pub delta: OpenAIDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
}

/// OpenAI Delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OpenAIToolCallDelta>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_call: Option<OpenAIFunctionCallDelta>,
}

/// OpenAI Tool Call Delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIToolCallDelta {
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "type")]
    pub tool_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<OpenAIFunctionCallDelta>,
}

/// OpenAI Function Call Delta
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIFunctionCallDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// OpenAI Content Part
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OpenAIContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: OpenAIImageUrl },
    #[serde(rename = "input_audio")]
    InputAudio { input_audio: OpenAIInputAudio },
    #[serde(rename = "image")]
    Image {
        source: OpenAIImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        image_url: Option<OpenAIImageUrl>,
    },
    #[serde(rename = "document")]
    Document {
        source: OpenAIDocumentSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<OpenAICacheControl>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: serde_json::Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

impl OpenAIContentPart {
    fn from_compatible_part(part: crate::core::models::openai::ContentPart) -> Self {
        match part {
            crate::core::models::openai::ContentPart::Text { text } => Self::Text { text },
            crate::core::models::openai::ContentPart::ImageUrl { image_url } => Self::ImageUrl {
                image_url: OpenAIImageUrl {
                    url: image_url.url,
                    detail: image_url.detail,
                },
            },
            crate::core::models::openai::ContentPart::Audio { audio } => Self::InputAudio {
                input_audio: OpenAIInputAudio {
                    data: audio.data,
                    format: audio.format,
                },
            },
            crate::core::models::openai::ContentPart::Image {
                source,
                detail,
                image_url,
            } => Self::Image {
                source: OpenAIImageSource {
                    media_type: source.media_type,
                    data: source.data,
                },
                detail,
                image_url: image_url.map(|url| OpenAIImageUrl {
                    url: url.url,
                    detail: url.detail,
                }),
            },
            crate::core::models::openai::ContentPart::Document {
                source,
                cache_control,
            } => Self::Document {
                source: OpenAIDocumentSource {
                    media_type: source.media_type,
                    data: source.data,
                },
                cache_control: cache_control.map(|cc| OpenAICacheControl {
                    cache_type: cc.cache_type,
                }),
            },
            crate::core::models::openai::ContentPart::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => Self::ToolResult {
                tool_use_id,
                content,
                is_error,
            },
            crate::core::models::openai::ContentPart::ToolUse { id, name, input } => {
                Self::ToolUse { id, name, input }
            }
        }
    }

    fn into_compatible_part(self) -> crate::core::models::openai::ContentPart {
        match self {
            Self::Text { text } => crate::core::models::openai::ContentPart::Text { text },
            Self::ImageUrl { image_url } => crate::core::models::openai::ContentPart::ImageUrl {
                image_url: crate::core::models::openai::ImageUrl {
                    url: image_url.url,
                    detail: image_url.detail,
                },
            },
            Self::InputAudio { input_audio } => crate::core::models::openai::ContentPart::Audio {
                audio: crate::core::models::openai::AudioContent {
                    data: input_audio.data,
                    format: input_audio.format,
                },
            },
            Self::Image {
                source,
                detail,
                image_url,
            } => crate::core::models::openai::ContentPart::Image {
                source: crate::core::models::openai::ImageSource {
                    media_type: source.media_type,
                    data: source.data,
                },
                detail,
                image_url: image_url.map(|url| crate::core::models::openai::ImageUrl {
                    url: url.url,
                    detail: url.detail,
                }),
            },
            Self::Document {
                source,
                cache_control,
            } => crate::core::models::openai::ContentPart::Document {
                source: crate::core::models::openai::DocumentSource {
                    media_type: source.media_type,
                    data: source.data,
                },
                cache_control: cache_control.map(|cc| crate::core::models::openai::CacheControl {
                    cache_type: cc.cache_type,
                }),
            },
            Self::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => crate::core::models::openai::ContentPart::ToolResult {
                tool_use_id,
                content,
                is_error,
            },
            Self::ToolUse { id, name, input } => {
                crate::core::models::openai::ContentPart::ToolUse { id, name, input }
            }
        }
    }
}

/// OpenAI Image URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// OpenAI Input Audio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIInputAudio {
    pub data: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIImageSource {
    pub media_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIDocumentSource {
    pub media_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAICacheControl {
    #[serde(rename = "type")]
    pub cache_type: String,
}

/// OpenAI Tool Choice
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OpenAIToolChoice {
    String(String), // "none", "auto", "required"
    Function {
        #[serde(rename = "type")]
        r#type: String,
        function: OpenAIFunctionChoice,
    },
}

impl OpenAIToolChoice {
    pub fn none() -> Self {
        Self::String("none".to_string())
    }

    pub fn auto() -> Self {
        Self::String("auto".to_string())
    }

    pub fn required() -> Self {
        Self::String("required".to_string())
    }
}

/// OpenAI Function Choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIFunctionChoice {
    pub name: String,
}

/// OpenAI Logprobs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAILogprobs {
    pub content: Option<Vec<OpenAITokenLogprob>>,
    pub refusal: Option<serde_json::Value>,
}

/// OpenAI Token Logprob
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAITokenLogprob {
    pub token: String,
    pub logprob: f64,
    pub bytes: Option<Vec<u8>>,
    pub top_logprobs: Vec<OpenAITopLogprob>,
}

/// OpenAI Top Logprob
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAITopLogprob {
    pub token: String,
    pub logprob: f64,
    pub bytes: Option<Vec<u8>>,
}
