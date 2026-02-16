#[cfg(test)]
use super::types::{MessageContent, RequestUtils};

#[test]
fn test_message_validation() {
    let valid_messages = vec![
        MessageContent {
            role: "user".to_string(),
            content: "Hello".to_string(),
        },
        MessageContent {
            role: "assistant".to_string(),
            content: "Hi there!".to_string(),
        },
    ];

    assert!(RequestUtils::validate_chat_completion_messages(&valid_messages).is_ok());

    let empty_messages: Vec<MessageContent> = vec![];
    assert!(RequestUtils::validate_chat_completion_messages(&empty_messages).is_err());
}

#[test]
fn test_invalid_role() {
    let invalid_messages = vec![MessageContent {
        role: "invalid_role".to_string(),
        content: "Hello".to_string(),
    }];

    assert!(RequestUtils::validate_chat_completion_messages(&invalid_messages).is_err());
}

#[test]
fn test_system_message_processing() {
    let system_msg = "You are a helpful assistant.";
    let processed = RequestUtils::process_system_message(system_msg, None, "gpt-4").unwrap();
    assert_eq!(processed, system_msg);

    let claude_processed =
        RequestUtils::process_system_message(system_msg, None, "claude-3").unwrap();
    assert_eq!(claude_processed, system_msg);
}

#[test]
fn test_tool_choice_validation() {
    let tools = Some(vec![serde_json::json!({
        "type": "function",
        "function": {
            "name": "test_function",
            "description": "A test function"
        }
    })]);

    assert!(RequestUtils::validate_tool_choice(&Some("auto".to_string()), &tools).is_ok());
    assert!(RequestUtils::validate_tool_choice(&Some("none".to_string()), &tools).is_ok());
    assert!(RequestUtils::validate_tool_choice(&Some("test_function".to_string()), &tools).is_ok());
    assert!(
        RequestUtils::validate_tool_choice(&Some("invalid_function".to_string()), &tools).is_err()
    );
    assert!(RequestUtils::validate_tool_choice(&Some("auto".to_string()), &None).is_err());
}

#[test]
fn test_message_truncation() {
    let long_message = "a".repeat(1000);
    let truncated = RequestUtils::truncate_message(&long_message, 100).unwrap();
    assert!(truncated.len() <= 100);
    assert!(truncated.ends_with("..."));
}

#[test]
fn test_has_tool_call_blocks() {
    let messages_with_tools = vec![MessageContent {
        role: "assistant".to_string(),
        content: "Here's a tool_calls example".to_string(),
    }];

    let messages_without_tools = vec![MessageContent {
        role: "user".to_string(),
        content: "Hello".to_string(),
    }];

    assert!(RequestUtils::has_tool_call_blocks(&messages_with_tools));
    assert!(!RequestUtils::has_tool_call_blocks(&messages_without_tools));
}

#[test]
fn test_convert_messages_to_dict() {
    let messages = vec![MessageContent {
        role: "user".to_string(),
        content: "Hello".to_string(),
    }];

    let dict = RequestUtils::convert_messages_to_dict(&messages);
    assert_eq!(dict.len(), 1);
    assert_eq!(dict[0].get("role").unwrap().as_str().unwrap(), "user");
    assert_eq!(dict[0].get("content").unwrap().as_str().unwrap(), "Hello");
}
