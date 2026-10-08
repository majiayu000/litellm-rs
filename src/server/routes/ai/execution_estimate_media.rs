//! Project native media once through the existing content-part token policy.

use crate::core::models::openai::{
    AudioContent, ChatMessage, ContentPart, DocumentSource, ImageSource, ImageUrl, MessageContent,
    MessageRole,
};
use crate::utils::ai::counter::token_counter::TokenizerIdentity;
use crate::utils::error::gateway_error::GatewayError;
use serde_json::Value;

pub(super) fn native_input<T: serde::Serialize>(
    request: &T,
    model: &str,
) -> Result<u64, GatewayError> {
    let mut projected = serde_json::to_value(request)
        .map_err(|_| GatewayError::validation("Request cannot be token-estimated"))?;
    let mut parts = Vec::new();
    project(&mut projected, &mut parts);
    // Pure JSON and opaque continuation strings retain their existing counter.
    let text = super::json_input(&projected)?;
    if parts.is_empty() {
        return Ok(text);
    }
    let media = super::super::super::spend::try_estimate_chat_prompt_tokens(
        &TokenizerIdentity::approximate("gateway", model),
        &[ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Parts(parts)),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }],
        None,
        None,
        None,
        None,
    )?;
    Ok(text.saturating_add(u64::from(media)))
}

fn project(value: &mut Value, parts: &mut Vec<ContentPart>) {
    // Gemini media occupies a field in a Part. Preserve any sibling fields.
    if let Some(object) = value.as_object_mut() {
        for name in ["inlineData", "inline_data", "fileData", "file_data"] {
            if let Some(source) = object.get_mut(name)
                && let Some(mime) = source
                    .get("mimeType")
                    .or_else(|| source.get("mime_type"))
                    .and_then(Value::as_str)
                && (source.get("data").and_then(Value::as_str).is_some()
                    || source
                        .get("fileUri")
                        .or_else(|| source.get("file_uri"))
                        .and_then(Value::as_str)
                        .is_some())
            {
                let data = source
                    .get("data")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                parts.push(encoded_part(mime, data));
                *source = Value::Null;
            }
        }
    }
    if let Some(part) = tagged_part(value) {
        parts.push(part);
        *value = Value::Null;
        return;
    }
    match value {
        Value::Array(values) => {
            for value in values {
                project(value, parts);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                project(value, parts);
            }
        }
        _ => {}
    }
}

fn encoded_part(mime: &str, data: &str) -> ContentPart {
    if mime.starts_with("image/") {
        ContentPart::Image {
            source: ImageSource {
                media_type: mime.into(),
                data: data.into(),
            },
            detail: None,
            image_url: None,
        }
    } else if mime.starts_with("audio/") {
        ContentPart::Audio {
            audio: AudioContent {
                data: data.into(),
                format: mime.into(),
            },
        }
    } else {
        ContentPart::Document {
            source: DocumentSource {
                media_type: mime.into(),
                data: data.into(),
            },
            cache_control: None,
        }
    }
}

fn tagged_part(value: &Value) -> Option<ContentPart> {
    let kind = value.get("type")?.as_str()?;
    match kind {
        "image" | "document" => {
            let source = value.get("source")?;
            if let (Some(mime), Some(data)) = (
                source.get("media_type").and_then(Value::as_str),
                source.get("data").and_then(Value::as_str),
            ) {
                return Some(encoded_part(mime, data));
            }
            let url = source.get("url")?.as_str()?;
            Some(if kind == "image" {
                ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: url.into(),
                        detail: None,
                    },
                }
            } else {
                encoded_part("application/octet-stream", "")
            })
        }
        "input_image" | "image_url" => {
            let image = value.get("image_url")?;
            let url = image
                .as_str()
                .or_else(|| image.get("url").and_then(Value::as_str))?;
            let detail = value
                .get("detail")
                .or_else(|| image.get("detail"))
                .and_then(Value::as_str)
                .map(str::to_owned);
            Some(ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: url.into(),
                    detail,
                },
            })
        }
        "input_audio" => {
            let audio = value.get("input_audio")?;
            Some(ContentPart::Audio {
                audio: AudioContent {
                    data: audio.get("data")?.as_str()?.into(),
                    format: audio.get("format")?.as_str()?.into(),
                },
            })
        }
        _ => None,
    }
}
