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
use crate::utils::net::http::{create_custom_client, create_streaming_client};

/// Azure OpenAI chat handler
#[derive(Debug, Clone)]
pub struct AzureChatHandler {
    config: AzureConfig,
    client: reqwest::Client,
    streaming_client: reqwest::Client,
}

impl AzureChatHandler {
    /// Create new chat handler
    pub fn new(config: AzureConfig) -> Result<Self, ProviderError> {
        let client = create_custom_client(ProviderConfig::timeout(&config))
            .map_err(|e| azure_config_error(format!("Failed to create HTTP client: {}", e)))?;
        let streaming_client = create_streaming_client().map_err(|e| {
            azure_config_error(format!("Failed to create streaming HTTP client: {}", e))
        })?;

        Ok(Self {
            config,
            client,
            streaming_client,
        })
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
            let error_body =
                crate::core::providers::base::connection_pool::read_streaming_error_body(response)
                    .await
                    .map_err(|err| err.into_provider_error("azure"))?;
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
        let response = crate::core::providers::base::connection_pool::send_streaming_request(
            apply_headers(
                self.streaming_client.post(&url).json(&azure_request),
                headers,
            ),
            "azure",
        )
        .await?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body =
                crate::core::providers::base::connection_pool::read_streaming_error_body(response)
                    .await
                    .map_err(|err| err.into_provider_error("azure"))?;
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
                                if let Ok(chunk_json) = serde_json::from_str::<Value>(data)
                                    && let Ok(chunk) = Self::transform_streaming_chunk(chunk_json, &deployment_clone) {
                                        yield Ok(chunk);
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
        if let Some(max_completion_tokens) = request.max_completion_tokens {
            body["max_completion_tokens"] = json!(max_completion_tokens);
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
                MessageRole::Developer => "developer",
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
                        audio: None,
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
                        audio: None,
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
#[path = "chat_tests.rs"]
mod tests;
