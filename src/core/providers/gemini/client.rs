//! Gemini client for Google AI Studio and Vertex AI endpoints.

use std::time::Duration;

use reqwest::Response;
use serde_json::{Value, json};
use tokio::time::timeout;

use crate::core::providers::GeminiNativeRequest;
use crate::core::providers::base::{
    BaseConfig, BaseHttpClient, HeaderPair, apply_provider_headers, header, header_owned,
    header_static, read_streaming_error_body,
};
use crate::core::providers::google::tool_loop::{
    GoogleToolPlanner, build_tool_config, build_tool_declarations, candidate_index,
    content_has_tool_part, content_has_tool_use, finish_reason, parse_function_call_parts,
};
use crate::core::providers::shared::{
    strict_direct_gemini_usage_metadata, strict_vertex_usage_metadata,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    chat::ChatMessage,
    chat::ChatRequest,
    content::ContentPart,
    message::MessageContent,
    message::MessageRole,
    responses::{ChatChoice, ChatResponse},
};

use super::config::GeminiConfig;
use super::error::{
    GeminiErrorMapper, gemini_multimodal_error, gemini_network_error, gemini_parse_error,
};
use super::streaming::GeminiUsagePolicy;

/// Gemini API client
#[derive(Debug, Clone)]
pub struct GeminiClient {
    config: GeminiConfig,
    http_client: BaseHttpClient,
    streaming_client: BaseHttpClient,
}

impl GeminiClient {
    pub(crate) fn api_key(&self) -> &str {
        self.config.api_key.as_deref().unwrap_or_default()
    }

    /// Create
    pub fn new(config: GeminiConfig) -> Result<Self, ProviderError> {
        config
            .validate_policy_client_settings()
            .map_err(|error| ProviderError::configuration("gemini", error))?;
        let base_config = BaseConfig {
            api_base: Some(config.base_url.clone()),
            endpoint_access: config.endpoint_access,
            timeout: config.request_timeout,
            ..Default::default()
        };
        let http_client = BaseHttpClient::new_for_provider("gemini", base_config.clone())?;
        let streaming_client = BaseHttpClient::new_for_provider_streaming("gemini", base_config)?;

        Ok(Self {
            config,
            http_client,
            streaming_client,
        })
    }

    pub(crate) async fn send_native_request(
        &self,
        request: &GeminiNativeRequest,
    ) -> Result<Response, ProviderError> {
        let api_key = self.api_key();
        let url =
            crate::core::providers::gemini_native_url(&self.config.base_url, api_key, request)?;
        let client = if request.stream {
            &self.streaming_client
        } else {
            &self.http_client
        };
        let send = apply_provider_headers(
            client.post(url)?.json(&request.body),
            self.get_request_headers(),
        )
        .send();
        let response = timeout(Duration::from_secs(self.config.request_timeout), send)
            .await
            .map_err(|_| ProviderError::timeout("gemini_proxy", "Gemini response header timeout"))?
            .map_err(|error| crate::core::providers::gemini_transport_error(error.is_timeout()))?;
        Ok(response)
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
        let gemini_request = self.transform_chat_request(&request)?;
        let endpoint = "streamGenerateContent";
        let mut response = self
            .send_stream_request(&request.model, endpoint, gemini_request)
            .await?;
        response
            .extensions_mut()
            .insert(GeminiUsagePolicy::from_vertex_ai(self.config.use_vertex_ai));
        Ok(response)
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
            let request_bytes = serde_json::to_vec(&body)
                .map(|bytes| bytes.len())
                .unwrap_or(0);
            tracing::debug!(
                %url,
                request_bytes,
                "Gemini request prepared"
            );
        }

        let response = timeout(
            Duration::from_secs(self.config.request_timeout),
            apply_provider_headers(self.http_client.post(&url)?.json(&body), headers).send(),
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
            let request_bytes = serde_json::to_vec(&body)
                .map(|bytes| bytes.len())
                .unwrap_or(0);
            tracing::debug!(
                %url,
                request_bytes,
                "Gemini stream request prepared"
            );
        }

        let response = timeout(
            Duration::from_secs(self.config.request_timeout),
            apply_provider_headers(self.streaming_client.post(&url)?.json(&body), headers).send(),
        )
        .await
        .map_err(|_| gemini_network_error("Request timeout"))?
        .map_err(|e| gemini_network_error(format!("Network error: {}", e)))?;

        // Check
        let status = response.status();
        if !status.is_success() {
            // Request
            let error_text = read_streaming_error_body(response)
                .await
                .map_err(|error| error.into_provider_error("gemini"))?;
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
            let response_bytes = response_text.len();
            tracing::debug!("Gemini response status: {}", status);
            tracing::debug!(%status, response_bytes, "Gemini response received");
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
        let mut tool_planner = GoogleToolPlanner::new("gemini");

        // Collect system message parts for systemInstruction field
        let mut system_parts: Vec<Value> = Vec::new();
        for (message_index, message) in request.messages.iter().enumerate() {
            if message.role == MessageRole::System {
                if let Some(text) = message.content.as_ref() {
                    system_parts.push(json!({"text": text.to_string()}));
                }
                continue;
            }

            let content =
                self.transform_message_content(message_index, message, &mut tool_planner)?;
            let role = match message.role {
                MessageRole::System | MessageRole::Developer => {
                    // Gemini doesn't directly support system/developer role, need to convert to user message prefix
                    continue;
                }
                MessageRole::User => "user",
                MessageRole::Assistant => "model",
                MessageRole::Tool | MessageRole::Function => "user",
            };

            contents.push(json!({
                "role": role,
                "parts": content
            }));
        }

        let mut gemini_request = json!({
            "contents": contents
        });

        // Place system instructions in the dedicated systemInstruction field
        if !system_parts.is_empty() {
            gemini_request["systemInstruction"] = json!({"parts": system_parts});
        }

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

        let (tools, declaration_names) = build_tool_declarations("gemini", request)?;
        if let Some(tools) = tools {
            gemini_request["tools"] = tools;
        }
        if let Some(tool_config) = build_tool_config("gemini", request, &declaration_names)? {
            gemini_request["toolConfig"] = tool_config;
        }

        Ok(gemini_request)
    }

    /// Transform message content
    fn transform_message_content(
        &self,
        message_index: usize,
        message: &ChatMessage,
        tool_planner: &mut GoogleToolPlanner,
    ) -> Result<Vec<Value>, ProviderError> {
        if let Some(tool_result) = tool_planner.top_level_result(message)? {
            return Ok(vec![tool_result.to_wire_value()]);
        }
        if message
            .tool_calls
            .as_ref()
            .is_some_and(|calls| !calls.is_empty())
            && message.role != MessageRole::Assistant
        {
            return Err(ProviderError::invalid_request(
                "gemini",
                "tool_calls require assistant role",
            ));
        }
        if message.function_call.is_some() && message.role != MessageRole::Assistant {
            return Err(ProviderError::invalid_request(
                "gemini",
                "function_call requires assistant role",
            ));
        }
        if message
            .tool_calls
            .as_ref()
            .is_some_and(|calls| !calls.is_empty())
            && content_has_tool_part(&message.content)
        {
            return Err(ProviderError::invalid_request(
                "gemini",
                "tool_calls cannot be combined with tool content parts",
            ));
        }
        if message.function_call.is_some() && content_has_tool_use(&message.content) {
            return Err(ProviderError::invalid_request(
                "gemini",
                "function_call cannot be combined with tool_use content parts",
            ));
        }
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
                        ContentPart::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => {
                            parts.push(
                                tool_planner
                                    .content_tool_result(tool_use_id, content, *is_error)?
                                    .to_wire_value(),
                            );
                        }
                        ContentPart::ToolUse { id, name, input } => {
                            if message.role != MessageRole::Assistant {
                                return Err(ProviderError::invalid_request(
                                    "gemini",
                                    "tool_use content requires assistant role",
                                ));
                            }
                            parts.push(
                                tool_planner
                                    .content_tool_use(id, name, input)?
                                    .to_wire_value(),
                            );
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

        if let Some(tool_calls) = &message.tool_calls {
            for tool_part in tool_planner.top_level_calls(tool_calls)? {
                parts.push(tool_part.to_wire_value());
            }
        }
        if let Some(function_call) = &message.function_call {
            parts.push(
                tool_planner
                    .legacy_function_call(message_index, function_call)?
                    .to_wire_value(),
            );
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

            // Extract text content and function calls
            let mut text_parts = Vec::new();
            for part in content {
                if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                    text_parts.push(text);
                }
            }
            let choice_index = candidate_index("gemini", candidate, index)?;
            let tool_calls = parse_function_call_parts("gemini", content, choice_index)?;
            let message_content = text_parts.join("");

            let finish_reason = finish_reason(
                "gemini",
                candidate.get("finishReason").and_then(|r| r.as_str()),
                !tool_calls.is_empty(),
            )?;

            let msg_content = if message_content.is_empty() && !tool_calls.is_empty() {
                None
            } else {
                Some(MessageContent::Text(message_content))
            };

            choices.push(ChatChoice {
                index: choice_index,
                message: crate::core::types::chat::ChatMessage {
                    role: MessageRole::Assistant,
                    content: msg_content,
                    thinking: None,
                    audio: None,
                    name: None,
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: None,
                    function_call: None,
                },
                finish_reason: Some(finish_reason),
                logprobs: None,
            });
        }

        // Extract usage_stats
        let usage = response.get("usageMetadata").and_then(|metadata| {
            if self.config.use_vertex_ai {
                strict_vertex_usage_metadata(metadata)
            } else {
                strict_direct_gemini_usage_metadata(metadata)
            }
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
    use crate::core::providers::google::tool_loop::GoogleToolPlanner;
    use crate::core::types::tools::{FunctionDefinition, Tool, ToolCall, ToolChoice, ToolType};

    fn transform_parts(client: &GeminiClient, message: &ChatMessage) -> Vec<Value> {
        let mut planner = GoogleToolPlanner::new("gemini");
        client
            .transform_message_content(0, message, &mut planner)
            .unwrap()
    }

    fn weather_tool() -> Tool {
        Tool {
            tool_type: ToolType::Function,
            function: FunctionDefinition {
                name: "get_weather".to_string(),
                description: Some("Get weather".to_string()),
                parameters: Some(json!({
                    "type": "object",
                    "properties": {
                        "city": {"type": "string"}
                    }
                })),
            },
        }
    }

    fn weather_call() -> ToolCall {
        ToolCall {
            id: "call_weather_1".to_string(),
            tool_type: "function".to_string(),
            function: crate::core::types::tools::FunctionCall {
                name: "get_weather".to_string(),
                arguments: r#"{"city":"Paris"}"#.to_string(),
            },
        }
    }

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
            audio: None,
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
        };

        let parts = transform_parts(&client, &message);
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
            audio: None,
            name: None,
            tool_calls: None,
            tool_call_id: None,
            function_call: None,
        };

        let parts = transform_parts(&client, &message);
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0]["text"], "What's in this image?");
        assert!(parts[1].get("inlineData").is_some());
    }

    #[test]
    fn test_gemini_request_maps_tool_call_and_result_round_trip() {
        let client = GeminiClient::new(GeminiConfig::new_google_ai("test-key")).unwrap();
        let request = ChatRequest {
            model: "gemini-2.0-flash".to_string(),
            messages: vec![
                ChatMessage {
                    role: MessageRole::User,
                    content: Some(MessageContent::Text("weather?".to_string())),
                    ..Default::default()
                },
                ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text("checking".to_string())),
                    tool_calls: Some(vec![weather_call()]),
                    ..Default::default()
                },
                ChatMessage {
                    role: MessageRole::Tool,
                    tool_call_id: Some("call_weather_1".to_string()),
                    content: Some(MessageContent::Text("sunny".to_string())),
                    ..Default::default()
                },
            ],
            tools: Some(vec![weather_tool()]),
            tool_choice: Some(ToolChoice::Specific {
                choice_type: "function".to_string(),
                function: Some(crate::core::types::tools::FunctionChoice {
                    name: "get_weather".to_string(),
                }),
            }),
            ..Default::default()
        };

        let body = client.transform_chat_request(&request).unwrap();
        assert_eq!(
            body["tools"][0]["functionDeclarations"][0]["name"],
            "get_weather"
        );
        assert_eq!(
            body["toolConfig"]["functionCallingConfig"]["allowedFunctionNames"][0],
            "get_weather"
        );
        assert_eq!(body["contents"][1]["role"], "model");
        assert_eq!(
            body["contents"][1]["parts"][1]["functionCall"],
            json!({
                "id": "call_weather_1",
                "name": "get_weather",
                "args": {"city": "Paris"}
            })
        );
        assert_eq!(body["contents"][2]["role"], "user");
        assert_eq!(
            body["contents"][2]["parts"][0]["functionResponse"],
            json!({
                "name": "get_weather",
                "response": {"result": "sunny"}
            })
        );
    }

    #[test]
    fn test_gemini_request_rejects_unknown_tool_result_id() {
        let client = GeminiClient::new(GeminiConfig::new_google_ai("test-key")).unwrap();
        let request = ChatRequest {
            model: "gemini-2.0-flash".to_string(),
            messages: vec![ChatMessage {
                role: MessageRole::Tool,
                tool_call_id: Some("missing".to_string()),
                content: Some(MessageContent::Text("sunny".to_string())),
                ..Default::default()
            }],
            ..Default::default()
        };
        let err = client.transform_chat_request(&request).unwrap_err();
        assert!(matches!(err, ProviderError::InvalidRequest { .. }));
    }

    #[test]
    fn test_gemini_finish_reason_tool_calls() {
        let config = GeminiConfig::new_google_ai("test-key");
        let client = GeminiClient::new(config).unwrap();
        let request = ChatRequest {
            model: "gemini-2.0-flash".to_string(),
            ..Default::default()
        };

        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "get_weather",
                            "args": {"city": "Paris"}
                        }
                    }]
                },
                "finishReason": "STOP"
            }]
        });

        let result = client.transform_chat_response(response, &request).unwrap();
        let choice = result.choices.first().unwrap();
        assert_eq!(
            choice.finish_reason,
            Some(crate::core::types::responses::FinishReason::ToolCalls)
        );
        let tool_call = choice
            .message
            .tool_calls
            .as_ref()
            .and_then(|calls| calls.first())
            .unwrap();
        assert_eq!(tool_call.function.name, "get_weather");
        assert_eq!(tool_call.function.arguments, r#"{"city":"Paris"}"#);
    }

    #[test]
    fn test_gemini_usage_preserves_cache_and_thought_tokens() {
        let config = GeminiConfig::new_google_ai("test-key");
        let client = GeminiClient::new(config).unwrap();
        let request = ChatRequest {
            model: "gemini-2.0-flash".to_string(),
            ..Default::default()
        };

        let response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Hello"}]
                },
                "finishReason": "STOP"
            }],
            "usageMetadata": {
                "promptTokenCount": 100,
                "toolUsePromptTokenCount": 10,
                "candidatesTokenCount": 25,
                "totalTokenCount": 140,
                "cachedContentTokenCount": 12,
                "thoughtsTokenCount": 15
            }
        });

        let result = client.transform_chat_response(response, &request).unwrap();
        let usage = result.usage.unwrap();
        let prompt_details = usage.prompt_tokens_details.unwrap();
        assert_eq!(usage.prompt_tokens, 110);
        assert_eq!(usage.completion_tokens, 40);
        assert_eq!(usage.total_tokens, 150);
        assert_eq!(prompt_details.cached_tokens, Some(12));
        assert_eq!(prompt_details.cache_read_tokens, Some(12));
        assert!(usage.completion_tokens_details.is_none());
        assert_eq!(
            usage
                .thinking_usage
                .as_ref()
                .and_then(|thinking| thinking.thinking_tokens),
            Some(15)
        );
    }

    #[test]
    fn test_gemini_usage_fails_closed_and_saturates_after_raw_total() {
        let client = GeminiClient::new(GeminiConfig::new_google_ai("test-key")).unwrap();
        let request = ChatRequest {
            model: "gemini-2.0-flash".to_string(),
            ..Default::default()
        };
        let transform = |client: &GeminiClient, usage| {
            client
                .transform_chat_response(
                    json!({
                        "candidates": [{"content": {"parts": [{"text": "ok"}]}}],
                        "usageMetadata": usage
                    }),
                    &request,
                )
                .unwrap()
                .usage
        };
        for bad in [
            json!({"promptTokenCount": 2, "candidatesTokenCount": 1, "totalTokenCount": 4}),
            json!({"promptTokenCount": 2, "candidatesTokenCount": "1", "totalTokenCount": 3}),
            json!({"promptTokenCount": 0, "candidatesTokenCount": 0, "totalTokenCount": 0}),
            json!({"promptTokenCount": 2, "candidatesTokenCount": 1, "cachedContentTokenCount": 3, "totalTokenCount": 3}),
        ] {
            assert!(transform(&client, bad).is_none());
        }
        let usage = transform(
            &client,
            json!({
                "promptTokenCount": u64::MAX, "candidatesTokenCount": 0,
                "totalTokenCount": u64::MAX
            }),
        )
        .unwrap();
        assert_eq!(
            (usage.prompt_tokens, usage.total_tokens),
            (u32::MAX, u32::MAX)
        );
        let vertex = GeminiClient::new(GeminiConfig::new_vertex_ai("project", "location")).unwrap();
        let mut vertex_usage = json!({
            "promptTokenCount": 2, "toolUsePromptTokenCount": 1,
            "candidatesTokenCount": 1, "thoughtsTokenCount": 1, "totalTokenCount": 5
        });
        let usage = transform(&vertex, vertex_usage.clone()).unwrap();
        assert_eq!((usage.prompt_tokens, usage.completion_tokens), (3, 2));
        assert!(usage.thinking_usage.is_none());
        vertex_usage["totalTokenCount"] = json!(4);
        assert!(transform(&vertex, vertex_usage).is_none());
    }
}
