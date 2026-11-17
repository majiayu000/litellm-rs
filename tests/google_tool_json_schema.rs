#![cfg(all(feature = "providers-extra", feature = "providers-extended"))]

use litellm_rs::core::providers::gemini::{GeminiClient, GeminiConfig};
use litellm_rs::core::providers::vertex_ai::{
    models::VertexAIModel, transformers::GeminiTransformer,
};
use litellm_rs::core::types::{
    chat::{ChatMessage, ChatRequest},
    message::{MessageContent, MessageRole},
    tools::{FunctionDefinition, Tool, ToolType},
};
use serde_json::json;

#[test]
fn google_requests_preserve_json_schema_for_modern_and_legacy_tools() {
    let gemini = GeminiClient::new(GeminiConfig::new_google_ai("test-key")).unwrap();
    let vertex = GeminiTransformer::new();
    // `const` is not part of Google's typed Schema. Passing this JSON Schema
    // through `parameters` fails in the official SDK before a request is sent.
    let schema = json!({
        "type": "object",
        "properties": {"state": {"const": "ready"}},
        "additionalProperties": false
    });
    let requests = [
        ChatRequest {
            tools: Some(vec![Tool {
                tool_type: ToolType::Function,
                function: FunctionDefinition {
                    name: "get_state".to_string(),
                    description: None,
                    parameters: Some(schema.clone()),
                },
            }]),
            ..Default::default()
        },
        ChatRequest {
            functions: Some(vec![json!({"name": "get_state", "parameters": schema})]),
            ..Default::default()
        },
    ];
    for mut request in requests {
        request.model = "gemini-2.5-flash".to_string();
        request.messages = vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Check state".to_string())),
            ..Default::default()
        }];
        let bodies = [
            ("gemini", gemini.transform_chat_request(&request).unwrap()),
            (
                "vertex",
                vertex
                    .transform_chat_request(&request, &VertexAIModel::GeminiPro)
                    .unwrap(),
            ),
        ];
        for (provider, body) in bodies {
            let declaration = &body["tools"][0]["functionDeclarations"][0];
            assert_eq!(declaration["parametersJsonSchema"], schema, "{provider}");
            assert!(declaration.get("parameters").is_none(), "{provider}");
        }
    }
}

#[test]
fn google_requests_leave_schema_unset_for_parameterless_tools() {
    let gemini = GeminiClient::new(GeminiConfig::new_google_ai("test-key")).unwrap();
    let vertex = GeminiTransformer::new();
    let request = ChatRequest {
        model: "gemini-2.5-flash".to_string(),
        messages: vec![ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Ping".to_string())),
            ..Default::default()
        }],
        tools: Some(vec![Tool {
            tool_type: ToolType::Function,
            function: FunctionDefinition {
                name: "ping".to_string(),
                description: None,
                parameters: None,
            },
        }]),
        ..Default::default()
    };
    let bodies = [
        ("gemini", gemini.transform_chat_request(&request).unwrap()),
        (
            "vertex",
            vertex
                .transform_chat_request(&request, &VertexAIModel::GeminiPro)
                .unwrap(),
        ),
    ];
    for (provider, body) in bodies {
        let declaration = &body["tools"][0]["functionDeclarations"][0];
        assert_eq!(declaration["name"], "ping", "{provider}");
        assert!(declaration.get("parameters").is_none(), "{provider}");
        assert!(
            declaration.get("parametersJsonSchema").is_none(),
            "{provider}"
        );
    }
}
