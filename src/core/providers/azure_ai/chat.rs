//! Azure AI Chat Handler
//!
//! Complete chat completion implementation for Azure AI Foundry

use futures::{Stream, StreamExt};
use reqwest::header::HeaderMap;
use serde_json::{Value, json};
use std::pin::Pin;

// Type system imports
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    context::RequestContext,
    message::MessageContent,
    message::MessageRole,
    responses::{ChatChoice, ChatChunk, ChatResponse, FinishReason, Usage},
};

use super::config::{AzureAIConfig, AzureAIEndpointType};
use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;
use crate::utils::net::http::create_custom_client_with_headers;

/// Azure AI chat handler - complete implementation
#[derive(Debug, Clone)]
pub struct AzureAIChatHandler {
    config: AzureAIConfig,
    client: reqwest::Client,
}

impl AzureAIChatHandler {
    /// Create new chat handler
    pub fn new(config: AzureAIConfig) -> Result<Self, ProviderError> {
        // Create headers for the client
        let mut headers = HeaderMap::new();
        let default_headers = config
            .create_default_headers()
            .map_err(|e| ProviderError::configuration("azure_ai", &e))?;

        for (key, value) in default_headers {
            let header_name =
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    ProviderError::configuration("azure_ai", format!("Invalid header name: {}", e))
                })?;
            let header_value = reqwest::header::HeaderValue::from_str(&value).map_err(|e| {
                ProviderError::configuration("azure_ai", format!("Invalid header value: {}", e))
            })?;
            headers.insert(header_name, header_value);
        }

        let client = create_custom_client_with_headers(config.timeout(), headers).map_err(|e| {
            ProviderError::configuration("azure_ai", format!("Failed to create HTTP client: {}", e))
        })?;

        Ok(Self { config, client })
    }

    /// Create chat completion
    pub async fn create_chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        // Validate request
        AzureAIChatUtils::validate_request(&request)?;

        // Transform request to Azure AI format
        let azure_request = AzureAIChatUtils::transform_request(&request)?;

        // Build URL
        let url = self
            .config
            .build_endpoint_url(AzureAIEndpointType::ChatCompletions.as_path())
            .map_err(|e| ProviderError::configuration("azure_ai", &e))?;

        // Execute request
        let response = self
            .client
            .post(&url)
            .json(&azure_request)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure_ai", format!("Request failed: {}", e)))?;

        // Handle error responses
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(HttpErrorMapper::map_status_code(
                "azure_ai",
                status,
                &error_body,
            ));
        }

        // Parse response
        let response_json: Value = response.json().await.map_err(|e| {
            ProviderError::response_parsing("azure_ai", format!("Failed to parse response: {}", e))
        })?;

        // Transform to standard format
        AzureAIChatUtils::transform_response(response_json, &request.model)
    }

    /// Create streaming chat completion
    pub async fn create_chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        // Validate request
        AzureAIChatUtils::validate_request(&request)?;

        // Transform request to Azure AI format with streaming enabled
        let mut azure_request = AzureAIChatUtils::transform_request(&request)?;
        azure_request["stream"] = json!(true);

        // Build URL
        let url = self
            .config
            .build_endpoint_url(AzureAIEndpointType::ChatCompletions.as_path())
            .map_err(|e| ProviderError::configuration("azure_ai", &e))?;

        // Execute streaming request
        let response = self
            .client
            .post(&url)
            .json(&azure_request)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure_ai", format!("Request failed: {}", e)))?;

        // Handle error responses
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(HttpErrorMapper::map_status_code(
                "azure_ai",
                status,
                &error_body,
            ));
        }

        // Create SSE stream
        let model = request.model.clone();
        let stream = response.bytes_stream().map(move |chunk_result| {
            chunk_result
                .map_err(|e| ProviderError::network("azure_ai", format!("Stream error: {}", e)))
                .and_then(|chunk| {
                    let chunk_str = String::from_utf8_lossy(&chunk);
                    AzureAIChatUtils::parse_streaming_chunk(&chunk_str, &model)
                })
        });

        Ok(Box::pin(stream))
    }
}

/// Utility struct for Azure AI chat operations
pub struct AzureAIChatUtils;

impl AzureAIChatUtils {
    /// Validate chat request
    pub fn validate_request(request: &ChatRequest) -> Result<(), ProviderError> {
        if request.messages.is_empty() {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Messages cannot be empty",
            ));
        }

        if request.model.is_empty() {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Model cannot be empty",
            ));
        }

        // Validate temperature range
        if let Some(temp) = request.temperature
            && !(0.0..=2.0).contains(&temp)
        {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Temperature must be between 0.0 and 2.0",
            ));
        }

        // Validate top_p range
        if let Some(top_p) = request.top_p
            && !(0.0..=1.0).contains(&top_p)
        {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "top_p must be between 0.0 and 1.0",
            ));
        }

        Ok(())
    }

    /// Transform ChatRequest to Azure AI format
    pub fn transform_request(request: &ChatRequest) -> Result<Value, ProviderError> {
        let mut azure_request = json!({
            "model": request.model,
            "messages": Self::transform_messages(&request.messages)?
        });

        // Add optional parameters
        if let Some(temp) = request.temperature {
            azure_request["temperature"] = json!(temp);
        }

        if let Some(max_tokens) = request.max_tokens {
            azure_request["max_tokens"] = json!(max_tokens);
        }

        if let Some(max_completion_tokens) = request.max_completion_tokens {
            azure_request["max_completion_tokens"] = json!(max_completion_tokens);
        }

        if let Some(top_p) = request.top_p {
            azure_request["top_p"] = json!(top_p);
        }

        if let Some(freq_penalty) = request.frequency_penalty {
            azure_request["frequency_penalty"] = json!(freq_penalty);
        }

        if let Some(pres_penalty) = request.presence_penalty {
            azure_request["presence_penalty"] = json!(pres_penalty);
        }

        if let Some(stop) = &request.stop {
            azure_request["stop"] = json!(stop);
        }

        if request.stream {
            azure_request["stream"] = json!(true);
        }

        // Add tools if present
        if let Some(tools) = &request.tools {
            azure_request["tools"] = serde_json::to_value(tools).map_err(|e| {
                ProviderError::transformation_error(
                    "azure_ai",
                    "request",
                    "azure_ai",
                    format!("Failed to serialize tools: {}", e),
                )
            })?;
        }

        if let Some(tool_choice) = &request.tool_choice {
            azure_request["tool_choice"] = serde_json::to_value(tool_choice).map_err(|e| {
                ProviderError::transformation_error(
                    "azure_ai",
                    "request",
                    "azure_ai",
                    format!("Failed to serialize tool_choice: {}", e),
                )
            })?;
        }

        Ok(azure_request)
    }

    /// Transform messages to Azure AI format
    fn transform_messages(messages: &[ChatMessage]) -> Result<Value, ProviderError> {
        let mut azure_messages = Vec::new();

        for message in messages {
            let mut azure_message = json!({
                "role": Self::transform_role(&message.role)
            });

            // Handle content based on type
            if let Some(content) = &message.content {
                match content {
                    MessageContent::Text(text) => {
                        azure_message["content"] = json!(text);
                    }
                    MessageContent::Parts(parts) => {
                        // Multi-modal content - transform parts to Azure AI format
                        let content_parts = parts
                            .iter()
                            .map(|part| {
                                // Transform ContentPart to Azure AI format
                                // This is a simplified transformation - expand as needed based on ContentPart structure
                                json!(part)
                            })
                            .collect::<Vec<_>>();
                        azure_message["content"] = json!(content_parts);
                    }
                }
            }

            // Add name if present
            if let Some(name) = &message.name {
                azure_message["name"] = json!(name);
            }

            // Add function call if present
            if let Some(function_call) = &message.function_call {
                azure_message["function_call"] =
                    serde_json::to_value(function_call).map_err(|e| {
                        ProviderError::transformation_error(
                            "azure_ai",
                            "request",
                            "azure_ai",
                            format!("Failed to serialize function_call: {}", e),
                        )
                    })?;
            }

            // Add tool calls if present
            if let Some(tool_calls) = &message.tool_calls {
                azure_message["tool_calls"] = serde_json::to_value(tool_calls).map_err(|e| {
                    ProviderError::transformation_error(
                        "azure_ai",
                        "request",
                        "azure_ai",
                        format!("Failed to serialize tool_calls: {}", e),
                    )
                })?;
            }

            // Add tool call ID if present
            if let Some(tool_call_id) = &message.tool_call_id {
                azure_message["tool_call_id"] = json!(tool_call_id);
            }

            azure_messages.push(azure_message);
        }

        Ok(json!(azure_messages))
    }

    /// Transform message role to Azure AI format
    fn transform_role(role: &MessageRole) -> &'static str {
        match role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Function => "function",
            MessageRole::Tool => "tool",
        }
    }

    /// Transform Azure AI response to ChatResponse
    pub fn transform_response(response: Value, model: &str) -> Result<ChatResponse, ProviderError> {
        let id = response["id"].as_str().unwrap_or("unknown").to_string();

        let created = response["created"]
            .as_i64()
            .unwrap_or_else(|| chrono::Utc::now().timestamp());

        let choices = response["choices"]
            .as_array()
            .ok_or_else(|| ProviderError::response_parsing("azure_ai", "Invalid choices format"))?
            .iter()
            .enumerate()
            .map(|(index, choice)| Self::transform_choice(choice, index))
            .collect::<Result<Vec<_>, _>>()?;

        let usage = response.get("usage").map(|usage_data| Usage {
            prompt_tokens: usage_data["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: usage_data["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: usage_data["total_tokens"].as_u64().unwrap_or(0) as u32,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        });

        Ok(ChatResponse {
            id,
            object: "chat.completion".to_string(),
            created,
            model: model.to_string(),
            choices,
            usage,
            system_fingerprint: response["system_fingerprint"]
                .as_str()
                .map(|s| s.to_string()),
        })
    }

    /// Transform choice from Azure AI format
    fn transform_choice(choice: &Value, index: usize) -> Result<ChatChoice, ProviderError> {
        let message_data = &choice["message"];
        let role = match message_data["role"].as_str().unwrap_or("assistant") {
            "system" => MessageRole::System,
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            "function" => MessageRole::Function,
            "tool" => MessageRole::Tool,
            _ => MessageRole::Assistant,
        };

        let content = if let Some(content_str) = message_data["content"].as_str() {
            MessageContent::Text(content_str.to_string())
        } else {
            MessageContent::Text(String::new())
        };

        let message = ChatMessage {
            role,
            content: Some(content),
            thinking: None,
            name: message_data["name"].as_str().map(|s| s.to_string()),
            function_call: None, // TODO: Handle function calls
            tool_calls: None,    // TODO: Handle tool calls
            tool_call_id: message_data["tool_call_id"].as_str().map(|s| s.to_string()),
        };

        let finish_reason = match choice["finish_reason"].as_str() {
            Some("stop") => Some(FinishReason::Stop),
            Some("length") => Some(FinishReason::Length),
            Some("content_filter") => Some(FinishReason::ContentFilter),
            Some("tool_calls") => Some(FinishReason::ToolCalls),
            Some("function_call") => Some(FinishReason::FunctionCall),
            _ => None,
        };

        Ok(ChatChoice {
            index: index as u32,
            message,
            finish_reason,
            logprobs: None, // TODO: Handle logprobs if needed
        })
    }

    /// Parse streaming chunk from Azure AI
    pub fn parse_streaming_chunk(chunk_str: &str, model: &str) -> Result<ChatChunk, ProviderError> {
        // Parse SSE format
        let lines: Vec<&str> = chunk_str.split("\n").collect();

        for line in lines {
            if let Some(data) = line.strip_prefix("data: ") {
                // Remove "data: " prefix

                if data == "[DONE]" {
                    // End of stream marker
                    return Ok(ChatChunk {
                        id: "stream_end".to_string(),
                        object: "chat.completion.chunk".to_string(),
                        created: chrono::Utc::now().timestamp(),
                        model: model.to_string(),
                        choices: vec![],
                        usage: None,
                        system_fingerprint: None,
                    });
                }

                // Parse JSON data
                let chunk_data: Value = serde_json::from_str(data).map_err(|e| {
                    ProviderError::response_parsing(
                        "azure_ai",
                        format!("Failed to parse chunk: {}", e),
                    )
                })?;

                return Self::transform_streaming_chunk(chunk_data, model);
            }
        }

        // Empty chunk
        Ok(ChatChunk {
            id: "empty".to_string(),
            object: "chat.completion.chunk".to_string(),
            created: chrono::Utc::now().timestamp(),
            model: model.to_string(),
            choices: vec![],
            usage: None,
            system_fingerprint: None,
        })
    }

    /// Transform streaming chunk data
    fn transform_streaming_chunk(
        chunk_data: Value,
        model: &str,
    ) -> Result<ChatChunk, ProviderError> {
        let id = chunk_data["id"].as_str().unwrap_or("unknown").to_string();

        let created = chunk_data["created"]
            .as_i64()
            .unwrap_or_else(|| chrono::Utc::now().timestamp());

        // Transform choices
        let choices = if let Some(choices_array) = chunk_data["choices"].as_array() {
            choices_array
                .iter()
                .enumerate()
                .map(|(index, choice)| {
                    // TODO: Implement proper streaming choice transformation
                    // For now, create a basic structure
                    crate::core::types::responses::ChatStreamChoice {
                        index: index as u32,
                        delta: crate::core::types::responses::ChatDelta {
                            role: None,
                            content: choice["delta"]["content"].as_str().map(|s| s.to_string()),
                            thinking: None,
                            function_call: None,
                            tool_calls: None,
                        },
                        finish_reason: match choice["finish_reason"].as_str() {
                            Some("stop") => Some(FinishReason::Stop),
                            Some("length") => Some(FinishReason::Length),
                            Some("content_filter") => Some(FinishReason::ContentFilter),
                            Some("tool_calls") => Some(FinishReason::ToolCalls),
                            Some("function_call") => Some(FinishReason::FunctionCall),
                            _ => None,
                        },
                        logprobs: None,
                    }
                })
                .collect()
        } else {
            vec![]
        };

        Ok(ChatChunk {
            id,
            object: "chat.completion.chunk".to_string(),
            created,
            model: model.to_string(),
            choices,
            usage: None, // Usage typically not provided in streaming chunks
            system_fingerprint: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_request() -> ChatRequest {
        ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                thinking: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn test_validate_request_success() {
        let request = create_test_request();
        assert!(AzureAIChatUtils::validate_request(&request).is_ok());
    }

    #[test]
    fn test_validate_request_empty_messages() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            ..Default::default()
        };
        let result = AzureAIChatUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_empty_model() {
        let mut request = create_test_request();
        request.model = String::new();
        let result = AzureAIChatUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_temperature_too_high() {
        let mut request = create_test_request();
        request.temperature = Some(2.5);
        let result = AzureAIChatUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_temperature_negative() {
        let mut request = create_test_request();
        request.temperature = Some(-0.5);
        let result = AzureAIChatUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_top_p_out_of_range() {
        let mut request = create_test_request();
        request.top_p = Some(1.5);
        let result = AzureAIChatUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_request_basic() {
        let request = create_test_request();
        let result = AzureAIChatUtils::transform_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["model"], "gpt-4");
        assert!(value["messages"].is_array());
    }

    #[test]
    fn test_transform_request_with_options() {
        let mut request = create_test_request();
        request.temperature = Some(0.5);
        request.max_tokens = Some(100);
        request.top_p = Some(0.9);
        request.frequency_penalty = Some(0.5);
        request.presence_penalty = Some(0.5);
        request.stop = Some(vec!["STOP".to_string()]);

        let result = AzureAIChatUtils::transform_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        // Use approximate comparison for floating point values
        assert!((value["temperature"].as_f64().unwrap() - 0.5).abs() < 0.001);
        assert_eq!(value["max_tokens"], 100);
        assert!((value["top_p"].as_f64().unwrap() - 0.9).abs() < 0.001);
        assert!((value["frequency_penalty"].as_f64().unwrap() - 0.5).abs() < 0.001);
        assert!((value["presence_penalty"].as_f64().unwrap() - 0.5).abs() < 0.001);
        assert!(value["stop"].is_array());
    }

    #[test]
    fn test_transform_request_with_stream() {
        let mut request = create_test_request();
        request.stream = true;

        let result = AzureAIChatUtils::transform_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["stream"], true);
    }

    #[test]
    fn test_transform_response() {
        let response = json!({
            "id": "chatcmpl-123",
            "created": 1700000000,
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello, how can I help?"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30
            },
            "system_fingerprint": "fp_123"
        });

        let result = AzureAIChatUtils::transform_response(response, "gpt-4");
        assert!(result.is_ok());
        let chat_response = result.unwrap();
        assert_eq!(chat_response.id, "chatcmpl-123");
        assert_eq!(chat_response.model, "gpt-4");
        assert_eq!(chat_response.choices.len(), 1);
        assert!(chat_response.usage.is_some());
        let usage = chat_response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 20);
        assert_eq!(usage.total_tokens, 30);
    }

    #[test]
    fn test_transform_response_finish_reasons() {
        let finish_reasons = vec![
            ("stop", FinishReason::Stop),
            ("length", FinishReason::Length),
            ("content_filter", FinishReason::ContentFilter),
            ("tool_calls", FinishReason::ToolCalls),
            ("function_call", FinishReason::FunctionCall),
        ];

        for (reason_str, expected_reason) in finish_reasons {
            let response = json!({
                "id": "test",
                "choices": [{
                    "message": {
                        "role": "assistant",
                        "content": "test"
                    },
                    "finish_reason": reason_str
                }]
            });

            let result = AzureAIChatUtils::transform_response(response, "gpt-4").unwrap();
            assert_eq!(result.choices[0].finish_reason, Some(expected_reason));
        }
    }

    #[test]
    fn test_parse_streaming_chunk_done() {
        let chunk = "data: [DONE]";
        let result = AzureAIChatUtils::parse_streaming_chunk(chunk, "gpt-4");
        assert!(result.is_ok());
        let chat_chunk = result.unwrap();
        assert_eq!(chat_chunk.id, "stream_end");
        assert!(chat_chunk.choices.is_empty());
    }

    #[test]
    fn test_parse_streaming_chunk_content() {
        let chunk = r#"data: {"id":"test","choices":[{"delta":{"content":"Hello"}}]}"#;
        let result = AzureAIChatUtils::parse_streaming_chunk(chunk, "gpt-4");
        assert!(result.is_ok());
        let chat_chunk = result.unwrap();
        assert_eq!(chat_chunk.model, "gpt-4");
        assert_eq!(chat_chunk.choices.len(), 1);
        assert_eq!(
            chat_chunk.choices[0].delta.content.as_ref().unwrap(),
            "Hello"
        );
    }

    #[test]
    fn test_parse_streaming_chunk_empty() {
        let chunk = "";
        let result = AzureAIChatUtils::parse_streaming_chunk(chunk, "gpt-4");
        assert!(result.is_ok());
        let chat_chunk = result.unwrap();
        assert_eq!(chat_chunk.id, "empty");
    }

    #[test]
    fn test_transform_role() {
        assert_eq!(
            AzureAIChatUtils::transform_role(&MessageRole::System),
            "system"
        );
        assert_eq!(AzureAIChatUtils::transform_role(&MessageRole::User), "user");
        assert_eq!(
            AzureAIChatUtils::transform_role(&MessageRole::Assistant),
            "assistant"
        );
        assert_eq!(
            AzureAIChatUtils::transform_role(&MessageRole::Function),
            "function"
        );
        assert_eq!(AzureAIChatUtils::transform_role(&MessageRole::Tool), "tool");
    }

    #[test]
    fn test_transform_messages_with_name() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                thinking: None,
                name: Some("TestUser".to_string()),
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            ..Default::default()
        };

        let result = AzureAIChatUtils::transform_request(&request).unwrap();
        assert!(result["messages"][0]["name"].is_string());
        assert_eq!(result["messages"][0]["name"], "TestUser");
    }

    #[test]
    fn test_transform_messages_with_tool_call_id() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::Tool,
                content: Some(MessageContent::Text("Result".to_string())),
                thinking: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: Some("call_123".to_string()),
            }],
            ..Default::default()
        };

        let result = AzureAIChatUtils::transform_request(&request).unwrap();
        assert!(result["messages"][0]["tool_call_id"].is_string());
        assert_eq!(result["messages"][0]["tool_call_id"], "call_123");
    }

    #[test]
    fn test_transform_response_missing_usage() {
        let response = json!({
            "id": "test",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "test"
                }
            }]
        });

        let result = AzureAIChatUtils::transform_response(response, "gpt-4").unwrap();
        assert!(result.usage.is_none());
    }

    #[test]
    fn test_transform_response_message_roles() {
        let roles = vec!["system", "user", "assistant", "function", "tool"];

        for role in roles {
            let response = json!({
                "id": "test",
                "choices": [{
                    "message": {
                        "role": role,
                        "content": "test"
                    }
                }]
            });

            let result = AzureAIChatUtils::transform_response(response, "gpt-4");
            assert!(result.is_ok());
        }
    }
}
