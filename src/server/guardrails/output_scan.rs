//! Output projection for guardrail checks.

use crate::core::guardrails::GuardrailEngine;
use crate::core::models::openai::{
    ChatCompletionResponse, ChatMessage, ContentPart, MessageContent,
};
use crate::utils::error::gateway_error::GatewayError;

pub(super) fn response_payload(
    engine: &GuardrailEngine,
    response: &ChatCompletionResponse,
) -> Result<String, GatewayError> {
    let mut fragments = Vec::new();
    let scan_documents = engine.is_enabled() && engine.config().check_output;
    for choice in &response.choices {
        collect_message(&choice.message, &mut fragments, scan_documents)?;
        push_optional(&mut fragments, choice.finish_reason.as_deref());
        for logprob in choice
            .logprobs
            .as_ref()
            .and_then(|logprobs| logprobs.content.as_ref())
            .into_iter()
            .flatten()
        {
            push(&mut fragments, &logprob.token);
            for top in logprob.top_logprobs.iter().flatten() {
                push(&mut fragments, &top.token);
            }
        }
    }
    Ok(fragments.join(super::FRAGMENT_SEPARATOR))
}

fn collect_message(
    message: &ChatMessage,
    fragments: &mut Vec<String>,
    scan_documents: bool,
) -> Result<(), GatewayError> {
    push_optional(fragments, message.name.as_deref());
    push_optional(fragments, message.tool_call_id.as_deref());
    if let Some(audio) = message.audio.as_ref() {
        push(fragments, &audio.format);
    }
    match message.content.as_ref() {
        Some(MessageContent::Text(text)) => push(fragments, text),
        Some(MessageContent::Parts(parts)) => collect_parts(parts, fragments, scan_documents)?,
        None => {}
    }
    if let Some(call) = &message.function_call {
        push(fragments, &call.name);
        collect_function_arguments(&call.arguments, fragments);
    }
    for call in message.tool_calls.iter().flatten() {
        push(fragments, &call.id);
        push(fragments, &call.tool_type);
        push(fragments, &call.function.name);
        collect_function_arguments(&call.function.arguments, fragments);
    }
    Ok(())
}

fn collect_parts(
    parts: &[ContentPart],
    fragments: &mut Vec<String>,
    scan_documents: bool,
) -> Result<(), GatewayError> {
    let mut text_group = String::new();
    for part in parts {
        if let ContentPart::Text { text } = part {
            if !text.is_empty() {
                if !text_group.is_empty() {
                    text_group.push('\n');
                }
                text_group.push_str(text);
            }
            continue;
        }
        push(fragments, &text_group);
        text_group.clear();
        collect_non_text_part(part, fragments, scan_documents)?;
    }
    push(fragments, &text_group);
    Ok(())
}

fn collect_non_text_part(
    part: &ContentPart,
    fragments: &mut Vec<String>,
    scan_documents: bool,
) -> Result<(), GatewayError> {
    match part {
        ContentPart::ImageUrl { image_url } => {
            push(fragments, &image_url.url);
            push_optional(fragments, image_url.detail.as_deref());
        }
        ContentPart::Image {
            source,
            detail,
            image_url,
        } => {
            push(fragments, &source.media_type);
            push_optional(fragments, detail.as_deref());
            if let Some(image_url) = image_url {
                push(fragments, &image_url.url);
                push_optional(fragments, image_url.detail.as_deref());
            }
        }
        ContentPart::ToolResult {
            tool_use_id,
            content,
            ..
        } => {
            push(fragments, tool_use_id);
            collect_json_content(content, fragments);
        }
        ContentPart::ToolUse { id, name, input } => {
            push(fragments, id);
            push(fragments, name);
            collect_json_content(input, fragments);
        }
        ContentPart::Audio { audio } => push(fragments, &audio.format),
        ContentPart::Document {
            source,
            cache_control,
        } => {
            push(fragments, &source.media_type);
            if let Some(cache_control) = cache_control {
                push(fragments, &cache_control.cache_type);
            }
            if scan_documents && is_textual_document(&source.media_type) {
                let (format, text) = super::input_scan::document_text(source, "response document")
                    .map_err(|cause| GatewayError::Internal(cause.to_string()))?;
                match format {
                    super::input_scan::DocumentFormat::PlainText => push(fragments, &text),
                    super::input_scan::DocumentFormat::Json => {
                        let values = super::input_scan::json_text_values(&text).map_err(|cause| {
                            GatewayError::Internal(format!(
                                "output guardrail cannot scan response document: invalid JSON document: {cause}"
                            ))
                        })?;
                        fragments.extend(values);
                    }
                }
            }
        }
        ContentPart::Text { .. } => {}
    }
    Ok(())
}

fn is_textual_document(media_type: &str) -> bool {
    let essence = media_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    essence.starts_with("text/")
        || essence == "application/json"
        || essence
            .strip_prefix("application/")
            .is_some_and(|subtype| subtype.ends_with("+json"))
}

fn collect_function_arguments(arguments: &str, fragments: &mut Vec<String>) {
    push(fragments, arguments);
    if let Ok(value) = serde_json::from_str(arguments) {
        collect_json_content(&value, fragments);
    }
}

fn collect_json_content(value: &serde_json::Value, fragments: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => push(fragments, text),
        serde_json::Value::Array(values) => {
            for value in values {
                collect_json_content(value, fragments);
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                push(fragments, key);
                collect_json_content(value, fragments);
            }
        }
        serde_json::Value::Number(number) => fragments.push(number.to_string()),
        serde_json::Value::Bool(_) | serde_json::Value::Null => {}
    }
}

fn push_optional(fragments: &mut Vec<String>, text: Option<&str>) {
    if let Some(text) = text {
        push(fragments, text);
    }
}

fn push(fragments: &mut Vec<String>, text: &str) {
    if !text.is_empty() {
        fragments.push(text.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::gateway::GatewayConfig;
    use crate::core::models::openai::{
        AudioContent, CacheControl, DocumentSource, ImageSource, MessageRole,
    };
    use base64::Engine as _;

    fn message(parts: Vec<ContentPart>) -> ChatMessage {
        ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Parts(parts)),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }
    }

    fn engine() -> GuardrailEngine {
        GuardrailEngine::new(GatewayConfig::default().guardrails)
            .expect("guardrail policy must compile")
    }

    #[test]
    fn response_envelope_is_not_treated_as_maskable_content() {
        let response = ChatCompletionResponse {
            id: "user@example.com".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "10.0.0.1".to_string(),
            system_fingerprint: Some("second@example.com".to_string()),
            choices: Vec::new(),
            usage: None,
        };

        let payload = response_payload(&engine(), &response).expect("response should scan");

        assert!(payload.is_empty());
    }

    #[test]
    fn structured_numeric_values_are_scanned() {
        let message = message(vec![ContentPart::ToolUse {
            id: "call-1".to_string(),
            name: "lookup".to_string(),
            input: serde_json::json!({"phone": 2125551234_u64}),
        }]);
        let mut fragments = Vec::new();

        collect_message(&message, &mut fragments, true).expect("message should scan");

        assert!(fragments.iter().any(|value| value == "2125551234"));
    }

    #[test]
    fn adjacent_text_parts_remain_contiguous_for_guardrail_matching() {
        let message = message(vec![
            ContentPart::Text {
                text: "123-45".to_string(),
            },
            ContentPart::Text {
                text: "6789".to_string(),
            },
        ]);
        let mut fragments = Vec::new();

        collect_message(&message, &mut fragments, true).expect("message should scan");

        assert_eq!(fragments, vec!["123-45\n6789"]);
    }

    #[test]
    fn encoded_binary_parts_are_not_scanned_as_text() {
        let encoded = "2125551234==".to_string();
        let mut message = message(vec![
            ContentPart::Audio {
                audio: AudioContent {
                    data: encoded.clone(),
                    format: "part-format-marker".to_string(),
                },
            },
            ContentPart::Image {
                source: ImageSource {
                    media_type: "image-media-marker".to_string(),
                    data: encoded.clone(),
                },
                detail: Some("image-detail-marker".to_string()),
                image_url: None,
            },
            ContentPart::Document {
                source: DocumentSource {
                    media_type: "document-media-marker".to_string(),
                    data: encoded,
                },
                cache_control: Some(CacheControl {
                    cache_type: "cache-marker".to_string(),
                }),
            },
        ]);
        message.audio = Some(AudioContent {
            data: "top-level-encoded".to_string(),
            format: "message-format-marker".to_string(),
        });
        let mut fragments = Vec::new();

        collect_message(&message, &mut fragments, true).expect("message should scan");

        assert_eq!(
            fragments,
            vec![
                "message-format-marker",
                "part-format-marker",
                "image-media-marker",
                "image-detail-marker",
                "document-media-marker",
                "cache-marker"
            ]
        );
    }

    #[test]
    fn textual_document_bodies_are_decoded_and_scanned() {
        let message = message(vec![
            ContentPart::Document {
                source: DocumentSource {
                    media_type: "text/plain; charset=utf-8".to_string(),
                    data: base64::engine::general_purpose::STANDARD.encode("plain-marker"),
                },
                cache_control: None,
            },
            ContentPart::Document {
                source: DocumentSource {
                    media_type: "application/problem+json".to_string(),
                    data: base64::engine::general_purpose::STANDARD
                        .encode(r#"{"field":"json-marker"}"#),
                },
                cache_control: None,
            },
        ]);
        let mut fragments = Vec::new();

        collect_message(&message, &mut fragments, true).expect("text documents should scan");

        assert!(fragments.iter().any(|value| value == "plain-marker"));
        assert!(fragments.iter().any(|value| value == "field"));
        assert!(fragments.iter().any(|value| value == "json-marker"));
    }

    #[test]
    fn invalid_textual_document_bodies_fail_closed() {
        let message = message(vec![ContentPart::Document {
            source: DocumentSource {
                media_type: "text/plain".to_string(),
                data: "not base64".to_string(),
            },
            cache_control: None,
        }]);

        assert!(collect_message(&message, &mut Vec::new(), true).is_err());
    }
}
