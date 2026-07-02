use super::create_test_provider;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::content::ContentPart;
use crate::core::types::{chat::ChatMessage, message::MessageContent, message::MessageRole};
use std::collections::HashMap;

// ==================== Messages to Prompt Conversion Tests ====================

#[test]
fn test_messages_to_prompt_simple_user_message() {
    let provider = create_test_provider();

    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text("Hello, how are you?".to_string())),
        ..Default::default()
    }];

    let prompt = provider.messages_to_prompt(&messages).unwrap();
    assert!(prompt.contains("Human: Hello, how are you?"));
    assert!(prompt.ends_with("Assistant:"));
}

#[test]
fn test_messages_to_prompt_system_message() {
    let provider = create_test_provider();

    let messages = vec![
        ChatMessage {
            role: MessageRole::System,
            content: Some(MessageContent::Text(
                "You are a helpful assistant.".to_string(),
            )),
            ..Default::default()
        },
        ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        },
    ];

    let prompt = provider.messages_to_prompt(&messages).unwrap();
    assert!(prompt.contains("System: You are a helpful assistant."));
    assert!(prompt.contains("Human: Hello"));
}

#[test]
fn test_messages_to_prompt_assistant_message() {
    let provider = create_test_provider();

    let messages = vec![
        ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            ..Default::default()
        },
        ChatMessage {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text("Hi there!".to_string())),
            ..Default::default()
        },
        ChatMessage {
            role: MessageRole::User,
            content: Some(MessageContent::Text("How are you?".to_string())),
            ..Default::default()
        },
    ];

    let prompt = provider.messages_to_prompt(&messages).unwrap();
    assert!(prompt.contains("Human: Hello"));
    assert!(prompt.contains("Assistant: Hi there!"));
    assert!(prompt.contains("Human: How are you?"));
}

#[test]
fn test_messages_to_prompt_tool_message() {
    let provider = create_test_provider();

    let messages = vec![ChatMessage {
        role: MessageRole::Tool,
        content: Some(MessageContent::Text("Tool output".to_string())),
        ..Default::default()
    }];

    let prompt = provider.messages_to_prompt(&messages).unwrap();
    assert!(prompt.contains("Tool: Tool output"));
}

#[test]
fn test_messages_to_prompt_function_message() {
    let provider = create_test_provider();

    let messages = vec![ChatMessage {
        role: MessageRole::Function,
        content: Some(MessageContent::Text("Function result".to_string())),
        ..Default::default()
    }];

    let prompt = provider.messages_to_prompt(&messages).unwrap();
    assert!(prompt.contains("Tool: Function result"));
}

#[test]
fn test_messages_to_prompt_with_content_parts() {
    let provider = create_test_provider();

    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Parts(vec![
            ContentPart::Text {
                text: "Hello".to_string(),
            },
            ContentPart::Text {
                text: "World".to_string(),
            },
        ])),
        ..Default::default()
    }];

    let prompt = provider.messages_to_prompt(&messages).unwrap();
    assert!(prompt.contains("Human: Hello World"));
}

#[test]
fn test_messages_to_prompt_none_content() {
    let provider = create_test_provider();

    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: None,
        ..Default::default()
    }];

    let prompt = provider.messages_to_prompt(&messages).unwrap();
    assert!(prompt.ends_with("Assistant:"));
}

// ==================== OpenAI Params Mapping Tests ====================

#[tokio::test]
async fn test_map_openai_params_max_tokens() {
    let provider = create_test_provider();

    let mut params = HashMap::new();
    params.insert(
        "max_tokens".to_string(),
        serde_json::Value::Number(100.into()),
    );

    let mapped = provider
        .map_openai_params(params, "anthropic.claude-3-sonnet-20240229")
        .await
        .unwrap();

    assert!(mapped.contains_key("max_tokens_to_sample"));
    assert_eq!(
        mapped.get("max_tokens_to_sample").unwrap(),
        &serde_json::Value::Number(100.into())
    );
}

#[tokio::test]
async fn test_map_openai_params_temperature() {
    let provider = create_test_provider();

    let mut params = HashMap::new();
    params.insert("temperature".to_string(), serde_json::json!(0.7));

    let mapped = provider
        .map_openai_params(params, "anthropic.claude-3-sonnet-20240229")
        .await
        .unwrap();

    assert!(mapped.contains_key("temperature"));
}

#[tokio::test]
async fn test_map_openai_params_unsupported_ignored() {
    let provider = create_test_provider();

    let mut params = HashMap::new();
    params.insert(
        "unsupported_param".to_string(),
        serde_json::Value::String("value".to_string()),
    );

    let mapped = provider
        .map_openai_params(params, "anthropic.claude-3-sonnet-20240229")
        .await
        .unwrap();

    assert!(!mapped.contains_key("unsupported_param"));
}

#[tokio::test]
async fn test_map_openai_params_multiple() {
    let provider = create_test_provider();

    let mut params = HashMap::new();
    params.insert("temperature".to_string(), serde_json::json!(0.5));
    params.insert("top_p".to_string(), serde_json::json!(0.9));
    params.insert("stream".to_string(), serde_json::Value::Bool(true));
    params.insert("stop".to_string(), serde_json::json!(["END"]));

    let mapped = provider
        .map_openai_params(params, "anthropic.claude-3-sonnet-20240229")
        .await
        .unwrap();

    assert!(mapped.contains_key("temperature"));
    assert!(mapped.contains_key("top_p"));
    assert!(mapped.contains_key("stream"));
    assert!(mapped.contains_key("stop"));
}
