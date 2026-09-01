//! Output projection for guardrail checks.

use crate::core::models::openai::{
    ChatCompletionResponse, ChatMessage, ContentPart, MessageContent,
};

pub(super) fn response_payload(response: &ChatCompletionResponse, separator: &str) -> String {
    let mut fragments = Vec::new();
    for choice in &response.choices {
        collect_message(&choice.message, &mut fragments);
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
    fragments.join(separator)
}

fn collect_message(message: &ChatMessage, fragments: &mut Vec<String>) {
    push_optional(fragments, message.name.as_deref());
    push_optional(fragments, message.tool_call_id.as_deref());
    match message.content.as_ref() {
        Some(MessageContent::Text(text)) => push(fragments, text),
        Some(MessageContent::Parts(parts)) => {
            for part in parts {
                match part {
                    ContentPart::Text { text } => push(fragments, text),
                    ContentPart::ImageUrl { image_url } => push(fragments, &image_url.url),
                    ContentPart::Image {
                        image_url: Some(image_url),
                        ..
                    } => push(fragments, &image_url.url),
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
                    ContentPart::Audio { .. }
                    | ContentPart::Image {
                        image_url: None, ..
                    }
                    | ContentPart::Document { .. } => {}
                }
            }
        }
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
    use crate::core::models::openai::{AudioContent, DocumentSource, ImageSource, MessageRole};

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

        let payload = response_payload(&response, "\n---\n");

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

        collect_message(&message, &mut fragments);

        assert!(fragments.iter().any(|value| value == "2125551234"));
    }

    #[test]
    fn encoded_binary_parts_are_not_scanned_as_text() {
        let encoded = "2125551234==".to_string();
        let message = message(vec![
            ContentPart::Audio {
                audio: AudioContent {
                    data: encoded.clone(),
                    format: "wav".to_string(),
                },
            },
            ContentPart::Image {
                source: ImageSource {
                    media_type: "image/png".to_string(),
                    data: encoded.clone(),
                },
                detail: None,
                image_url: None,
            },
            ContentPart::Document {
                source: DocumentSource {
                    media_type: "application/pdf".to_string(),
                    data: encoded,
                },
                cache_control: None,
            },
        ]);
        let mut fragments = Vec::new();

        collect_message(&message, &mut fragments);

        assert!(fragments.is_empty());
    }
}
