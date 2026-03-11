//! Gemini Client
//!
//! Error handling
//! Supports both Google AI Studio and Vertex AI endpoints

use std::time::Duration;

use reqwest::{Client, ClientBuilder, Response};
use serde_json::{Value, json};
use tokio::time::timeout;

use crate::core::providers::base::{
    HeaderPair, apply_headers, header, header_owned, header_static,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    content::ContentPart,
    message::MessageContent,
    message::MessageRole,
    responses::{ChatChoice, ChatResponse, Usage},
};

use super::config::GeminiConfig;
use super::error::{
    GeminiErrorMapper, gemini_multimodal_error, gemini_network_error, gemini_parse_error,
};

/// Gemini API client
#[derive(Debug, Clone)]
pub struct GeminiClient {
    config: GeminiConfig,
    http_client: Client,
}

impl GeminiClient {
    /// Create
    pub fn new(config: GeminiConfig) -> Result<Self, ProviderError> {
        let mut builder = ClientBuilder::new()
            .timeout(Duration::from_secs(config.request_timeout))
            .connect_timeout(Duration::from_secs(config.connect_timeout));

        // Configuration
        if let Some(proxy_url) = &config.proxy_url {
            let proxy = reqwest::Proxy::all(proxy_url)
                .map_err(|e| gemini_network_error(format!("Invalid proxy URL: {}", e)))?;
            builder = builder.proxy(proxy);
        }

        let http_client = builder
            .build()
            .map_err(|e| gemini_network_error(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            http_client,
        })
    }

    /// Request
    pub async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, ProviderError> {
        // Request
        let gemini_request = self.transform_chat_request(&request)?;

        // Request
        let endpoint = "generateContent";
        let response = self
            .send_request(&request.model, endpoint, gemini_request)
            .await?;

        // Response
        self.transform_chat_response(response, &request)
    }

    /// Request
    pub async fn chat_stream(
        &self,
        request: ChatRequest,
    ) -> Result<reqwest::Response, ProviderError> {
        // Request
        let gemini_request = self.transform_chat_request(&request)?;

        // Request
        let endpoint = "streamGenerateContent";
        self.send_stream_request(&request.model, endpoint, gemini_request)
            .await
    }

    /// Request
    async fn send_request(
        &self,
        model: &str,
        operation: &str,
        body: Value,
    ) -> Result<Value, ProviderError> {
        let url = self.config.get_endpoint(model, operation);
        let headers = self.get_request_headers();

        if self.config.debug {
            tracing::debug!("Gemini request URL: {}", url);
            tracing::debug!(
                "Gemini request body: {}",
                serde_json::to_string_pretty(&body).unwrap_or_default()
            );
        }

        let response = timeout(
            Duration::from_secs(self.config.request_timeout),
            apply_headers(self.http_client.post(&url).json(&body), headers).send(),
        )
        .await
        .map_err(|_| gemini_network_error("Request timeout"))?
        .map_err(|e| gemini_network_error(format!("Network error: {}", e)))?;

        self.handle_response(response).await
    }

    /// Request
    async fn send_stream_request(
        &self,
        model: &str,
        operation: &str,
        body: Value,
    ) -> Result<Response, ProviderError> {
        let url = self.config.get_endpoint(model, operation);
        let headers = self.get_request_headers();

        if self.config.debug {
            tracing::debug!("Gemini stream request URL: {}", url);
            tracing::debug!(
                "Gemini stream request body: {}",
                serde_json::to_string_pretty(&body).unwrap_or_default()
            );
        }

        let response = timeout(
            Duration::from_secs(self.config.request_timeout),
            apply_headers(self.http_client.post(&url).json(&body), headers).send(),
        )
        .await
        .map_err(|_| gemini_network_error("Request timeout"))?
        .map_err(|e| gemini_network_error(format!("Network error: {}", e)))?;

        // Check
        let status = response.status();
        if !status.is_success() {
            // Request
            let error_text = response.text().await.map_err(|e| {
                gemini_network_error(format!("Failed to read error response: {}", e))
            })?;
            return Err(GeminiErrorMapper::from_http_status(
                status.as_u16(),
                &error_text,
            ));
        }

        Ok(response)
    }

    /// Build request headers using the unified HeaderPair pattern.
    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(4);
        headers.push(header_static("Content-Type", "application/json"));

        // Vertex AI uses Bearer token, Google AI Studio uses API key as query parameter
        if self.config.use_vertex_ai
            && let Some(api_key) = &self.config.api_key
        {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        // Add custom headers
        for (key, value) in &self.config.custom_headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    }

    /// Handle
    async fn handle_response(&self, response: Response) -> Result<Value, ProviderError> {
        let status = response.status();
        let response_text = response
            .text()
            .await
            .map_err(|e| gemini_network_error(format!("Failed to read response: {}", e)))?;

        if self.config.debug {
            tracing::debug!("Gemini response status: {}", status);
            tracing::debug!("Gemini response body: {}", response_text);
        }

        if !status.is_success() {
            return Err(GeminiErrorMapper::from_http_status(
                status.as_u16(),
                &response_text,
            ));
        }

        // Response
        let json_response: Value = serde_json::from_str(&response_text)
            .map_err(|e| gemini_parse_error(format!("Failed to parse response JSON: {}", e)))?;

        // Error
        if json_response.get("error").is_some() {
            return Err(GeminiErrorMapper::from_api_response(&json_response));
        }

        Ok(json_response)
    }

    /// Request
    pub fn transform_chat_request(&self, request: &ChatRequest) -> Result<Value, ProviderError> {
        let mut contents = Vec::new();

        for message in &request.messages {
            let content = self.transform_message_content(message)?;
            let role = match message.role {
                MessageRole::System => {
                    // Gemini doesn't directly support system role, need to convert to user message prefix
                    continue;
                }
                MessageRole::User => "user",
                MessageRole::Assistant => "model",
                MessageRole::Tool => "function", // Function call result
                MessageRole::Function => "function", // Function call result
            };

            contents.push(json!({
                "role": role,
                "parts": content
            }));
        }

        // Handle
        if let Some(system_msg) = request
            .messages
            .iter()
            .find(|m| m.role == MessageRole::System)
            && let Some(system_text) = system_msg.content.as_ref()
            && let Some(first_content) = contents.first_mut()
            && let Some(parts) = first_content
                .get_mut("parts")
                .and_then(|p| p.as_array_mut())
        {
            parts.insert(0, json!({"text": format!("System: {}", system_text)}));
        }

        let mut gemini_request = json!({
            "contents": contents
        });

        // Configuration
        let mut generation_config = json!({});

        if let Some(max_tokens) = request.max_tokens {
            generation_config["maxOutputTokens"] = json!(max_tokens);
        }

        if let Some(temperature) = request.temperature {
            generation_config["temperature"] = json!(temperature);
        }

        if let Some(top_p) = request.top_p {
            generation_config["topP"] = json!(top_p);
        }

        if let Some(stop) = &request.stop {
            let stop_sequences = stop.clone();
            if !stop_sequences.is_empty() {
                generation_config["stopSequences"] = json!(stop_sequences);
            }
        }

        // Only add generationConfig if it has values (safely check if object is non-empty)
        if generation_config
            .as_object()
            .is_some_and(|obj| !obj.is_empty())
        {
            gemini_request["generationConfig"] = generation_config;
        }

        // Settings
        if let Some(safety_settings) = &self.config.safety_settings {
            let gemini_safety: Vec<Value> = safety_settings
                .iter()
                .map(|setting| {
                    json!({
                        "category": setting.category,
                        "threshold": setting.threshold
                    })
                })
                .collect();
            gemini_request["safetySettings"] = json!(gemini_safety);
        }

        Ok(gemini_request)
    }

    /// Transform message content
    fn transform_message_content(
        &self,
        message: &ChatMessage,
    ) -> Result<Vec<Value>, ProviderError> {
        let mut parts = Vec::new();

        match &message.content {
            Some(MessageContent::Text(text)) => {
                parts.push(json!({
                    "text": text
                }));
            }
            Some(MessageContent::Parts(content_parts)) => {
                // Handle
                for part in content_parts {
                    match part {
                        ContentPart::Text { text } => {
                            parts.push(json!({
                                "text": text
                            }));
                        }
                        ContentPart::ImageUrl { image_url } => {
                            // Gemini supports inline image data
                            if image_url.url.starts_with("data:") {
                                // parsedata URL
                                if let Some((mime_type, data)) =
                                    self.parse_data_url(&image_url.url)?
                                {
                                    parts.push(json!({
                                        "inlineData": {
                                            "mimeType": mime_type,
                                            "data": data
                                        }
                                    }));
                                }
                            } else {
                                // External image URL - Gemini doesn't support directly, need to download first
                                return Err(gemini_multimodal_error(
                                    "External image URLs not supported directly. Please convert to base64 data URL",
                                ));
                            }
                        }
                        ContentPart::Audio { .. } => {
                            return Err(gemini_multimodal_error(
                                "Audio content not yet implemented",
                            ));
                        }
                        ContentPart::Image { source, .. } => {
                            // Handle
                            parts.push(json!({
                                "inlineData": {
                                    "mimeType": source.media_type,
                                    "data": source.data
                                }
                            }));
                        }
                        ContentPart::Document { .. } => {
                            return Err(gemini_multimodal_error(
                                "Document content not yet supported in Gemini",
                            ));
                        }
                        ContentPart::ToolResult { .. } => {
                            return Err(gemini_multimodal_error(
                                "Tool result content should be handled separately",
                            ));
                        }
                        ContentPart::ToolUse { .. } => {
                            return Err(gemini_multimodal_error(
                                "Tool use content should be handled separately",
                            ));
                        }
                    }
                }
            }
            None => {
                // Plain text message
                if let Some(content) = &message.content {
                    parts.push(json!({
                        "text": content
                    }));
                }
            }
        }

        if parts.is_empty() {
            parts.push(json!({
                "text": ""
            }));
        }

        Ok(parts)
    }

    /// parsedata URL
    fn parse_data_url(&self, data_url: &str) -> Result<Option<(String, String)>, ProviderError> {
        if !data_url.starts_with("data:") {
            return Ok(None);
        }

        let parts: Vec<&str> = data_url.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(gemini_parse_error("Invalid data URL format"));
        }

        let header = parts[0];
        let data = parts[1];

        // Parse MIME type
        let mime_parts: Vec<&str> = header.split(';').collect();
        let mime_type = mime_parts[0]
            .strip_prefix("data:")
            .unwrap_or("application/octet-stream");

        Ok(Some((mime_type.to_string(), data.to_string())))
    }

    /// Response
    pub fn transform_chat_response(
        &self,
        response: Value,
        request: &ChatRequest,
    ) -> Result<ChatResponse, ProviderError> {
        let candidates = response
            .get("candidates")
            .and_then(|c| c.as_array())
            .ok_or_else(|| gemini_parse_error("No candidates in response"))?;

        let mut choices = Vec::new();

        for (index, candidate) in candidates.iter().enumerate() {
            let content = candidate
                .get("content")
                .and_then(|c| c.get("parts"))
                .and_then(|p| p.as_array())
                .ok_or_else(|| gemini_parse_error("Invalid candidate content structure"))?;

            // Extract text content
            let mut text_parts = Vec::new();
            for part in content {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text);
                }
            }
            let message_content = text_parts.join("");

            // Check
            let finish_reason = candidate
                .get("finishReason")
                .and_then(|r| r.as_str())
                .map(|r| match r {
                    "STOP" => "stop",
                    "MAX_TOKENS" => "length",
                    "SAFETY" => "content_filter",
                    "RECITATION" => "content_filter",
                    _ => "stop",
                })
                .unwrap_or("stop");

            choices.push(ChatChoice {
                index: index as u32,
                message: crate::core::types::chat::ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text(message_content)),
                    thinking: None,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    function_call: None,
                },
                finish_reason: Some(match finish_reason {
                    "stop" => crate::core::types::responses::FinishReason::Stop,
                    "length" => crate::core::types::responses::FinishReason::Length,
                    "content_filter" => crate::core::types::responses::FinishReason::ContentFilter,
                    _ => crate::core::types::responses::FinishReason::Stop,
                }),
                logprobs: None,
            });
        }

        // Extract usage_stats
        let usage = response.get("usageMetadata").map(|usage_metadata| Usage {
            prompt_tokens: usage_metadata
                .get("promptTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: usage_metadata
                .get("candidatesTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: usage_metadata
                .get("totalTokenCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        });

        // Use current timestamp, defaulting to 0 if system time is before UNIX_EPOCH
        let now = std::time::SystemTime::now();
        let nanos = now
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let secs = now
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Ok(ChatResponse {
            id: format!("gemini-{}", nanos),
            object: "chat.completion".to_string(),
            created: secs,
            model: request.model.clone(),
            choices,
            usage,
            system_fingerprint: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = GeminiConfig::new_google_ai("test-key");
        let client = GeminiClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_data_url_parsing() {
        let config = GeminiConfig::new_google_ai("test-key");
        let client = GeminiClient::new(config).unwrap();

        let data_url = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        let result = client.parse_data_url(data_url).unwrap();

        assert!(result.is_some());
        let (mime_type, _data) = result.unwrap();
        assert_eq!(mime_type, "image/png");
    }

    #[test]
    fn test_message_transformation() {
        let config = GeminiConfig::new_google_ai("test-key");
        let client = GeminiClient::new(config).unwrap();

        let message = ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello, world!".to_string())),
            thinking: None,
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
        };

        let parts = client.transform_message_content(&message).unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["text"], "Hello, world!");
    }

    #[test]
    fn test_multimodal_message() {
        let config = GeminiConfig::new_google_ai("test-key");
        let client = GeminiClient::new(config).unwrap();

        let message = ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Parts(vec![
                ContentPart::Text {
                    text: "What's in this image?".to_string(),
                },
                ContentPart::Image {
                    source: crate::core::types::content::ImageSource {
                        data: "test".to_string(),
                        media_type: "image/png".to_string(),
                    },
                    image_url: None,
                    detail: None,
                },
            ])),
            thinking: None,
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
        };

        let parts = client.transform_message_content(&message).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["text"], "What's in this image?");
        assert!(parts[1].get("inlineData").is_some());
    }
}
