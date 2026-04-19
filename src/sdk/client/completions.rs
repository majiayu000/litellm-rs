//! Chat completion methods

use super::llm_client::LLMClient;
use super::provider_payloads::{
    build_anthropic_request_body, build_openai_request_body, convert_anthropic_response,
    convert_messages_to_anthropic,
};
use crate::sdk::{errors::*, types::*};
use futures::StreamExt;
use serde::de::DeserializeOwned;
use std::collections::VecDeque;
use std::pin::Pin;
use std::time::SystemTime;
use tracing::{debug, error};

async fn api_error_from_response(response: reqwest::Response) -> SDKError {
    let status = response.status();
    let error_text = response.text().await.unwrap_or_default();
    SDKError::ApiError(format!("HTTP {}: {}", status, error_text))
}

async fn send_json_request(
    request_builder: reqwest::RequestBuilder,
    body: &serde_json::Value,
) -> Result<reqwest::Response> {
    request_builder
        .header("content-type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| SDKError::NetworkError(e.to_string()))
}

async fn parse_json_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    response
        .json()
        .await
        .map_err(|e| SDKError::ParseError(e.to_string()))
}

impl LLMClient {
    /// Send chat message (using load balancing)
    pub async fn chat(&self, messages: Vec<Message>) -> Result<ChatResponse> {
        let request = SdkChatRequest {
            model: String::new(),
            messages,
            options: ChatOptions::default(),
        };

        self.chat_with_options(request).await
    }

    /// Send chat message (with options)
    pub async fn chat_with_options(&self, request: SdkChatRequest) -> Result<ChatResponse> {
        let start_time = SystemTime::now();
        let provider = self.select_provider(&request).await?;
        let result = self.execute_chat_request(&provider.id, request).await;

        self.update_provider_stats(&provider.id, start_time, &result)
            .await;

        result
    }

    /// Streaming chat
    pub async fn chat_stream(
        &self,
        messages: Vec<Message>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<ChatChunk>> + Send>>> {
        let provider = self.select_provider_for_stream(&messages).await?;
        self.execute_stream_request(&provider.id, messages).await
    }

    /// Execute chat request with a specific provider
    pub(crate) async fn execute_chat_request(
        &self,
        provider_id: &str,
        request: SdkChatRequest,
    ) -> Result<ChatResponse> {
        let provider = self.provider_config(provider_id)?;

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
        messages: Vec<Message>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<ChatChunk>> + Send>>> {
        let provider = self.provider_config(provider_id)?;

        match provider.provider_type {
            crate::sdk::config::ProviderType::OpenAI | crate::sdk::config::ProviderType::Ollama => {
                self.call_openai_stream_api(provider, messages).await
            }
            crate::sdk::config::ProviderType::Anthropic => {
                self.call_anthropic_stream_api(provider, messages).await
            }
            _ => Err(SDKError::NotSupported(format!(
                "Streaming not supported for provider type {:?}",
                provider.provider_type
            ))),
        }
    }

    async fn call_openai_stream_api(
        &self,
        provider: &crate::sdk::config::SdkProviderConfig,
        messages: Vec<Message>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<ChatChunk>> + Send>>> {
        let body = serde_json::json!({
            "model": provider.models.first().unwrap_or(&"gpt-4".to_string()),
            "messages": messages,
            "stream": true,
        });

        let default_url = "https://api.openai.com".to_string();
        let base_url = provider.base_url.as_ref().unwrap_or(&default_url);
        let base = base_url.trim_end_matches('/');
        let url = if base.contains("/v1") {
            format!("{}/chat/completions", base)
        } else {
            format!("{}/v1/chat/completions", base)
        };

        debug!("Calling OpenAI stream API: {}", url);

        let response = self
            .stream_http_client
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

        let byte_stream = response.bytes_stream();

        let s = futures::stream::unfold(
            (
                byte_stream,
                Vec::<u8>::new(),
                VecDeque::<Result<ChatChunk>>::new(),
                false,
            ),
            |(mut byte_stream, mut buffer, mut pending, mut done)| async move {
                loop {
                    if let Some(item) = pending.pop_front() {
                        return Some((item, (byte_stream, buffer, pending, done)));
                    }

                    if done {
                        return None;
                    }

                    if let Some((pos, delim_len)) = find_sse_record_end_bytes(&buffer) {
                        let record_bytes = buffer[..pos].to_vec();
                        buffer.drain(..pos + delim_len);
                        match String::from_utf8(record_bytes) {
                            Ok(record) => {
                                for line in record.lines() {
                                    match parse_openai_sse_line(line) {
                                        Some(Ok(chunk)) => pending.push_back(Ok(chunk)),
                                        Some(Err(e)) => {
                                            done = true;
                                            pending.push_back(Err(e));
                                            break;
                                        }
                                        None => {}
                                    }
                                }
                            }
                            Err(_) => {
                                done = true;
                                pending.push_back(Err(SDKError::ParseError(
                                    "SSE record contained invalid UTF-8".to_string(),
                                )));
                            }
                        }
                        continue;
                    }

                    match byte_stream.next().await {
                        Some(Ok(bytes)) => {
                            buffer.extend_from_slice(&bytes);
                        }
                        Some(Err(e)) => {
                            return Some((
                                Err(SDKError::NetworkError(e.to_string())),
                                (byte_stream, buffer, pending, true),
                            ));
                        }
                        None => {
                            done = true;
                            let remaining = std::mem::take(&mut buffer);
                            if !remaining.is_empty() {
                                let remaining_str =
                                    String::from_utf8_lossy(&remaining).into_owned();
                                for line in remaining_str.lines() {
                                    match parse_openai_sse_line(line) {
                                        Some(Ok(chunk)) => pending.push_back(Ok(chunk)),
                                        Some(Err(e)) => {
                                            pending.push_back(Err(e));
                                            break;
                                        }
                                        None => {}
                                    }
                                }
                            }
                        }
                    }
                }
            },
        );

        Ok(Box::pin(s))
    }

    async fn call_anthropic_stream_api(
        &self,
        provider: &crate::sdk::config::SdkProviderConfig,
        messages: Vec<Message>,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<ChatChunk>> + Send>>> {
        let (system_message, anthropic_messages) = convert_messages_to_anthropic(&messages)?;

        let mut body = serde_json::json!({
            "model": provider.models.first().unwrap_or(&"claude-sonnet-4-5".to_string()),
            "messages": anthropic_messages,
            "max_tokens": 1000,
            "stream": true,
        });

        if let Some(system) = system_message {
            body["system"] = serde_json::json!(system);
        }

        let default_url = "https://api.anthropic.com".to_string();
        let base_url = provider.base_url.as_ref().unwrap_or(&default_url);
        let url = if base_url.contains("/v1") {
            format!("{}/messages", base_url.trim_end_matches('/'))
        } else {
            format!("{}/v1/messages", base_url.trim_end_matches('/'))
        };

        debug!("Calling Anthropic stream API: {}", url);

        let response = self
            .stream_http_client
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
            error!("Anthropic stream API error: {} - {}", status, error_text);
            return Err(SDKError::ApiError(format!(
                "HTTP {}: {}",
                status, error_text
            )));
        }

        let byte_stream = response.bytes_stream();

        let s = futures::stream::unfold(
            (
                byte_stream,
                Vec::<u8>::new(),
                VecDeque::<Result<ChatChunk>>::new(),
                false,
                Option::<(String, String)>::None,
            ),
            |(mut byte_stream, mut buffer, mut pending, mut done, mut current_tool)| async move {
                loop {
                    if let Some(item) = pending.pop_front() {
                        return Some((item, (byte_stream, buffer, pending, done, current_tool)));
                    }

                    if done {
                        return None;
                    }

                    if let Some((pos, delim_len)) = find_sse_record_end_bytes(&buffer) {
                        let record_bytes = buffer[..pos].to_vec();
                        buffer.drain(..pos + delim_len);
                        match String::from_utf8(record_bytes) {
                            Ok(record) => {
                                let mut event_line = None;
                                let mut data_line = None;
                                for l in record.lines() {
                                    if let Some(ev) = l.strip_prefix("event: ") {
                                        event_line = Some(ev.to_string());
                                    } else if let Some(d) = l.strip_prefix("data: ") {
                                        data_line = Some(d.to_string());
                                    }
                                }
                                if let (Some(event), Some(data)) = (event_line, data_line) {
                                    if event == "content_block_start" {
                                        if let Ok(v) =
                                            serde_json::from_str::<serde_json::Value>(&data)
                                        {
                                            if v.get("content_block")
                                                .and_then(|cb| cb.get("type"))
                                                .and_then(|t| t.as_str())
                                                == Some("tool_use")
                                            {
                                                let id = v
                                                    .get("content_block")
                                                    .and_then(|cb| cb.get("id"))
                                                    .and_then(|i| i.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                let name = v
                                                    .get("content_block")
                                                    .and_then(|cb| cb.get("name"))
                                                    .and_then(|n| n.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                current_tool = Some((id, name));
                                            } else {
                                                current_tool = None;
                                            }
                                        }
                                    } else {
                                        let tool_ref = current_tool
                                            .as_ref()
                                            .map(|(id, name)| (id.as_str(), name.as_str()));
                                        match parse_anthropic_sse_record(&event, &data, tool_ref) {
                                            Some(Ok(chunk)) => pending.push_back(Ok(chunk)),
                                            Some(Err(e)) => {
                                                done = true;
                                                pending.push_back(Err(e));
                                            }
                                            None => {}
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                done = true;
                                pending.push_back(Err(SDKError::ParseError(
                                    "SSE record contained invalid UTF-8".to_string(),
                                )));
                            }
                        }
                        continue;
                    }

                    match byte_stream.next().await {
                        Some(Ok(bytes)) => {
                            buffer.extend_from_slice(&bytes);
                        }
                        Some(Err(e)) => {
                            return Some((
                                Err(SDKError::NetworkError(e.to_string())),
                                (byte_stream, buffer, pending, true, current_tool),
                            ));
                        }
                        None => {
                            done = true;
                        }
                    }
                }
            },
        );

        Ok(Box::pin(s))
    }

    /// Call Anthropic API
    async fn call_anthropic_api(
        &self,
        provider: &crate::sdk::config::SdkProviderConfig,
        request: SdkChatRequest,
    ) -> Result<ChatResponse> {
        let model = self.provider_default_model(provider, "claude-sonnet-4-5");
        let body = build_anthropic_request_body(&request, model)?;
        let url = self.anthropic_messages_endpoint(provider);

        debug!("Calling Anthropic API: {}", url);

        let response = send_json_request(
            self.http_client
                .post(&url)
                .header("x-api-key", &provider.api_key)
                .header("anthropic-version", "2023-06-01"),
            &body,
        )
        .await?;

        if !response.status().is_success() {
            let error = api_error_from_response(response).await;
            error!("Anthropic API error: {}", error);
            return Err(error);
        }

        let anthropic_response: serde_json::Value = parse_json_response(response).await?;
        convert_anthropic_response(anthropic_response, model)
    }

    /// Call OpenAI API
    async fn call_openai_api(
        &self,
        provider: &crate::sdk::config::SdkProviderConfig,
        request: SdkChatRequest,
    ) -> Result<ChatResponse> {
        let model = self.provider_default_model(provider, "gpt-5.2-chat");
        let body = build_openai_request_body(&request, model);
        let url = self.provider_endpoint(provider, "https://api.openai.com", "v1/chat/completions");

        debug!("Calling OpenAI API: {}", url);

        let response = send_json_request(
            self.http_client
                .post(&url)
                .header("Authorization", format!("Bearer {}", provider.api_key)),
            &body,
        )
        .await?;

        if !response.status().is_success() {
            return Err(api_error_from_response(response).await);
        }

        parse_json_response(response).await
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
}

/// Find the end of an SSE record in a raw byte buffer, accepting both LF (`\n\n`) and CRLF
/// (`\r\n\r\n`) framing.
///
/// Returns `(record_end_pos, delimiter_len)` for the first boundary found, or `None`.
fn find_sse_record_end_bytes(buffer: &[u8]) -> Option<(usize, usize)> {
    let lf = buffer.windows(2).position(|w| w == b"\n\n");
    let crlf = buffer.windows(4).position(|w| w == b"\r\n\r\n");
    match (lf, crlf) {
        (Some(a), Some(b)) if b < a => Some((b, 4)),
        (Some(a), _) => Some((a, 2)),
        (None, Some(b)) => Some((b, 4)),
        (None, None) => None,
    }
}

/// Map Anthropic-native `stop_reason` values to OpenAI-style `finish_reason` values.
fn normalize_anthropic_stop_reason(stop_reason: &str) -> &str {
    match stop_reason {
        "end_turn" => "stop",
        "max_tokens" => "length",
        "tool_use" => "tool_calls",
        other => other,
    }
}

/// Parse a single OpenAI SSE line.
///
/// Returns `None` for non-data lines or the `[DONE]` terminator.
/// Returns `Some(Ok(chunk))` for a valid JSON chunk.
/// Returns `Some(Err(...))` for a malformed data line.
pub(crate) fn parse_openai_sse_line(line: &str) -> Option<Result<ChatChunk>> {
    let data = line.strip_prefix("data: ")?;
    if data == "[DONE]" {
        return None;
    }
    Some(
        serde_json::from_str(data)
            .map_err(|e| SDKError::ParseError(format!("Failed to parse SSE chunk: {}", e))),
    )
}

/// Parse an Anthropic SSE record (event + data pair).
///
/// Returns `Some(Ok(chunk))` for `content_block_delta` (text) and `message_delta` (finish_reason).
/// Returns `Some(Err(...))` for `error` events or malformed data on handled event types.
/// Returns `None` for lifecycle events that carry no user-visible content.
///
/// `current_tool` carries `(tool_id, tool_name)` captured from the preceding
/// `content_block_start` event so `input_json_delta` chunks include the tool identity.
pub(crate) fn parse_anthropic_sse_record(
    event: &str,
    data: &str,
    current_tool: Option<(&str, &str)>,
) -> Option<Result<ChatChunk>> {
    match event {
        "error" => {
            let msg = serde_json::from_str::<serde_json::Value>(data)
                .ok()
                .and_then(|v| {
                    v.get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .map(|s| s.to_string())
                })
                .unwrap_or_else(|| data.to_string());
            Some(Err(SDKError::ApiError(format!(
                "Anthropic stream error: {}",
                msg
            ))))
        }
        "message_delta" => {
            let v: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(e) => {
                    return Some(Err(SDKError::ParseError(format!(
                        "Failed to parse Anthropic message_delta: {}",
                        e
                    ))));
                }
            };
            let stop_reason = v
                .get("delta")
                .and_then(|d| d.get("stop_reason"))
                .and_then(|r| r.as_str())
                .map(|s| normalize_anthropic_stop_reason(s).to_string());
            Some(Ok(ChatChunk {
                id: String::new(),
                model: String::new(),
                choices: vec![ChunkChoice {
                    index: 0,
                    delta: MessageDelta {
                        role: None,
                        content: None,
                        tool_calls: None,
                    },
                    finish_reason: stop_reason,
                }],
            }))
        }
        "content_block_delta" => {
            let v: serde_json::Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(e) => {
                    return Some(Err(SDKError::ParseError(format!(
                        "Failed to parse Anthropic SSE record: {}",
                        e
                    ))));
                }
            };
            let delta_type = v
                .get("delta")
                .and_then(|d| d.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("text_delta");
            match delta_type {
                "text_delta" => {
                    let text = v
                        .get("delta")
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    Some(Ok(ChatChunk {
                        id: String::new(),
                        model: String::new(),
                        choices: vec![ChunkChoice {
                            index: 0,
                            delta: MessageDelta {
                                role: None,
                                content: Some(text.to_string()),
                                tool_calls: None,
                            },
                            finish_reason: None,
                        }],
                    }))
                }
                "input_json_delta" => {
                    let partial_json = v
                        .get("delta")
                        .and_then(|d| d.get("partial_json"))
                        .and_then(|j| j.as_str())
                        .unwrap_or("");
                    let (tool_id, tool_name) = current_tool.unwrap_or(("", ""));
                    Some(Ok(ChatChunk {
                        id: String::new(),
                        model: String::new(),
                        choices: vec![ChunkChoice {
                            index: 0,
                            delta: MessageDelta {
                                role: None,
                                content: None,
                                tool_calls: Some(vec![crate::sdk::types::ToolCall {
                                    id: tool_id.to_string(),
                                    tool_type: "function".to_string(),
                                    function: crate::sdk::types::Function {
                                        name: tool_name.to_string(),
                                        description: None,
                                        parameters: serde_json::Value::Null,
                                        arguments: Some(partial_json.to_string()),
                                    },
                                }]),
                            },
                            finish_reason: None,
                        }],
                    }))
                }
                _ => None,
            }
        }
        _ => None,
    }
}
