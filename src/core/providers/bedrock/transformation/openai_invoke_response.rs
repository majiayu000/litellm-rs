//! OpenAI-compatible Invoke response parsing for runtime-resolved Bedrock ARNs.

use serde_json::Value;

use crate::core::types::responses::{ChatChoice, FinishReason};
use crate::core::types::tools::{FunctionCall, ToolCall};
use crate::core::types::{
    chat::ChatMessage, content::ContentPart, message::MessageContent, message::MessageRole,
};

pub(super) fn parse_response(response: &Value) -> Vec<ChatChoice> {
    let Some(choices) = response.get("choices").and_then(Value::as_array) else {
        return vec![fallback_choice()];
    };

    let parsed = choices
        .iter()
        .enumerate()
        .map(|(fallback_index, choice)| parse_choice(choice, fallback_index))
        .collect::<Vec<_>>();

    if parsed.is_empty() {
        vec![fallback_choice()]
    } else {
        parsed
    }
}

fn parse_choice(choice: &Value, fallback_index: usize) -> ChatChoice {
    let message = choice.get("message");
    let delta = choice.get("delta");
    let content = message
        .and_then(|message| message.get("content"))
        .and_then(parse_content)
        .or_else(|| {
            choice
                .get("text")
                .and_then(Value::as_str)
                .map(|text| MessageContent::Text(text.to_string()))
        })
        .or_else(|| {
            delta
                .and_then(|delta| delta.get("content"))
                .and_then(parse_content)
        });
    let tool_calls = message
        .and_then(|message| message.get("tool_calls"))
        .and_then(parse_tool_calls)
        .or_else(|| {
            delta
                .and_then(|delta| delta.get("tool_calls"))
                .and_then(parse_tool_calls)
        });
    let finish_reason = choice
        .get("finish_reason")
        .and_then(Value::as_str)
        .map(parse_finish_reason)
        .unwrap_or(FinishReason::Stop);

    ChatChoice {
        index: choice
            .get("index")
            .and_then(Value::as_u64)
            .and_then(|index| u32::try_from(index).ok())
            .unwrap_or(fallback_index as u32),
        message: ChatMessage {
            role: MessageRole::Assistant,
            content,
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls,
            tool_call_id: None,
        },
        finish_reason: Some(finish_reason),
        logprobs: None,
    }
}

fn parse_content(value: &Value) -> Option<MessageContent> {
    if value.is_null() {
        return None;
    }
    if let Some(text) = value.as_str() {
        return Some(MessageContent::Text(text.to_string()));
    }

    let parts = value.as_array()?;
    serde_json::from_value::<Vec<ContentPart>>(value.clone())
        .ok()
        .map(MessageContent::Parts)
        .or_else(|| {
            let text = parts
                .iter()
                .filter_map(|part| part.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("");
            (!text.is_empty()).then_some(MessageContent::Text(text))
        })
}

fn parse_tool_calls(value: &Value) -> Option<Vec<ToolCall>> {
    let calls = value.as_array()?;
    let parsed = calls
        .iter()
        .map(|call| {
            let function = call.get("function");
            ToolCall {
                id: call
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                tool_type: call
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("function")
                    .to_string(),
                function: FunctionCall {
                    name: function
                        .and_then(|function| function.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                    arguments: function
                        .and_then(|function| function.get("arguments"))
                        .and_then(json_value_to_string)
                        .unwrap_or_default(),
                },
            }
        })
        .collect::<Vec<_>>();

    (!parsed.is_empty()).then_some(parsed)
}

fn json_value_to_string(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::to_string)
        .or_else(|| (!value.is_null()).then(|| value.to_string()))
}

fn parse_finish_reason(reason: &str) -> FinishReason {
    match reason {
        "length" => FinishReason::Length,
        "tool_calls" => FinishReason::ToolCalls,
        "content_filter" => FinishReason::ContentFilter,
        "stop_sequence" => FinishReason::StopSequence,
        _ => FinishReason::Stop,
    }
}

fn fallback_choice() -> ChatChoice {
    ChatChoice {
        index: 0,
        message: ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text(String::new())),
            thinking: None,
            audio: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
        },
        finish_reason: Some(FinishReason::Stop),
        logprobs: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tool_calls() {
        let response = serde_json::json!({
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_123",
                        "type": "function",
                        "function": {
                            "name": "lookup_weather",
                            "arguments": "{\"city\":\"Paris\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let choices = parse_response(&response);
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].finish_reason, Some(FinishReason::ToolCalls));
        let tool_calls = choices[0]
            .message
            .tool_calls
            .as_ref()
            .unwrap_or_else(|| panic!("tool calls should be preserved"));
        assert_eq!(tool_calls[0].id, "call_123");
        assert_eq!(tool_calls[0].function.name, "lookup_weather");
        assert_eq!(tool_calls[0].function.arguments, "{\"city\":\"Paris\"}");
    }

    #[test]
    fn parses_content_parts_and_all_choices() {
        let response = serde_json::json!({
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "content": [{"type": "text", "text": "first"}]
                    },
                    "finish_reason": "stop"
                },
                {
                    "index": 1,
                    "message": {
                        "content": "second"
                    },
                    "finish_reason": "length"
                }
            ]
        });

        let choices = parse_response(&response);
        assert_eq!(choices.len(), 2);
        assert!(matches!(
            choices[0].message.content,
            Some(MessageContent::Parts(_))
        ));
        assert!(matches!(
            choices[1].message.content.as_ref(),
            Some(MessageContent::Text(text)) if text == "second"
        ));
        assert_eq!(choices[1].finish_reason, Some(FinishReason::Length));
    }
}
