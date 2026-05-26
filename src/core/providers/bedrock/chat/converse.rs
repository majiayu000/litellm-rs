//! Converse API Implementation
//!
//! Modern unified API for chat completions in Bedrock

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use crate::core::types::{message::MessageContent, message::MessageRole};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Converse API request format
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConverseRequest {
    pub messages: Vec<ConverseMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Vec<SystemMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inference_config: Option<InferenceConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<ToolConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardrail_config: Option<GuardrailConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_model_request_fields: Option<Value>,
}

/// Converse message format
#[derive(Debug, Serialize, Deserialize)]
pub struct ConverseMessage {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

/// System message format
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guardrail_content: Option<GuardrailContent>,
}

/// Content block for messages
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        image: ImageBlock,
    },
    Document {
        document: DocumentBlock,
    },
    ToolUse {
        #[serde(rename = "toolUse")]
        tool_use: ToolUseBlock,
    },
    ToolResult {
        #[serde(rename = "toolResult")]
        tool_result: ToolResultBlock,
    },
    GuardrailContent {
        #[serde(rename = "guardrailContent")]
        guardrail_content: GuardrailContent,
    },
}

/// Image block for multimodal input
#[derive(Debug, Serialize, Deserialize)]
pub struct ImageBlock {
    pub format: String,
    pub source: ImageSource,
}

/// Image source
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ImageSource {
    Bytes { bytes: String },
}

/// Document block for document input
#[derive(Debug, Serialize, Deserialize)]
pub struct DocumentBlock {
    pub format: String,
    pub name: String,
    pub source: DocumentSource,
}

/// Document source
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum DocumentSource {
    Bytes { bytes: String },
}

/// Tool use block
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolUseBlock {
    pub tool_use_id: String,
    pub name: String,
    pub input: Value,
}

/// Tool result block
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResultBlock {
    pub tool_use_id: String,
    pub content: Vec<ToolResultContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

/// Tool result content
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum ToolResultContent {
    Text { text: String },
    Image { image: ImageBlock },
    Document { document: DocumentBlock },
}

/// Guardrail content
#[derive(Debug, Serialize, Deserialize)]
pub struct GuardrailContent {
    pub text: String,
}

/// Inference configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
}

/// Tool configuration
#[derive(Debug, Serialize, Deserialize)]
pub struct ToolConfig {
    pub tools: Vec<ToolSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
}

/// Tool specification
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpec {
    pub tool_spec: ToolSpecDefinition,
}

/// Tool specification definition
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpecDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: InputSchema,
}

/// Input schema for tools
#[derive(Debug, Serialize, Deserialize)]
pub struct InputSchema {
    pub json: Value,
}

/// Tool choice
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ToolChoice {
    Auto,
    Any,
    Tool { name: String },
}

/// Guardrail configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardrailConfig {
    pub guardrail_identifier: String,
    pub guardrail_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace: Option<bool>,
}

/// Execute a converse API request
pub async fn execute_converse(
    client: &crate::core::providers::bedrock::client::BedrockClient,
    request: &ChatRequest,
) -> Result<Value, ProviderError> {
    // Transform ChatRequest to ConverseRequest
    let converse_request = transform_to_converse(request)?;

    // Send request using the client
    let response = client
        .send_request(
            &request.model,
            "converse",
            &serde_json::to_value(converse_request)?,
        )
        .await?;

    // Parse response and return as Value
    response
        .json::<Value>()
        .await
        .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))
}

/// Transform OpenAI-style ChatRequest to Converse API format
pub(in crate::core::providers::bedrock) fn transform_to_converse(
    request: &ChatRequest,
) -> Result<ConverseRequest, ProviderError> {
    let mut messages = Vec::new();
    let mut system_messages = Vec::new();

    for msg in &request.messages {
        match msg.role {
            MessageRole::System => {
                // Extract system message
                if let Some(content) = &msg.content {
                    let text = match content {
                        MessageContent::Text(text) => text.clone(),
                        MessageContent::Parts(parts) => {
                            // Extract text from parts
                            parts
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
                                .join(" ")
                        }
                    };
                    system_messages.push(SystemMessage {
                        text: Some(text),
                        guardrail_content: None,
                    });
                }
            }
            MessageRole::Tool | MessageRole::Function => {
                let tool_use_id = msg.tool_call_id.clone().ok_or_else(|| {
                    ProviderError::invalid_request(
                        "bedrock",
                        "Tool/function message missing tool_call_id",
                    )
                })?;
                messages.push(ConverseMessage {
                    role: "user".to_string(),
                    content: vec![ContentBlock::ToolResult {
                        tool_result: ToolResultBlock {
                            tool_use_id,
                            content: message_content_to_tool_result_contents(msg.content.as_ref())?,
                            status: None,
                        },
                    }],
                });
            }
            MessageRole::User | MessageRole::Assistant => {
                // Transform to converse message
                let role = match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    _ => continue,
                }
                .to_string();

                let mut content = if let Some(msg_content) = &msg.content {
                    match msg_content {
                        MessageContent::Text(text) => {
                            vec![ContentBlock::Text { text: text.clone() }]
                        }
                        MessageContent::Parts(parts) => content_parts_to_blocks(parts)?,
                    }
                } else {
                    vec![]
                };

                if msg.role == MessageRole::Assistant
                    && let Some(tool_calls) = &msg.tool_calls
                {
                    for tool_call in tool_calls {
                        content.push(ContentBlock::ToolUse {
                            tool_use: ToolUseBlock {
                                tool_use_id: tool_call.id.clone(),
                                name: tool_call.function.name.clone(),
                                input: serde_json::from_str::<Value>(&tool_call.function.arguments)
                                    .unwrap_or(Value::Object(Default::default())),
                            },
                        });
                    }
                }

                messages.push(ConverseMessage { role, content });
            }
            MessageRole::Developer => {}
        }
    }

    // Build inference config
    let inference_config = Some(InferenceConfig {
        max_tokens: request.max_tokens,
        temperature: request.temperature.map(|t| t as f64),
        top_p: request.top_p.map(|t| t as f64),
        stop_sequences: request.stop.clone(),
    });

    // Build tool config if tools are present
    let tool_config = if let Some(tools) = &request.tools {
        let tool_specs: Vec<ToolSpec> = tools
            .iter()
            .map(|tool| ToolSpec {
                tool_spec: ToolSpecDefinition {
                    name: tool.function.name.clone(),
                    description: tool.function.description.clone().unwrap_or_default(),
                    input_schema: InputSchema {
                        json: tool
                            .function
                            .parameters
                            .clone()
                            .unwrap_or(Value::Object(Default::default())),
                    },
                },
            })
            .collect();

        Some(ToolConfig {
            tools: tool_specs,
            tool_choice: None, // NOTE: tool_choice mapping not yet implemented
        })
    } else {
        None
    };

    Ok(ConverseRequest {
        messages,
        system: if system_messages.is_empty() {
            None
        } else {
            Some(system_messages)
        },
        inference_config,
        tool_config,
        guardrail_config: None, // NOTE: guardrail support not yet implemented
        additional_model_request_fields: None,
    })
}

fn content_parts_to_blocks(
    parts: &[crate::core::types::content::ContentPart],
) -> Result<Vec<ContentBlock>, ProviderError> {
    parts.iter().map(content_part_to_block).collect()
}

fn content_part_to_block(
    part: &crate::core::types::content::ContentPart,
) -> Result<ContentBlock, ProviderError> {
    match part {
        crate::core::types::content::ContentPart::Text { text } => {
            Ok(ContentBlock::Text { text: text.clone() })
        }
        crate::core::types::content::ContentPart::ToolUse { id, name, input } => {
            Ok(ContentBlock::ToolUse {
                tool_use: ToolUseBlock {
                    tool_use_id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                },
            })
        }
        crate::core::types::content::ContentPart::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => Ok(ContentBlock::ToolResult {
            tool_result: ToolResultBlock {
                tool_use_id: tool_use_id.clone(),
                content: tool_result_contents_from_value(content)?,
                status: is_error.unwrap_or(false).then(|| "error".to_string()),
            },
        }),
        crate::core::types::content::ContentPart::Image { .. }
        | crate::core::types::content::ContentPart::ImageUrl { .. } => Err(
            ProviderError::not_implemented("bedrock", "Converse image content parts"),
        ),
        crate::core::types::content::ContentPart::Audio { .. } => Err(
            ProviderError::not_implemented("bedrock", "Converse audio content parts"),
        ),
        crate::core::types::content::ContentPart::Document { .. } => Err(
            ProviderError::not_implemented("bedrock", "Converse document content parts"),
        ),
    }
}

fn message_content_to_tool_result_contents(
    content: Option<&MessageContent>,
) -> Result<Vec<ToolResultContent>, ProviderError> {
    match content {
        Some(MessageContent::Text(text)) => {
            Ok(vec![ToolResultContent::Text { text: text.clone() }])
        }
        Some(MessageContent::Parts(parts)) => {
            let mut result = Vec::new();
            for part in parts {
                match part {
                    crate::core::types::content::ContentPart::Text { text } => {
                        result.push(ToolResultContent::Text { text: text.clone() });
                    }
                    crate::core::types::content::ContentPart::ToolResult { content, .. } => {
                        result.extend(tool_result_contents_from_value(content)?);
                    }
                    crate::core::types::content::ContentPart::Image { .. }
                    | crate::core::types::content::ContentPart::ImageUrl { .. } => {
                        return Err(ProviderError::not_implemented(
                            "bedrock",
                            "Converse tool-result image content",
                        ));
                    }
                    crate::core::types::content::ContentPart::Audio { .. } => {
                        return Err(ProviderError::not_implemented(
                            "bedrock",
                            "Converse tool-result audio content",
                        ));
                    }
                    crate::core::types::content::ContentPart::Document { .. } => {
                        return Err(ProviderError::not_implemented(
                            "bedrock",
                            "Converse tool-result document content",
                        ));
                    }
                    crate::core::types::content::ContentPart::ToolUse { .. } => {
                        return Err(ProviderError::invalid_request(
                            "bedrock",
                            "Tool result message cannot contain tool_use content",
                        ));
                    }
                }
            }
            Ok(result)
        }
        None => Ok(vec![ToolResultContent::Text {
            text: String::new(),
        }]),
    }
}

fn tool_result_contents_from_value(value: &Value) -> Result<Vec<ToolResultContent>, ProviderError> {
    if let Some(text) = value.as_str() {
        return Ok(vec![ToolResultContent::Text {
            text: text.to_string(),
        }]);
    }

    if let Some(items) = value.as_array() {
        let mut result = Vec::new();
        for item in items {
            if let Some(item_type) = item.get("type").and_then(|v| v.as_str()) {
                match item_type {
                    "text" => {
                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                            result.push(ToolResultContent::Text {
                                text: text.to_string(),
                            });
                        }
                    }
                    "image" | "image_url" => {
                        return Err(ProviderError::not_implemented(
                            "bedrock",
                            "Converse tool-result image content",
                        ));
                    }
                    "document" => {
                        return Err(ProviderError::not_implemented(
                            "bedrock",
                            "Converse tool-result document content",
                        ));
                    }
                    _ => {
                        result.push(ToolResultContent::Text {
                            text: item.to_string(),
                        });
                    }
                }
            } else {
                result.push(ToolResultContent::Text {
                    text: item.to_string(),
                });
            }
        }
        return Ok(result);
    }

    Ok(vec![ToolResultContent::Text {
        text: value.to_string(),
    }])
}

#[cfg(test)]
#[path = "converse_tests.rs"]
mod tests;
