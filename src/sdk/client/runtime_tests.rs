use super::*;
use crate::sdk::types::{ChatOptions, Function, Role, Tool};

fn sdk_user_message_fixture() -> Message {
    Message {
        role: Role::User,
        content: Some(crate::sdk::types::Content::Text("hello".to_string())),
        name: None,
        tool_calls: None,
    }
}

#[test]
fn sdk_request_adapter_preserves_tools_and_explicit_choice() {
    let request = SdkChatRequest {
        model: "public-model".to_string(),
        messages: vec![sdk_user_message_fixture()],
        options: ChatOptions {
            tools: Some(vec![Tool {
                tool_type: "function".to_string(),
                function: Function {
                    name: "lookup".to_string(),
                    description: Some("lookup a value".to_string()),
                    parameters: serde_json::json!({"type": "object"}),
                    arguments: None,
                },
            }]),
            tool_choice: Some(SdkToolChoice::Function {
                name: "lookup".to_string(),
            }),
            ..Default::default()
        },
    };

    let adapted = sdk_request_to_core("public-model", request).unwrap();
    assert_eq!(adapted.messages.len(), 1);
    assert_eq!(adapted.tools.as_ref().map(Vec::len), Some(1));
    assert_eq!(
        serde_json::to_value(adapted.tool_choice).unwrap(),
        serde_json::json!({"type": "function", "function": {"name": "lookup"}})
    );
}

#[test]
fn response_adapter_preserves_complete_tool_calls() {
    let response: CoreChatResponse = serde_json::from_value(serde_json::json!({
        "id": "chatcmpl-tool",
        "object": "chat.completion",
        "created": 1,
        "model": "tool-model",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call-1",
                    "type": "function",
                    "function": {"name": "lookup", "arguments": "{\"id\":1}"}
                }]
            },
            "finish_reason": "tool_calls"
        }]
    }))
    .unwrap();

    let adapted = core_response_to_sdk(response).unwrap();
    let call = &adapted.choices[0].message.tool_calls.as_ref().unwrap()[0];
    assert_eq!(call.id, "call-1");
    assert_eq!(call.function.name, "lookup");
    assert_eq!(call.function.arguments.as_deref(), Some("{\"id\":1}"));
}

#[test]
fn stream_adapter_fails_closed_for_unrepresentable_deltas() {
    let chunk: CoreChatChunk = serde_json::from_value(serde_json::json!({
        "id": "chatcmpl-thinking",
        "object": "chat.completion.chunk",
        "created": 1,
        "model": "thinking-model",
        "choices": [{
            "index": 0,
            "delta": {"thinking": {"content": "secret reasoning"}},
            "finish_reason": null
        }]
    }))
    .unwrap();

    let error = core_chunk_to_sdk(chunk).unwrap_err();
    assert!(matches!(error, SDKError::NotSupported(_)));
}
