//! Chat completion methods

use super::llm_client::LLMClient;
use crate::sdk::{errors::*, types::*};
use std::time::SystemTime;
use tracing::{debug, error};

impl LLMClient {
    /// Send chat message (using load balancing)
    pub async fn chat(&self, messages: Vec<Message>) -> Result<ChatResponse> {
        let request = SdkChatRequest {
            model: String::new(), // Will be set by load balancer
            messages,
            options: ChatOptions::default(),
        };

        self.chat_with_options(request).await
    }

    /// Send chat message (with options)
    pub async fn chat_with_options(&self, request: SdkChatRequest) -> Result<ChatResponse> {
        let start_time = SystemTime::now();

        // Select best provider
        let provider = self.select_provider(&request).await?;

        // Execute request
        let result = self.execute_chat_request(&provider.id, request).await;

        // Update statistics
        self.update_provider_stats(&provider.id, start_time, &result)
            .await;

        result
    }

    /// Streaming chat
    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
    ) -> Result<impl futures::Stream<Item = Result<ChatChunk>>> {
        let provider = self.select_provider_for_stream(&messages).await?;
        self.execute_stream_request(&provider.id, messages).await
    }

    /// Execute chat request with a specific provider
    pub(crate) async fn execute_chat_request(
        &self,
        provider_id: &str,
        request: SdkChatRequest,
    ) -> Result<ChatResponse> {
        let provider = self
            .config
            .providers
            .iter()
            .find(|p| p.id == provider_id)
            .ok_or_else(|| SDKError::ProviderNotFound(provider_id.to_string()))?;

        debug!("Executing chat request with provider: {}", provider_id);

        match provider.provider_type {
            crate::sdk::config::ProviderType::Anthropic => {
                self.call_anthropic_api(provider, request).await
            }
            crate::sdk::config::ProviderType::OpenAI => {
                self.call_openai_api(provider, request).await
            }
            crate::sdk::config::ProviderType::Google => {
                self.call_google_api(provider, request).await
            }
            _ => Err(SDKError::ProviderError(format!(
                "Provider type {:?} is not implemented in SDK client",
                provider.provider_type
            ))),
        }
    }

    /// Execute stream request
    pub(crate) async fn execute_stream_request(
        &self,
        provider_id: &str,
        _messages: Vec<Message>,
    ) -> Result<impl futures::Stream<Item = Result<ChatChunk>>> {
        let provider = self
            .config
            .providers
            .iter()
            .find(|p| p.id == provider_id)
            .ok_or_else(|| SDKError::ProviderNotFound(provider_id.to_string()))?;

        Err::<futures::stream::Empty<Result<ChatChunk>>, _>(SDKError::ProviderError(format!(
            "Streaming is not implemented for provider type {:?}",
            provider.provider_type
        )))
    }

    /// Call Anthropic API
    async fn call_anthropic_api(
        &self,
        provider: &crate::sdk::config::SdkProviderConfig,
        request: SdkChatRequest,
    ) -> Result<ChatResponse> {
        // Convert message format
        let (system_message, anthropic_messages) =
            self.convert_messages_to_anthropic(&request.messages)?;

        // Build request body
        let mut body = serde_json::json!({
            "model": provider.models.first().unwrap_or(&"claude-sonnet-4-5".to_string()),
            "messages": anthropic_messages,
            "max_tokens": request.options.max_tokens.unwrap_or(1000)
        });

        if let Some(system) = system_message {
            body["system"] = serde_json::json!(system);
        }

        if let Some(temp) = request.options.temperature {
            body["temperature"] = serde_json::json!(temp);
        }

        if let Some(top_p) = request.options.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }

        // Send request
        let default_url = "https://api.anthropic.com".to_string();
        let base_url = provider.base_url.as_ref().unwrap_or(&default_url);
        let url = if base_url.contains("/v1") {
            format!("{}/messages", base_url.trim_end_matches('/'))
        } else {
            format!("{}/v1/messages", base_url.trim_end_matches('/'))
        };

        debug!("Calling Anthropic API: {}", url);

        let response = self
            .http_client
            .post(&url)
            .header("x-api-key", &provider.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SDKError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            error!("Anthropic API error: {} - {}", status, error_text);
            return Err(SDKError::ApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        let anthropic_response: serde_json::Value = response
            .json()
            .await
            .map_err(|e| SDKError::ParseError(e.to_string()))?;

        // Convert response
        self.convert_anthropic_response(
            anthropic_response,
            provider
                .models
                .first()
                .unwrap_or(&"claude-sonnet-4-5".to_string()),
        )
    }

    /// Call OpenAI API
    async fn call_openai_api(
        &self,
        provider: &crate::sdk::config::SdkProviderConfig,
        request: SdkChatRequest,
    ) -> Result<ChatResponse> {
        let body = serde_json::json!({
            "model": provider.models.first().unwrap_or(&"gpt-5.2-chat".to_string()),
            "messages": request.messages,
            "max_tokens": request.options.max_tokens.unwrap_or(1000),
            "temperature": request.options.temperature.unwrap_or(0.7),
            "stream": false
        });

        let default_url = "https://api.openai.com".to_string();
        let base_url = provider.base_url.as_ref().unwrap_or(&default_url);
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

        debug!("Calling OpenAI API: {}", url);

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", provider.api_key))
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SDKError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(SDKError::ApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        // Parse response
        let openai_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| SDKError::ParseError(e.to_string()))?;

        Ok(openai_response)
    }

    /// Call Google API
    async fn call_google_api(
        &self,
        provider: &crate::sdk::config::SdkProviderConfig,
        _request: SdkChatRequest,
    ) -> Result<ChatResponse> {
        Err(SDKError::ProviderError(format!(
            "Provider '{}' (Google) is not implemented in SDK client",
            provider.id
        )))
    }

    /// Convert messages to Anthropic format
    fn convert_messages_to_anthropic(
        &self,
        messages: &[Message],
    ) -> Result<(Option<String>, Vec<serde_json::Value>)> {
        let mut system_message = None;
        let mut anthropic_messages = Vec::new();

        for message in messages {
            match message.role {
                Role::System => {
                    if let Some(Content::Text(text)) = &message.content {
                        system_message = Some(text.clone());
                    }
                }
                Role::User => {
                    anthropic_messages.push(serde_json::json!({
                        "role": "user",
                        "content": self.convert_content_to_anthropic(message.content.as_ref())?
                    }));
                }
                Role::Assistant => {
                    anthropic_messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": self.convert_content_to_anthropic(message.content.as_ref())?
                    }));
                }
                _ => {} // Ignore other roles
            }
        }

        Ok((system_message, anthropic_messages))
    }

    /// Convert content to Anthropic format
    fn convert_content_to_anthropic(&self, content: Option<&Content>) -> Result<serde_json::Value> {
        match content {
            Some(Content::Text(text)) => Ok(serde_json::json!(text)),
            Some(Content::Multimodal(parts)) => {
                let mut anthropic_content = Vec::new();
                for part in parts {
                    match part {
                        ContentPart::Text { text } => {
                            anthropic_content.push(serde_json::json!({
                                "type": "text",
                                "text": text
                            }));
                        }
                        ContentPart::Image { image_url } => {
                            // Parse data URI: "data:<mime>[;<param>...];base64,<data>"
                            // Split on the first comma to isolate the header from the payload,
                            // then extract the MIME type from the header's first semicolon-delimited
                            // segment. This correctly handles URIs with extra params such as
                            // "data:image/png;name=foo;base64,..." or
                            // "data:image/svg+xml;charset=utf-8;base64,..."
                            if let Some(rest) = image_url.url.strip_prefix("data:") {
                                let comma_pos = rest.find(',').ok_or_else(|| {
                                    SDKError::InvalidRequest(format!(
                                        "malformed data URI (no comma separator): {}",
                                        &image_url.url
                                    ))
                                })?;
                                let header = &rest[..comma_pos];
                                let data = &rest[comma_pos + 1..];
                                let mime = header
                                    .split(';')
                                    .next()
                                    .filter(|s| !s.is_empty())
                                    .unwrap_or("image/jpeg");
                                anthropic_content.push(serde_json::json!({
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": mime,
                                        "data": data
                                    }
                                }));
                            } else {
                                // Plain URLs are not supported for Anthropic image content;
                                // the provider requires base64-encoded data URIs.
                                return Err(SDKError::InvalidRequest(
                                    "URL images are not supported for Anthropic; use a base64 data URI instead".to_string(),
                                ));
                            }
                        }
                        _ => {} // Ignore other types
                    }
                }
                Ok(serde_json::json!(anthropic_content))
            }
            None => Ok(serde_json::json!("")),
        }
    }

    /// Convert Anthropic response to standard format
    fn convert_anthropic_response(
        &self,
        anthropic_response: serde_json::Value,
        model: &str,
    ) -> Result<ChatResponse> {
        let id = anthropic_response
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("chatcmpl-anthropic")
            .to_string();

        let content = anthropic_response
            .get("content")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|item| item.get("text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let usage = if let Some(u) = anthropic_response.get("usage") {
            Usage {
                prompt_tokens: u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                completion_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                    as u32,
                total_tokens: 0, // Will be calculated below
            }
        } else {
            Usage::default()
        };

        let mut usage = usage;
        usage.total_tokens = usage.prompt_tokens + usage.completion_tokens;

        Ok(ChatResponse {
            id,
            model: model.to_string(),
            choices: vec![ChatChoice {
                index: 0,
                message: Message {
                    role: Role::Assistant,
                    content: Some(Content::Text(content)),
                    name: None,
                    tool_calls: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage,
            created: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sdk::{
        config::{ClientConfig, ClientSettings, ProviderType, SdkProviderConfig},
        types::{Content, ContentPart, ImageUrl},
    };
    use std::collections::HashMap;

    fn make_client() -> LLMClient {
        let config = ClientConfig {
            default_provider: None,
            providers: vec![SdkProviderConfig {
                id: "test".to_string(),
                provider_type: ProviderType::OpenAI,
                name: "Test".to_string(),
                api_key: "sk-test".to_string(),
                base_url: None,
                models: vec!["gpt-4o".to_string()],
                enabled: true,
                weight: 1.0,
                rate_limit_rpm: None,
                rate_limit_tpm: None,
                settings: HashMap::new(),
            }],
            settings: ClientSettings::default(),
        };
        LLMClient::new(config).expect("client creation failed")
    }

    fn image_content(data_uri: &str) -> Content {
        Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: data_uri.to_string(),
                detail: None,
            },
        }])
    }

    #[test]
    fn test_jpeg_data_uri() {
        let client = make_client();
        let content = image_content("data:image/jpeg;base64,/9j/abc123");
        let val = client.convert_content_to_anthropic(Some(&content)).unwrap();
        let source = &val[0]["source"];
        assert_eq!(source["media_type"], "image/jpeg");
        assert_eq!(source["data"], "/9j/abc123");
    }

    #[test]
    fn test_png_data_uri() {
        let client = make_client();
        let content = image_content("data:image/png;base64,iVBORw==");
        let val = client.convert_content_to_anthropic(Some(&content)).unwrap();
        let source = &val[0]["source"];
        assert_eq!(source["media_type"], "image/png");
        assert_eq!(source["data"], "iVBORw==");
    }

    #[test]
    fn test_webp_data_uri() {
        let client = make_client();
        let content = image_content("data:image/webp;base64,UklGR==");
        let val = client.convert_content_to_anthropic(Some(&content)).unwrap();
        let source = &val[0]["source"];
        assert_eq!(source["media_type"], "image/webp");
        assert_eq!(source["data"], "UklGR==");
    }

    #[test]
    fn test_gif_data_uri() {
        let client = make_client();
        let content = image_content("data:image/gif;base64,R0lGOD==");
        let val = client.convert_content_to_anthropic(Some(&content)).unwrap();
        let source = &val[0]["source"];
        assert_eq!(source["media_type"], "image/gif");
        assert_eq!(source["data"], "R0lGOD==");
    }

    #[test]
    fn test_malformed_data_uri_returns_error() {
        let client = make_client();
        let content = image_content("data:image/png;base64");
        let err = client
            .convert_content_to_anthropic(Some(&content))
            .unwrap_err();
        assert!(matches!(err, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_plain_url_returns_error() {
        let client = make_client();
        let content = image_content("https://example.com/image.png");
        let err = client
            .convert_content_to_anthropic(Some(&content))
            .unwrap_err();
        assert!(matches!(err, SDKError::InvalidRequest(_)));
    }
}
