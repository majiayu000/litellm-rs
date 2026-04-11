//! Provider-specific request/response payload helpers for the SDK client.

use crate::sdk::{errors::Result, types::*};
use std::time::SystemTime;

pub(super) fn build_anthropic_request_body(
    request: &SdkChatRequest,
    model: &str,
) -> serde_json::Value {
    let (system_message, anthropic_messages) = convert_messages_to_anthropic(&request.messages);

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

    body
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
) -> (Option<String>, Vec<serde_json::Value>) {
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
                    "content": convert_content_to_anthropic(message.content.as_ref())
                }));
            }
            Role::Assistant => {
                anthropic_messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": convert_content_to_anthropic(message.content.as_ref())
                }));
            }
            _ => {}
        }
    }

    (system_message, anthropic_messages)
}

pub(super) fn convert_content_to_anthropic(content: Option<&Content>) -> serde_json::Value {
    match content {
        Some(Content::Text(text)) => serde_json::json!(text),
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
                        anthropic_content.push(serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": "image/jpeg",
                                "data": image_url.url.trim_start_matches("data:image/jpeg;base64,")
                            }
                        }));
                    }
                    _ => {}
                }
            }
            serde_json::json!(anthropic_content)
        }
        None => serde_json::json!(""),
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
        let converted = convert_content_to_anthropic(Some(&Content::Text("hello".to_string())));

        assert_eq!(converted, serde_json::json!("hello"));
    }

    #[test]
    fn test_convert_content_to_anthropic_multimodal_filters_supported_parts() {
        let converted = convert_content_to_anthropic(Some(&Content::Multimodal(vec![
            ContentPart::Text {
                text: "hello".to_string(),
            },
            ContentPart::Image {
                image_url: ImageUrl {
                    url: "data:image/jpeg;base64,abc123".to_string(),
                    detail: None,
                },
            },
            ContentPart::Audio {
                audio: AudioData {
                    data: "ignored".to_string(),
                    format: None,
                },
            },
        ])));

        assert_eq!(
            converted,
            serde_json::json!([
                { "type": "text", "text": "hello" },
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/jpeg",
                        "data": "abc123"
                    }
                }
            ])
        );
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

        let body = build_anthropic_request_body(&request, "claude-sonnet-4-5");

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
