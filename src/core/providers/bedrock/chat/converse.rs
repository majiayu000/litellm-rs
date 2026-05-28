//! Converse API Implementation
//!
//! Modern unified API for chat completions in Bedrock

use crate::core::providers::bedrock::model_id::is_prompt_management_model_id;
use crate::core::providers::bedrock::parameter_policy::{
    has_bedrock_model_parameter_overrides, serialize_bedrock_chat_parameters,
};
use crate::core::providers::bedrock::parse_bedrock_model_id;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use crate::core::types::tools as openai_tools;
use crate::core::types::{message::MessageContent, message::MessageRole};
use serde::ser::SerializeMap;
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
    pub prompt_variables: Option<Value>,
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
#[serde(rename_all = "camelCase")]
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
#[derive(Debug, Deserialize)]
pub enum ToolChoice {
    Auto,
    Any,
    Tool { name: String },
}

impl Serialize for ToolChoice {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        #[derive(Serialize)]
        struct ToolChoiceTool<'a> {
            name: &'a str,
        }

        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            ToolChoice::Auto => map.serialize_entry("auto", &serde_json::json!({}))?,
            ToolChoice::Any => map.serialize_entry("any", &serde_json::json!({}))?,
            ToolChoice::Tool { name } => map.serialize_entry("tool", &ToolChoiceTool { name })?,
        }
        map.end()
    }
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
    let execution_model_id = parse_bedrock_model_id(&request.model).execution_model_id;

    // Send request using the client
    let response = client
        .send_request(
            &execution_model_id,
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
    let prompt_management = is_prompt_management_model_id(&request.model);
    let parameter_fields = if prompt_management {
        None
    } else {
        Some(serialize_bedrock_chat_parameters(request)?)
    };

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

    if prompt_management && !system_messages.is_empty() {
        return Err(ProviderError::invalid_request(
            "bedrock",
            "Prompt-management ARNs do not support request-level system messages",
        ));
    }

    let inference_config = if prompt_management {
        if request.max_tokens.is_some()
            || request.max_completion_tokens.is_some()
            || request.temperature.is_some()
            || request.top_p.is_some()
            || request.stop.is_some()
            || has_bedrock_model_parameter_overrides(request)
        {
            return Err(ProviderError::invalid_request(
                "bedrock",
                "Prompt-management ARNs do not support request-level inferenceConfig or additionalModelRequestFields",
            ));
        }
        None
    } else {
        let Some(parameter_fields) = parameter_fields.as_ref() else {
            return Err(ProviderError::configuration(
                "bedrock",
                "Bedrock parameter fields were not serialized for a Converse request",
            ));
        };
        Some(InferenceConfig {
            max_tokens: parameter_fields.max_tokens,
            temperature: parameter_fields.temperature,
            top_p: parameter_fields.top_p,
            stop_sequences: parameter_fields.stop_sequences.clone(),
        })
    };

    // Build tool config if tools are present
    let tool_config = if let Some(tools) = &request.tools {
        if prompt_management {
            return Err(ProviderError::invalid_request(
                "bedrock",
                "Prompt-management ARNs do not support request-level toolConfig",
            ));
        }
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
            tool_choice: map_tool_choice(request.tool_choice.as_ref())?,
        })
    } else if request.tool_choice.is_some() {
        return Err(ProviderError::invalid_request(
            "bedrock",
            "Bedrock Converse tool_choice requires tools",
        ));
    } else {
        None
    };

    let prompt_variables = if prompt_management {
        prompt_variables_from_extra_params(request)?
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
        prompt_variables,
        tool_config,
        guardrail_config: None, // NOTE: guardrail support not yet implemented
        additional_model_request_fields: parameter_fields
            .as_ref()
            .and_then(|fields| fields.additional_model_request_fields()),
    })
}

fn map_tool_choice(
    tool_choice: Option<&openai_tools::ToolChoice>,
) -> Result<Option<ToolChoice>, ProviderError> {
    let Some(tool_choice) = tool_choice else {
        return Ok(None);
    };

    match tool_choice {
        openai_tools::ToolChoice::String(choice) => match choice.as_str() {
            "auto" => Ok(Some(ToolChoice::Auto)),
            "required" | "any" => Ok(Some(ToolChoice::Any)),
            "none" => Err(ProviderError::invalid_request(
                "bedrock",
                "Bedrock Converse does not support tool_choice=none when tools are provided",
            )),
            other => Err(ProviderError::invalid_request(
                "bedrock",
                format!("Unsupported Bedrock Converse tool_choice: {other}"),
            )),
        },
        openai_tools::ToolChoice::Specific { function, .. } => {
            let Some(function) = function else {
                return Err(ProviderError::invalid_request(
                    "bedrock",
                    "Specific Bedrock Converse tool_choice requires a function name",
                ));
            };
            Ok(Some(ToolChoice::Tool {
                name: function.name.clone(),
            }))
        }
    }
}

fn prompt_variables_from_extra_params(
    request: &ChatRequest,
) -> Result<Option<Value>, ProviderError> {
    let Some(prompt_variables) = request
        .extra_params
        .get("promptVariables")
        .or_else(|| request.extra_params.get("prompt_variables"))
    else {
        return Ok(None);
    };

    let variables = prompt_variables.as_object().ok_or_else(|| {
        ProviderError::invalid_request(
            "bedrock",
            "promptVariables must be an object for Bedrock prompt-management ARNs",
        )
    })?;
    let mut normalized = serde_json::Map::new();

    for (name, value) in variables {
        let prompt_value = if let Some(text) = value.as_str() {
            serde_json::json!({ "text": text })
        } else if value.as_object().is_some_and(|object| {
            object.len() == 1 && object.get("text").and_then(Value::as_str).is_some()
        }) {
            value.clone()
        } else {
            return Err(ProviderError::invalid_request(
                "bedrock",
                "promptVariables values must be strings or objects with a string text field",
            ));
        };
        normalized.insert(name.clone(), prompt_value);
    }

    Ok(Some(Value::Object(normalized)))
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

#[cfg(test)]
#[path = "converse_parameter_policy_tests.rs"]
mod parameter_policy_tests;
