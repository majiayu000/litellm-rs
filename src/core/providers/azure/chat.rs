//! Azure OpenAI Chat Handler
//!
//! Complete chat completion implementation for Azure OpenAI Service

use futures::{Stream, StreamExt};
use serde_json::{Value, json};
use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    context::RequestContext,
    message::MessageContent,
    message::MessageRole,
    responses::{
        ChatChoice, ChatChunk, ChatDelta, ChatResponse, ChatStreamChoice, FinishReason, Usage,
    },
};

use super::config::AzureConfig;
use super::error::{azure_api_error, azure_config_error};
use super::utils::{AzureEndpointType, AzureUtils};
use crate::core::providers::base::{
    HeaderPair, apply_headers, header, header_owned, header_static,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::streaming::utils::is_done_marker;
use crate::core::traits::provider::ProviderConfig;
use crate::utils::net::http::create_custom_client;

/// Azure OpenAI chat handler
#[derive(Debug, Clone)]
pub struct AzureChatHandler {
    config: AzureConfig,
    client: reqwest::Client,
}

impl AzureChatHandler {
    /// Create new chat handler
    pub fn new(config: AzureConfig) -> Result<Self, ProviderError> {
        let client = create_custom_client(ProviderConfig::timeout(&config))
            .map_err(|e| azure_config_error(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Build request headers using the unified HeaderPair pattern.
    async fn get_request_headers(&self) -> Result<Vec<HeaderPair>, ProviderError> {
        let mut headers = Vec::with_capacity(4);

        // Add API key
        if let Some(api_key) = self.config.get_effective_api_key().await {
            headers.push(header("api-key", api_key));
        } else {
            return Err(ProviderError::authentication(
                "azure",
                "No API key available".to_string(),
            ));
        }

        headers.push(header_static("Content-Type", "application/json"));

        // Add custom headers
        for (key, value) in &self.config.custom_headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        Ok(headers)
    }

    /// Create chat completion
    pub async fn create_chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> Result<ChatResponse, ProviderError> {
        // Get deployment name
        let deployment = self.config.get_effective_deployment_name(&request.model);

        // Get Azure endpoint
        let azure_endpoint = self
            .config
            .get_effective_azure_endpoint()
            .ok_or_else(|| azure_config_error("Azure endpoint not configured".to_string()))?;

        // Build URL
        let url = AzureUtils::build_azure_url(
            &azure_endpoint,
            &deployment,
            &self.config.api_version,
            AzureEndpointType::ChatCompletions,
        );

        // Transform request
        let azure_request = self.transform_request(&request)?;

        // Build headers
        let headers = self.get_request_headers().await?;

        // Execute request
        let response = apply_headers(self.client.post(&url).json(&azure_request), headers)
            .send()
            .await?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(azure_api_error(status, error_body));
        }

        // Parse response
        let response_json: Value = response.json().await?;

        // Transform to standard format
        self.transform_response(response_json, &deployment)
    }

    /// Create streaming chat completion
    pub async fn create_chat_completion_stream(
        &self,
        mut request: ChatRequest,
        _context: RequestContext,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>>, ProviderError>
    {
        // Force streaming
        request.stream = true;

        // Get deployment name
        let deployment = self.config.get_effective_deployment_name(&request.model);

        // Get Azure endpoint
        let azure_endpoint = self
            .config
            .get_effective_azure_endpoint()
            .ok_or_else(|| azure_config_error("Azure endpoint not configured".to_string()))?;

        // Build URL
        let url = AzureUtils::build_azure_url(
            &azure_endpoint,
            &deployment,
            &self.config.api_version,
            AzureEndpointType::ChatCompletions,
        );

        // Transform request
        let azure_request = self.transform_request(&request)?;

        // Build headers
        let headers = self.get_request_headers().await?;

        // Execute streaming request
        let response = apply_headers(self.client.post(&url).json(&azure_request), headers)
            .send()
            .await?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(azure_api_error(status, error_body));
        }

        // Create stream from response
        let deployment_clone = deployment.clone();
        let stream = async_stream::stream! {
            let mut bytes_stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = bytes_stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        // Process complete SSE messages
                        while let Some(line_end) = buffer.find('\n') {
                            let line = buffer.drain(..=line_end).collect::<String>();
                            let line = line.trim();

                            if let Some(data) = line.strip_prefix("data: ") {
                                if is_done_marker(data) {
                                    // End of stream
                                    break;
                                }

                                // Parse chunk
                                if let Ok(chunk_json) = serde_json::from_str::<Value>(data) {
                                    if let Ok(chunk) = Self::transform_streaming_chunk(chunk_json, &deployment_clone) {
                                        yield Ok(chunk);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        yield Err(ProviderError::network("azure", format!("Stream error: {}", e)));
                        break;
                    }
                }
            }
        };

        Ok(Box::pin(stream))
    }

    /// Transform request to Azure format
    pub fn transform_request(&self, request: &ChatRequest) -> Result<Value, ProviderError> {
        let mut body = json!({
            "messages": request.messages.iter().map(|msg| {
                self.transform_message(msg)
            }).collect::<Result<Vec<_>, _>>()?,
        });

        // Add optional parameters
        if let Some(temperature) = request.temperature {
            body["temperature"] = json!(temperature);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = json!(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(frequency_penalty) = request.frequency_penalty {
            body["frequency_penalty"] = json!(frequency_penalty);
        }
        if let Some(presence_penalty) = request.presence_penalty {
            body["presence_penalty"] = json!(presence_penalty);
        }
        if let Some(stop) = &request.stop {
            body["stop"] = json!(stop);
        }
        if request.stream {
            body["stream"] = json!(true);
        }

        // Add tools/functions if present
        if let Some(tools) = &request.tools {
            body["tools"] = json!(tools);
        }
        if let Some(tool_choice) = &request.tool_choice {
            body["tool_choice"] = json!(tool_choice);
        }

        // Add response format if present
        if let Some(response_format) = &request.response_format {
            body["response_format"] = json!(response_format);
        }

        // Add user if present
        if let Some(user) = &request.user {
            body["user"] = json!(user);
        }

        Ok(body)
    }

    /// Transform message to Azure format
    fn transform_message(&self, message: &ChatMessage) -> Result<Value, ProviderError> {
        let mut msg = json!({
            "role": match message.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Function => "function",
                MessageRole::Tool => "tool",
            }
        });

        // Add content
        if let Some(content) = &message.content {
            match content {
                MessageContent::Text(text) => {
                    msg["content"] = json!(text);
                }
                MessageContent::Parts(parts) => {
                    // Convert parts to Azure format
                    msg["content"] = json!(parts);
                }
            }
        }

        // Add optional fields
        if let Some(name) = &message.name {
            msg["name"] = json!(name);
        }
        if let Some(function_call) = &message.function_call {
            msg["function_call"] = json!(function_call);
        }
        if let Some(tool_calls) = &message.tool_calls {
            msg["tool_calls"] = json!(tool_calls);
        }
        if let Some(tool_call_id) = &message.tool_call_id {
            msg["tool_call_id"] = json!(tool_call_id);
        }

        Ok(msg)
    }

    /// Transform Azure response to standard format
    pub fn transform_response(
        &self,
        response: Value,
        model: &str,
    ) -> Result<ChatResponse, ProviderError> {
        let choices = response["choices"]
            .as_array()
            .ok_or_else(|| {
                ProviderError::serialization("azure", "Missing choices array".to_string())
            })?
            .iter()
            .map(|choice| {
                let message = &choice["message"];
                let content = message["content"]
                    .as_str()
                    .map(|s| MessageContent::Text(s.to_string()));

                ChatChoice {
                    index: choice["index"].as_u64().unwrap_or(0) as u32,
                    message: ChatMessage {
                        role: match message["role"].as_str().unwrap_or("assistant") {
                            "system" => MessageRole::System,
                            "user" => MessageRole::User,
                            "assistant" => MessageRole::Assistant,
                            "function" => MessageRole::Function,
                            "tool" => MessageRole::Tool,
                            _ => MessageRole::Assistant,
                        },
                        content,
                        thinking: None,
                        name: message["name"].as_str().map(|s| s.to_string()),
                        function_call: message["function_call"].as_object().and_then(|_| {
                            serde_json::from_value(message["function_call"].clone()).ok()
                        }),
                        tool_calls: message["tool_calls"].as_array().and_then(|_| {
                            serde_json::from_value(message["tool_calls"].clone()).ok()
                        }),
                        tool_call_id: message["tool_call_id"].as_str().map(|s| s.to_string()),
                    },
                    finish_reason: choice["finish_reason"].as_str().map(|reason| match reason {
                        "stop" => FinishReason::Stop,
                        "length" => FinishReason::Length,
                        "tool_calls" => FinishReason::ToolCalls,
                        "content_filter" => FinishReason::ContentFilter,
                        "function_call" => FinishReason::FunctionCall,
                        _ => FinishReason::Stop,
                    }),
                    logprobs: None,
                }
            })
            .collect();

        let usage = response.get("usage").map(|u| Usage {
            prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        });

        let timestamp = response["created"].as_i64().unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

        Ok(ChatResponse {
            id: response["id"].as_str().unwrap_or("").to_string(),
            object: "chat.completion".to_string(),
            created: timestamp,
            model: model.to_string(),
            choices,
            usage,
            system_fingerprint: response["system_fingerprint"]
                .as_str()
                .map(|s| s.to_string()),
        })
    }

    /// Transform streaming chunk
    fn transform_streaming_chunk(chunk: Value, model: &str) -> Result<ChatChunk, ProviderError> {
        let choices = if let Some(choices_array) = chunk["choices"].as_array() {
            choices_array
                .iter()
                .map(|choice| ChatStreamChoice {
                    index: choice["index"].as_u64().unwrap_or(0) as u32,
                    delta: ChatDelta {
                        role: choice["delta"]["role"].as_str().map(|r| match r {
                            "system" => MessageRole::System,
                            "user" => MessageRole::User,
                            "assistant" => MessageRole::Assistant,
                            "function" => MessageRole::Function,
                            "tool" => MessageRole::Tool,
                            _ => MessageRole::Assistant,
                        }),
                        content: choice["delta"]["content"].as_str().map(|s| s.to_string()),
                        thinking: None,
                        function_call: choice["delta"]["function_call"].as_object().and_then(
                            |_| {
                                serde_json::from_value(choice["delta"]["function_call"].clone())
                                    .ok()
                            },
                        ),
                        tool_calls: choice["delta"]["tool_calls"].as_array().and_then(|_| {
                            serde_json::from_value(choice["delta"]["tool_calls"].clone()).ok()
                        }),
                    },
                    finish_reason: choice["finish_reason"].as_str().map(|reason| match reason {
                        "stop" => FinishReason::Stop,
                        "length" => FinishReason::Length,
                        "tool_calls" => FinishReason::ToolCalls,
                        "content_filter" => FinishReason::ContentFilter,
                        "function_call" => FinishReason::FunctionCall,
                        _ => FinishReason::Stop,
                    }),
                    logprobs: None,
                })
                .collect()
        } else {
            vec![]
        };

        let timestamp = chunk["created"].as_i64().unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        });

        Ok(ChatChunk {
            id: chunk["id"].as_str().unwrap_or("").to_string(),
            object: "chat.completion.chunk".to_string(),
            created: timestamp,
            model: model.to_string(),
            choices,
            usage: None,
            system_fingerprint: chunk["system_fingerprint"].as_str().map(|s| s.to_string()),
        })
    }
}

/// Azure chat utilities
pub struct AzureChatUtils;

impl AzureChatUtils {
    /// Validate chat request
    pub fn validate_request(request: &ChatRequest) -> Result<(), ProviderError> {
        if request.messages.is_empty() {
            return Err(azure_config_error("Messages cannot be empty".to_string()));
        }
        Ok(())
    }

    /// Check if deployment supports functions
    pub fn supports_functions(deployment: &str) -> bool {
        let lower = deployment.to_lowercase();
        lower.contains("gpt-4") || lower.contains("gpt-35-turbo") || lower.contains("gpt-3.5-turbo")
    }

    /// Check if deployment supports tools
    pub fn supports_tools(deployment: &str) -> bool {
        let lower = deployment.to_lowercase();
        // GPT-4 Turbo and newer models support tools
        (lower.contains("gpt-4") && (lower.contains("turbo") || lower.contains("1106")))
            || (lower.contains("gpt-35-turbo") && lower.contains("1106"))
            || lower.contains("gpt-4o")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_message(role: MessageRole, content: &str) -> ChatMessage {
        ChatMessage {
            role,
            content: Some(MessageContent::Text(content.to_string())),
            thinking: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }

    fn create_test_request() -> ChatRequest {
        ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![create_test_message(MessageRole::User, "Hello")],
            ..Default::default()
        }
    }

    #[test]
    fn test_azure_chat_utils_validate_request_valid() {
        let request = create_test_request();
        assert!(AzureChatUtils::validate_request(&request).is_ok());
    }

    #[test]
    fn test_azure_chat_utils_validate_request_empty_messages() {
        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![],
            ..Default::default()
        };
        let result = AzureChatUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_supports_functions_gpt4() {
        assert!(AzureChatUtils::supports_functions("gpt-4"));
        assert!(AzureChatUtils::supports_functions("gpt-4-32k"));
        assert!(AzureChatUtils::supports_functions("gpt-4-turbo"));
        assert!(AzureChatUtils::supports_functions("GPT-4")); // Case insensitive
    }

    #[test]
    fn test_supports_functions_gpt35() {
        assert!(AzureChatUtils::supports_functions("gpt-35-turbo"));
        assert!(AzureChatUtils::supports_functions("gpt-35-turbo-16k"));
        assert!(AzureChatUtils::supports_functions("gpt-3.5-turbo"));
    }

    #[test]
    fn test_supports_functions_other_models() {
        assert!(!AzureChatUtils::supports_functions("text-davinci-003"));
        assert!(!AzureChatUtils::supports_functions(
            "text-embedding-ada-002"
        ));
        assert!(!AzureChatUtils::supports_functions("dall-e-3"));
    }

    #[test]
    fn test_supports_tools_gpt4_turbo() {
        assert!(AzureChatUtils::supports_tools("gpt-4-turbo"));
        assert!(AzureChatUtils::supports_tools("gpt-4-1106-preview"));
        assert!(AzureChatUtils::supports_tools("gpt-4-turbo-1106"));
    }

    #[test]
    fn test_supports_tools_gpt4o() {
        assert!(AzureChatUtils::supports_tools("gpt-4o"));
        assert!(AzureChatUtils::supports_tools("gpt-4o-mini"));
    }

    #[test]
    fn test_supports_tools_gpt35_1106() {
        assert!(AzureChatUtils::supports_tools("gpt-35-turbo-1106"));
    }

    #[test]
    fn test_supports_tools_older_models() {
        assert!(!AzureChatUtils::supports_tools("gpt-4"));
        assert!(!AzureChatUtils::supports_tools("gpt-35-turbo"));
        assert!(!AzureChatUtils::supports_tools("gpt-4-32k"));
    }

    #[test]
    fn test_azure_chat_handler_new() {
        let config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
        let handler = AzureChatHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_transform_request_basic() {
        let config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
        let handler = AzureChatHandler::new(config).unwrap();

        let request = create_test_request();
        let result = handler.transform_request(&request);
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!(value["messages"].is_array());
    }

    #[test]
    fn test_transform_request_with_options() {
        let config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
        let handler = AzureChatHandler::new(config).unwrap();

        let request = ChatRequest {
            model: "gpt-4".to_string(),
            messages: vec![create_test_message(MessageRole::User, "Hello")],
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: Some(0.9),
            frequency_penalty: Some(0.5),
            presence_penalty: Some(0.3),
            stop: Some(vec!["STOP".to_string()]),
            stream: true,
            ..Default::default()
        };

        let result = handler.transform_request(&request);
        assert!(result.is_ok());

        let value = result.unwrap();
        assert!((value["temperature"].as_f64().unwrap() - 0.7).abs() < 0.001);
        assert_eq!(value["max_tokens"], 100);
        assert!((value["top_p"].as_f64().unwrap() - 0.9).abs() < 0.001);
        assert!(value["stop"].is_array());
        assert!(value["stream"].as_bool().unwrap());
    }

    #[test]
    fn test_transform_response() {
        let config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
        let handler = AzureChatHandler::new(config).unwrap();

        let response = json!({
            "id": "chatcmpl-123",
            "created": 1234567890,
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello there!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 5,
                "total_tokens": 15
            },
            "system_fingerprint": "fp_abc123"
        });

        let result = handler.transform_response(response, "gpt-4");
        assert!(result.is_ok());

        let chat_response = result.unwrap();
        assert_eq!(chat_response.id, "chatcmpl-123");
        assert_eq!(chat_response.model, "gpt-4");
        assert_eq!(chat_response.choices.len(), 1);
        assert_eq!(
            chat_response.choices[0].message.role,
            MessageRole::Assistant
        );
        assert_eq!(
            chat_response.choices[0].finish_reason,
            Some(FinishReason::Stop)
        );
        assert!(chat_response.usage.is_some());
        let usage = chat_response.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 10);
        assert_eq!(usage.completion_tokens, 5);
        assert_eq!(usage.total_tokens, 15);
        assert_eq!(
            chat_response.system_fingerprint,
            Some("fp_abc123".to_string())
        );
    }

    #[test]
    fn test_transform_response_finish_reasons() {
        let config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
        let handler = AzureChatHandler::new(config).unwrap();

        let finish_reasons = vec![
            ("stop", FinishReason::Stop),
            ("length", FinishReason::Length),
            ("tool_calls", FinishReason::ToolCalls),
            ("content_filter", FinishReason::ContentFilter),
            ("function_call", FinishReason::FunctionCall),
        ];

        for (reason_str, expected_reason) in finish_reasons {
            let response = json!({
                "id": "test",
                "created": 1234567890,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Test"
                    },
                    "finish_reason": reason_str
                }]
            });

            let result = handler.transform_response(response, "gpt-4");
            assert!(result.is_ok());
            assert_eq!(
                result.unwrap().choices[0].finish_reason,
                Some(expected_reason)
            );
        }
    }

    #[test]
    fn test_transform_response_roles() {
        let config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
        let handler = AzureChatHandler::new(config).unwrap();

        let roles = vec![
            ("system", MessageRole::System),
            ("user", MessageRole::User),
            ("assistant", MessageRole::Assistant),
            ("function", MessageRole::Function),
            ("tool", MessageRole::Tool),
        ];

        for (role_str, expected_role) in roles {
            let response = json!({
                "id": "test",
                "created": 1234567890,
                "choices": [{
                    "index": 0,
                    "message": {
                        "role": role_str,
                        "content": "Test"
                    },
                    "finish_reason": "stop"
                }]
            });

            let result = handler.transform_response(response, "gpt-4");
            assert!(result.is_ok());
            assert_eq!(result.unwrap().choices[0].message.role, expected_role);
        }
    }

    #[test]
    fn test_transform_streaming_chunk() {
        let chunk = json!({
            "id": "chatcmpl-123",
            "created": 1234567890,
            "choices": [{
                "index": 0,
                "delta": {
                    "role": "assistant",
                    "content": "Hello"
                },
                "finish_reason": null
            }],
            "system_fingerprint": "fp_abc123"
        });

        let result = AzureChatHandler::transform_streaming_chunk(chunk, "gpt-4");
        assert!(result.is_ok());

        let chat_chunk = result.unwrap();
        assert_eq!(chat_chunk.id, "chatcmpl-123");
        assert_eq!(chat_chunk.model, "gpt-4");
        assert_eq!(chat_chunk.choices.len(), 1);
        assert_eq!(
            chat_chunk.choices[0].delta.content,
            Some("Hello".to_string())
        );
        assert_eq!(
            chat_chunk.choices[0].delta.role,
            Some(MessageRole::Assistant)
        );
    }

    #[test]
    fn test_transform_streaming_chunk_finish() {
        let chunk = json!({
            "id": "chatcmpl-123",
            "created": 1234567890,
            "choices": [{
                "index": 0,
                "delta": {},
                "finish_reason": "stop"
            }]
        });

        let result = AzureChatHandler::transform_streaming_chunk(chunk, "gpt-4");
        assert!(result.is_ok());

        let chat_chunk = result.unwrap();
        assert_eq!(
            chat_chunk.choices[0].finish_reason,
            Some(FinishReason::Stop)
        );
    }

    #[test]
    fn test_transform_streaming_chunk_empty_choices() {
        let chunk = json!({
            "id": "chatcmpl-123",
            "created": 1234567890
        });

        let result = AzureChatHandler::transform_streaming_chunk(chunk, "gpt-4");
        assert!(result.is_ok());

        let chat_chunk = result.unwrap();
        assert!(chat_chunk.choices.is_empty());
    }
}
