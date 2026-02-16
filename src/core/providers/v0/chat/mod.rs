//! V0 Chat Completion Module
//!
//! Handles chat completion requests for V0 provider

use super::V0Provider;
use crate::core::providers::unified_provider::ProviderError;

/// Provider name constant for error messages
const PROVIDER_NAME: &str = "v0";
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    message::MessageRole,
    responses::{ChatChoice, ChatResponse, FinishReason, Usage},
};
use serde::{Deserialize, Serialize};

/// V0 Chat request (OpenAI-compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V0ChatRequest {
    /// Messages in the conversation
    pub messages: Vec<V0Message>,
    /// Model to use
    pub model: String,
    /// Whether to stream the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Tools available for function calling
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<V0Tool>>,
    /// Tool choice configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
}

/// V0 Message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V0Message {
    /// Role of the message sender
    pub role: String,
    /// Content of the message
    pub content: String,
    /// Optional tool calls
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<V0ToolCall>>,
    /// Optional tool call ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// V0 Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V0Tool {
    /// Type of tool (always "function")
    #[serde(rename = "type")]
    pub tool_type: String,
    /// Function definition
    pub function: V0Function,
}

/// V0 Function definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V0Function {
    /// Name of the function
    pub name: String,
    /// Description of the function
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Parameters schema
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// V0 Tool call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V0ToolCall {
    /// ID of the tool call
    pub id: String,
    /// Type of tool call
    #[serde(rename = "type")]
    pub call_type: String,
    /// Function call details
    pub function: V0FunctionCall,
}

/// V0 Function call
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V0FunctionCall {
    /// Name of the function
    pub name: String,
    /// Arguments passed to the function
    pub arguments: String,
}

/// V0 Chat response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V0ChatResponse {
    /// Response ID
    pub id: String,
    /// Object type
    pub object: String,
    /// Creation timestamp
    pub created: i64,
    /// Model used
    pub model: String,
    /// System fingerprint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    /// Response choices
    pub choices: Vec<V0Choice>,
    /// Usage information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<V0Usage>,
}

/// V0 Choice in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V0Choice {
    /// Index of the choice
    pub index: i32,
    /// Message content
    pub message: V0Message,
    /// Finish reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// V0 Usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct V0Usage {
    /// Prompt tokens used
    pub prompt_tokens: i32,
    /// Completion tokens used
    pub completion_tokens: i32,
    /// Total tokens used
    pub total_tokens: i32,
}

/// V0 Chat handler
pub struct V0ChatHandler;

impl V0ChatHandler {
    /// Handle chat completion request
    pub async fn handle_chat_completion(
        provider: &V0Provider,
        request: ChatRequest,
    ) -> Result<ChatResponse, ProviderError> {
        // Transform the request to V0 format
        let v0_request = Self::transform_request(request)?;

        // Send request to V0 API
        let v0_response = Self::send_request(provider, v0_request).await?;

        // Transform response back to standard format
        let response = Self::transform_response(v0_response)?;

        Ok(response)
    }

    /// Transform standard ChatRequest to V0ChatRequest
    fn transform_request(request: ChatRequest) -> Result<V0ChatRequest, ProviderError> {
        // Transform messages
        let messages = request
            .messages
            .into_iter()
            .map(|msg| V0Message {
                role: msg.role.to_string(),
                content: match msg.content {
                    Some(crate::core::types::message::MessageContent::Text(text)) => text,
                    Some(crate::core::types::message::MessageContent::Parts(_)) => {
                        // V0 doesn't support multimodal content, extract text only
                        String::new()
                    }
                    None => String::new(),
                },
                tool_calls: None, // TODO: Transform tool calls if present
                tool_call_id: None,
            })
            .collect();

        // Transform tools if present
        let tools = request.tools.map(|tools| {
            tools
                .into_iter()
                .map(|tool| V0Tool {
                    tool_type: "function".to_string(),
                    function: V0Function {
                        name: tool.function.name,
                        description: tool.function.description,
                        parameters: tool.function.parameters,
                    },
                })
                .collect()
        });

        Ok(V0ChatRequest {
            messages,
            model: request.model,
            stream: Some(request.stream),
            tools,
            tool_choice: request
                .tool_choice
                .map(|tc| serde_json::to_value(tc).unwrap_or_default()),
        })
    }

    /// Send request to V0 API
    async fn send_request(
        provider: &V0Provider,
        request: V0ChatRequest,
    ) -> Result<V0ChatResponse, ProviderError> {
        let url = provider.get_endpoint("chat/completions");
        let headers = provider.create_headers();

        let response = provider
            .client
            .post(&url)
            .headers(headers)
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            return match status.as_u16() {
                401 => Err(ProviderError::authentication(
                    PROVIDER_NAME,
                    "Authentication failed",
                )),
                429 => Err(ProviderError::rate_limit(PROVIDER_NAME, None)),
                404 => Err(ProviderError::model_not_found(PROVIDER_NAME, request.model)),
                _ => Err(ProviderError::api_error(
                    PROVIDER_NAME,
                    status.as_u16(),
                    format!("HTTP {}: {}", status, error_text),
                )),
            };
        }

        let v0_response: V0ChatResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::serialization(PROVIDER_NAME, e.to_string()))?;

        Ok(v0_response)
    }

    /// Transform V0ChatResponse to standard ChatResponse
    fn transform_response(v0_response: V0ChatResponse) -> Result<ChatResponse, ProviderError> {
        let choices = v0_response
            .choices
            .into_iter()
            .map(|choice| ChatChoice {
                index: choice.index as u32,
                message: ChatMessage {
                    role: match choice.message.role.as_str() {
                        "system" => MessageRole::System,
                        "user" => MessageRole::User,
                        "assistant" => MessageRole::Assistant,
                        "tool" => MessageRole::Tool,
                        "function" => MessageRole::Function,
                        _ => MessageRole::Assistant, // default fallback
                    },
                    content: Some(crate::core::types::message::MessageContent::Text(
                        choice.message.content,
                    )),
                    thinking: None,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                },
                finish_reason: choice
                    .finish_reason
                    .and_then(|reason| match reason.as_str() {
                        "stop" => Some(FinishReason::Stop),
                        "length" => Some(FinishReason::Length),
                        "tool_calls" => Some(FinishReason::ToolCalls),
                        "content_filter" => Some(FinishReason::ContentFilter),
                        "function_call" => Some(FinishReason::FunctionCall),
                        _ => None,
                    }),
                logprobs: None,
            })
            .collect();

        let usage = v0_response.usage.map(|usage| Usage {
            prompt_tokens: usage.prompt_tokens as u32,
            completion_tokens: usage.completion_tokens as u32,
            total_tokens: usage.total_tokens as u32,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        });

        Ok(ChatResponse {
            id: v0_response.id,
            object: v0_response.object,
            created: v0_response.created,
            model: v0_response.model,
            system_fingerprint: v0_response.system_fingerprint,
            choices,
            usage,
        })
    }

    /// Get supported parameters for V0
    pub fn get_supported_parameters() -> Vec<&'static str> {
        vec!["messages", "model", "stream", "tools", "tool_choice"]
    }

    /// Validate request parameters
    pub fn validate_request(request: &ChatRequest) -> Result<(), ProviderError> {
        // Check required fields
        if request.messages.is_empty() {
            return Err(ProviderError::invalid_request(
                PROVIDER_NAME,
                "Messages cannot be empty",
            ));
        }

        if request.model.is_empty() {
            return Err(ProviderError::invalid_request(
                PROVIDER_NAME,
                "Model is required",
            ));
        }

        // Validate messages
        for (i, message) in request.messages.iter().enumerate() {
            if message.role.is_empty() {
                return Err(ProviderError::invalid_request(
                    PROVIDER_NAME,
                    format!("Message {} role cannot be empty", i),
                ));
            }

            match &message.content {
                Some(crate::core::types::message::MessageContent::Text(text)) => {
                    if text.is_empty() {
                        return Err(ProviderError::invalid_request(
                            PROVIDER_NAME,
                            format!("Message {} content cannot be empty", i),
                        ));
                    }
                }
                Some(crate::core::types::message::MessageContent::Parts(array)) => {
                    if array.is_empty() {
                        return Err(ProviderError::invalid_request(
                            PROVIDER_NAME,
                            format!("Message {} content array cannot be empty", i),
                        ));
                    }
                }
                None => {
                    // Content is optional for some roles like tool
                    if message.role == MessageRole::User {
                        return Err(ProviderError::invalid_request(
                            PROVIDER_NAME,
                            format!("Message {} content cannot be None for user role", i),
                        ));
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};

    #[test]
    fn test_transform_request() {
        let request = ChatRequest {
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello, world!".to_string())),
                ..Default::default()
            }],
            model: "v0-default".to_string(),
            stream: false,
            ..Default::default()
        };

        let v0_request = V0ChatHandler::transform_request(request).unwrap();
        assert_eq!(v0_request.model, "v0-default");
        assert_eq!(v0_request.messages.len(), 1);
        assert_eq!(v0_request.messages[0].role, "user");
        assert_eq!(v0_request.messages[0].content, "Hello, world!");
    }

    #[test]
    fn test_validate_request() {
        let valid_request = ChatRequest {
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                ..Default::default()
            }],
            model: "v0-default".to_string(),
            ..Default::default()
        };

        assert!(V0ChatHandler::validate_request(&valid_request).is_ok());

        let invalid_request = ChatRequest {
            messages: vec![],
            model: "".to_string(),
            ..Default::default()
        };

        assert!(V0ChatHandler::validate_request(&invalid_request).is_err());
    }

    #[test]
    fn test_supported_parameters() {
        let params = V0ChatHandler::get_supported_parameters();
        assert!(params.contains(&"messages"));
        assert!(params.contains(&"model"));
        assert!(params.contains(&"stream"));
        assert!(params.contains(&"tools"));
        assert!(params.contains(&"tool_choice"));
    }
}
