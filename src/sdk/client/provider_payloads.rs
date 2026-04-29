//! Provider-specific request/response payload helpers for the SDK client.

use crate::sdk::{errors::*, types::*};
use base64::Engine as _;
use std::time::SystemTime;

pub(super) fn build_anthropic_request_body(
    request: &SdkChatRequest,
    model: &str,
) -> Result<serde_json::Value> {
    let (system_message, anthropic_messages) = convert_messages_to_anthropic(&request.messages)?;

    let mut body = serde_json::json!({
        "model": model,
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

    Ok(body)
}

pub(super) fn build_openai_request_body(
    request: &SdkChatRequest,
    model: &str,
) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": request.messages,
        "max_tokens": request.options.max_tokens.unwrap_or(1000),
        "temperature": request.options.temperature.unwrap_or(0.7),
        "stream": false
    })
}

pub(super) fn convert_messages_to_anthropic(
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
                    "content": convert_content_to_anthropic(message.content.as_ref())?
                }));
            }
            Role::Assistant => {
                anthropic_messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": convert_content_to_anthropic(message.content.as_ref())?
                }));
            }
            _ => {}
        }
    }

    Ok((system_message, anthropic_messages))
}

/// Anthropic vision accepts exactly these four raster types.
const ANTHROPIC_IMAGE_TYPES: &[&str] = &["image/jpeg", "image/png", "image/gif", "image/webp"];

/// Parse `data:<media_type>;base64,<data>` into `(media_type, base64_data)`.
/// Returns `None` for plain URLs, non-base64 data URIs, or malformed data URIs.
/// Requires the explicit `;base64,` marker and a MIME type in `ANTHROPIC_IMAGE_TYPES`.
/// `data:text/plain;base64,…`, `data:image/svg+xml;base64,…`, and
/// `data:image/png;charset=utf-8,…` all return `None`.
/// The returned media type is normalized to ASCII lowercase per RFC 2045.
fn parse_data_uri(url: &str) -> Option<(String, &str)> {
    let rest = url.strip_prefix("data:")?;
    // Split on the explicit ";base64," marker so non-base64 params are rejected.
    let (header, data) = rest.split_once(";base64,")?;
    // Strip any trailing media-type parameters (e.g. `image/png;charset=utf-8` → `image/png`).
    let media_type = header.split(';').next().filter(|s| !s.is_empty())?;
    // Normalize to lowercase — MIME types are case-insensitive per RFC 2045.
    let normalized = media_type.to_ascii_lowercase();
    // Only Anthropic's raster whitelist is valid.
    if !ANTHROPIC_IMAGE_TYPES.contains(&normalized.as_str()) {
        return None;
    }
    // Reject empty payloads or payloads that fail strict base64 decoding.
    if data.is_empty() {
        return None;
    }
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .ok()?;
    Some((normalized, data))
}

/// Convert content to Anthropic format.
/// Returns `Err(SDKError::InvalidRequest)` for URL images, non-base64 data URIs, or
/// data URIs that lack the `;base64,` marker.
pub(super) fn convert_content_to_anthropic(content: Option<&Content>) -> Result<serde_json::Value> {
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
                        let url = &image_url.url;
                        if url.starts_with("data:") {
                            match parse_data_uri(url) {
                                Some((media_type, data)) => {
                                    anthropic_content.push(serde_json::json!({
                                        "type": "image",
                                        "source": {
                                            "type": "base64",
                                            "media_type": media_type,
                                            "data": data
                                        }
                                    }));
                                }
                                None => {
                                    return Err(SDKError::InvalidRequest(
                                        "data URI must use ';base64,' encoding with a valid, non-empty base64 payload".to_string(),
                                    ));
                                }
                            }
                        } else {
                            return Err(SDKError::InvalidRequest(
                                "URL images are not supported for Anthropic; use a base64 data URI instead".to_string(),
                            ));
                        }
                    }
                    ContentPart::Audio { .. } => {
                        return Err(SDKError::InvalidRequest(
                            "audio content is not supported by the Anthropic messages API"
                                .to_string(),
                        ));
                    }
                }
            }
            Ok(serde_json::json!(anthropic_content))
        }
        None => Ok(serde_json::json!("")),
    }
}

pub(super) fn convert_anthropic_response(
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
            completion_tokens: u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            total_tokens: 0,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_content_to_anthropic_text() {
        let converted =
            convert_content_to_anthropic(Some(&Content::Text("hello".to_string()))).unwrap();
        assert_eq!(converted, serde_json::json!("hello"));
    }

    #[test]
    fn test_jpeg_data_uri() {
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "data:image/jpeg;base64,/9j/4AAQ".to_string(),
                detail: None,
            },
        }]);
        let val = convert_content_to_anthropic(Some(&content)).unwrap();
        assert_eq!(val[0]["source"]["media_type"], "image/jpeg");
        assert_eq!(val[0]["source"]["data"], "/9j/4AAQ");
    }

    #[test]
    fn test_png_data_uri() {
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "data:image/png;base64,iVBORw==".to_string(),
                detail: None,
            },
        }]);
        let val = convert_content_to_anthropic(Some(&content)).unwrap();
        assert_eq!(val[0]["source"]["media_type"], "image/png");
        assert_eq!(val[0]["source"]["data"], "iVBORw==");
    }

    #[test]
    fn test_webp_data_uri() {
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "data:image/webp;base64,UklGRg==".to_string(),
                detail: None,
            },
        }]);
        let val = convert_content_to_anthropic(Some(&content)).unwrap();
        assert_eq!(val[0]["source"]["media_type"], "image/webp");
        assert_eq!(val[0]["source"]["data"], "UklGRg==");
    }

    #[test]
    fn test_gif_data_uri() {
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "data:image/gif;base64,R0lGODlh".to_string(),
                detail: None,
            },
        }]);
        let val = convert_content_to_anthropic(Some(&content)).unwrap();
        assert_eq!(val[0]["source"]["media_type"], "image/gif");
        assert_eq!(val[0]["source"]["data"], "R0lGODlh");
    }

    #[test]
    fn test_multimodal_audio_part_returns_error() {
        // Audio parts are not supported by the Anthropic messages API.
        let content = Content::Multimodal(vec![
            ContentPart::Text {
                text: "hello".to_string(),
            },
            ContentPart::Audio {
                audio: AudioData {
                    data: "base64audiodata".to_string(),
                    format: Some("mp3".to_string()),
                },
            },
        ]);
        let err = convert_content_to_anthropic(Some(&content)).unwrap_err();
        assert!(matches!(err, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_audio_only_multimodal_returns_error() {
        // Audio-only content is not supported by the Anthropic messages API.
        let content = Content::Multimodal(vec![ContentPart::Audio {
            audio: AudioData {
                data: "base64audiodata".to_string(),
                format: None,
            },
        }]);
        let err = convert_content_to_anthropic(Some(&content)).unwrap_err();
        assert!(matches!(err, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_malformed_data_uri_returns_error() {
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "data:image/png;base64".to_string(),
                detail: None,
            },
        }]);
        let err = convert_content_to_anthropic(Some(&content)).unwrap_err();
        assert!(matches!(err, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_plain_url_returns_error() {
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "https://example.com/image.png".to_string(),
                detail: None,
            },
        }]);
        let err = convert_content_to_anthropic(Some(&content)).unwrap_err();
        assert!(matches!(err, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_non_base64_charset_param_returns_error() {
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "data:image/png;charset=utf-8,abc".to_string(),
                detail: None,
            },
        }]);
        let err = convert_content_to_anthropic(Some(&content)).unwrap_err();
        assert!(matches!(err, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_invalid_base64_payload_returns_error() {
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "data:image/png;base64,invalid!!!".to_string(),
                detail: None,
            },
        }]);
        let err = convert_content_to_anthropic(Some(&content)).unwrap_err();
        assert!(matches!(err, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_empty_base64_payload_returns_error() {
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "data:image/png;base64,".to_string(),
                detail: None,
            },
        }]);
        let err = convert_content_to_anthropic(Some(&content)).unwrap_err();
        assert!(matches!(err, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_parse_data_uri_jpeg() {
        let (mt, data) = parse_data_uri("data:image/jpeg;base64,/9j/4AAQ").unwrap();
        assert_eq!(mt, "image/jpeg");
        assert_eq!(data, "/9j/4AAQ");
    }

    #[test]
    fn test_parse_data_uri_plain_url_returns_none() {
        assert!(parse_data_uri("https://example.com/image.png").is_none());
    }

    #[test]
    fn test_parse_data_uri_with_media_type_params() {
        let (mt, data) = parse_data_uri("data:image/png;charset=utf-8;base64,iVBORw==").unwrap();
        assert_eq!(mt, "image/png");
        assert_eq!(data, "iVBORw==");
    }

    #[test]
    fn test_parse_data_uri_non_base64_returns_none() {
        assert!(parse_data_uri("data:image/png;charset=utf-8,abc").is_none());
        assert!(parse_data_uri("data:image/png;name=foo,abc").is_none());
        assert!(parse_data_uri("data:image/png,abc").is_none());
    }

    #[test]
    fn test_parse_data_uri_empty_payload_returns_none() {
        assert!(parse_data_uri("data:image/png;base64,").is_none());
    }

    #[test]
    fn test_parse_data_uri_invalid_base64_chars_returns_none() {
        assert!(parse_data_uri("data:image/png;base64,invalid!!!").is_none());
        assert!(parse_data_uri("data:image/png;base64,abc def").is_none());
    }

    #[test]
    fn test_parse_data_uri_non_image_mime_returns_none() {
        assert!(parse_data_uri("data:text/plain;base64,SGVsbG8=").is_none());
        assert!(parse_data_uri("data:application/pdf;base64,SGVsbG8=").is_none());
    }

    #[test]
    fn test_parse_data_uri_case_insensitive_mime() {
        // RFC 2045: MIME types are case-insensitive; normalized to lowercase on return.
        let (mt, data) = parse_data_uri("data:IMAGE/PNG;base64,iVBORw==").unwrap();
        assert_eq!(mt, "image/png");
        assert_eq!(data, "iVBORw==");

        let (mt, _) = parse_data_uri("data:Image/WebP;base64,UklGRg==").unwrap();
        assert_eq!(mt, "image/webp");

        let (mt, _) = parse_data_uri("data:IMAGE/JPEG;base64,/9j/4AAQ").unwrap();
        assert_eq!(mt, "image/jpeg");
    }

    #[test]
    fn test_convert_content_uppercase_mime_accepted() {
        // case-insensitive MIME: IMAGE/PNG should be accepted and normalized.
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "data:IMAGE/PNG;base64,iVBORw==".to_string(),
                detail: None,
            },
        }]);
        let val = convert_content_to_anthropic(Some(&content)).unwrap();
        assert_eq!(val[0]["source"]["media_type"], "image/png");
        assert_eq!(val[0]["source"]["data"], "iVBORw==");
    }

    #[test]
    fn test_parse_data_uri_unsupported_image_subtype_returns_none() {
        // Non-whitelisted image/* subtypes must also be rejected locally.
        assert!(parse_data_uri("data:image/svg+xml;base64,SGVsbG8=").is_none());
        assert!(parse_data_uri("data:image/bmp;base64,SGVsbG8=").is_none());
        assert!(parse_data_uri("data:image/tiff;base64,SGVsbG8=").is_none());
    }

    #[test]
    fn test_unsupported_image_subtype_returns_error() {
        // svg+xml passes `starts_with("image/")` but must be caught by the whitelist.
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "data:image/svg+xml;base64,SGVsbG8=".to_string(),
                detail: None,
            },
        }]);
        let err = convert_content_to_anthropic(Some(&content)).unwrap_err();
        assert!(matches!(err, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_non_image_mime_type_returns_error() {
        let content = Content::Multimodal(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "data:text/plain;base64,SGVsbG8=".to_string(),
                detail: None,
            },
        }]);
        let err = convert_content_to_anthropic(Some(&content)).unwrap_err();
        assert!(matches!(err, SDKError::InvalidRequest(_)));
    }

    #[test]
    fn test_parse_data_uri_length_invalid_base64_returns_none() {
        // Single char — valid alphabet but invalid length
        assert!(parse_data_uri("data:image/png;base64,a").is_none());
        // 5 chars — valid alphabet but invalid length
        assert!(parse_data_uri("data:image/png;base64,abcde").is_none());
    }

    #[test]
    fn test_convert_anthropic_response_maps_usage_and_message() {
        let response = convert_anthropic_response(
            serde_json::json!({
                "id": "anthropic-response-1",
                "content": [{ "text": "hello from claude" }],
                "usage": {
                    "input_tokens": 11,
                    "output_tokens": 7
                }
            }),
            "claude-sonnet-4-5",
        )
        .unwrap();

        assert_eq!(response.id, "anthropic-response-1");
        assert_eq!(response.model, "claude-sonnet-4-5");
        assert_eq!(response.usage.prompt_tokens, 11);
        assert_eq!(response.usage.completion_tokens, 7);
        assert_eq!(response.usage.total_tokens, 18);
        assert_eq!(response.choices.len(), 1);
        assert!(matches!(response.choices[0].message.role, Role::Assistant));
        assert!(matches!(
            response.choices[0].message.content,
            Some(Content::Text(ref text)) if text == "hello from claude"
        ));
    }

    #[test]
    fn test_build_anthropic_request_body_includes_system_and_sampling_fields() {
        let request = SdkChatRequest {
            model: "ignored".to_string(),
            messages: vec![
                Message {
                    role: Role::System,
                    content: Some(Content::Text("system prompt".to_string())),
                    name: None,
                    tool_calls: None,
                },
                Message {
                    role: Role::User,
                    content: Some(Content::Text("hello".to_string())),
                    name: None,
                    tool_calls: None,
                },
            ],
            options: ChatOptions {
                temperature: Some(0.2),
                max_tokens: Some(42),
                top_p: Some(0.8),
                ..Default::default()
            },
        };

        let body = build_anthropic_request_body(&request, "claude-sonnet-4-5").unwrap();

        assert_eq!(body["model"], "claude-sonnet-4-5");
        assert_eq!(body["system"], "system prompt");
        assert_eq!(body["max_tokens"], 42);
        assert!((body["temperature"].as_f64().unwrap() - 0.2).abs() < 1e-6);
        assert!((body["top_p"].as_f64().unwrap() - 0.8).abs() < 1e-6);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "hello");
    }

    #[test]
    fn test_build_openai_request_body_uses_defaults_and_preserves_messages() {
        let request = SdkChatRequest {
            model: "ignored".to_string(),
            messages: vec![Message {
                role: Role::User,
                content: Some(Content::Text("hello".to_string())),
                name: Some("alice".to_string()),
                tool_calls: Some(vec![ToolCall {
                    id: "call-1".to_string(),
                    tool_type: "function".to_string(),
                    function: Function {
                        name: "lookup".to_string(),
                        description: Some("Lookup info".to_string()),
                        parameters: serde_json::json!({"type":"object"}),
                        arguments: None,
                    },
                }]),
            }],
            options: ChatOptions::default(),
        };

        let body = build_openai_request_body(&request, "gpt-5.2-chat");

        assert_eq!(body["model"], "gpt-5.2-chat");
        assert_eq!(body["max_tokens"], 1000);
        assert!((body["temperature"].as_f64().unwrap() - 0.7).abs() < 1e-6);
        assert_eq!(body["stream"], false);
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "hello");
        assert_eq!(body["messages"][0]["name"], "alice");
        assert_eq!(body["messages"][0]["tool_calls"][0]["id"], "call-1");
    }
}
