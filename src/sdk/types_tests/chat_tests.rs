use crate::sdk::types::*;

// ==================== ChatOptions Tests ====================

#[test]
fn test_chat_options_default() {
    let options = ChatOptions::default();
    assert!(options.temperature.is_none());
    assert!(options.max_tokens.is_none());
    assert!(options.top_p.is_none());
    assert!(options.frequency_penalty.is_none());
    assert!(options.presence_penalty.is_none());
    assert!(options.stop.is_none());
    assert!(!options.stream);
    assert!(options.tools.is_none());
    assert!(options.tool_choice.is_none());
}

#[test]
fn test_chat_options_with_values() {
    let options = ChatOptions {
        temperature: Some(0.7),
        max_tokens: Some(1000),
        top_p: Some(0.9),
        frequency_penalty: Some(0.5),
        presence_penalty: Some(0.5),
        stop: Some(vec!["STOP".to_string()]),
        stream: true,
        tools: None,
        tool_choice: None,
    };
    assert_eq!(options.temperature, Some(0.7));
    assert_eq!(options.max_tokens, Some(1000));
    assert_eq!(options.top_p, Some(0.9));
    assert!(options.stream);
}

#[test]
fn test_chat_options_clone() {
    let options = ChatOptions {
        temperature: Some(0.5),
        max_tokens: Some(500),
        ..Default::default()
    };
    let cloned = options.clone();
    assert_eq!(options.temperature, cloned.temperature);
    assert_eq!(options.max_tokens, cloned.max_tokens);
}

// ==================== SdkChatRequest Tests ====================

#[test]
fn test_chat_request_creation() {
    let request = SdkChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![Message {
            role: Role::User,
            content: Some(Content::Text("Hello".to_string())),
            name: None,
            tool_calls: None,
        }],
        options: ChatOptions::default(),
    };
    assert_eq!(request.model, "gpt-4");
    assert_eq!(request.messages.len(), 1);
}

#[test]
fn test_chat_request_multiple_messages() {
    let request = SdkChatRequest {
        model: "claude-3-opus".to_string(),
        messages: vec![
            Message {
                role: Role::System,
                content: Some(Content::Text("You are helpful.".to_string())),
                name: None,
                tool_calls: None,
            },
            Message {
                role: Role::User,
                content: Some(Content::Text("Hi".to_string())),
                name: None,
                tool_calls: None,
            },
        ],
        options: ChatOptions::default(),
    };
    assert_eq!(request.messages.len(), 2);
    assert_eq!(request.messages[0].role, Role::System);
    assert_eq!(request.messages[1].role, Role::User);
}

#[test]
fn test_chat_request_clone() {
    let request = SdkChatRequest {
        model: "gpt-4".to_string(),
        messages: vec![],
        options: ChatOptions::default(),
    };
    let cloned = request.clone();
    assert_eq!(request.model, cloned.model);
}

// ==================== ChatResponse Tests ====================

#[test]
fn test_chat_response_creation() {
    let response = ChatResponse {
        id: "resp_123".to_string(),
        model: "gpt-4".to_string(),
        choices: vec![],
        usage: Usage::default(),
        created: 1234567890,
    };
    assert_eq!(response.id, "resp_123");
    assert_eq!(response.model, "gpt-4");
    assert_eq!(response.created, 1234567890);
}

#[test]
fn test_chat_response_with_choices() {
    let response = ChatResponse {
        id: "resp_456".to_string(),
        model: "gpt-4".to_string(),
        choices: vec![ChatChoice {
            index: 0,
            message: Message {
                role: Role::Assistant,
                content: Some(Content::Text("Hello!".to_string())),
                name: None,
                tool_calls: None,
            },
            finish_reason: Some("stop".to_string()),
        }],
        usage: Usage {
            prompt_tokens: 10,
            completion_tokens: 5,
            total_tokens: 15,
        },
        created: 1234567890,
    };
    assert_eq!(response.choices.len(), 1);
    assert_eq!(response.choices[0].index, 0);
    assert_eq!(response.choices[0].finish_reason, Some("stop".to_string()));
}

#[test]
fn test_chat_response_clone() {
    let response = ChatResponse {
        id: "resp_789".to_string(),
        model: "gpt-4".to_string(),
        choices: vec![],
        usage: Usage::default(),
        created: 0,
    };
    let cloned = response.clone();
    assert_eq!(response.id, cloned.id);
}

// ==================== ChatChoice Tests ====================

#[test]
fn test_chat_choice_creation() {
    let choice = ChatChoice {
        index: 0,
        message: Message {
            role: Role::Assistant,
            content: Some(Content::Text("Response".to_string())),
            name: None,
            tool_calls: None,
        },
        finish_reason: Some("stop".to_string()),
    };
    assert_eq!(choice.index, 0);
    assert_eq!(choice.finish_reason, Some("stop".to_string()));
}

#[test]
fn test_chat_choice_no_finish_reason() {
    let choice = ChatChoice {
        index: 1,
        message: Message {
            role: Role::Assistant,
            content: None,
            name: None,
            tool_calls: None,
        },
        finish_reason: None,
    };
    assert!(choice.finish_reason.is_none());
}
