//! Tests for validation utilities

#[cfg(test)]
use crate::core::models::openai::{ChatMessage, MessageContent, MessageRole};
use crate::utils::data::validation::{ApiValidator, DataValidator, RequestValidator};
use std::collections::HashMap;

// ==================== RequestValidator Tests ====================

fn create_user_message(content: &str) -> ChatMessage {
    ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text(content.to_string())),
        name: None,
        tool_calls: None,
        tool_call_id: None,
        function_call: None,
        audio: None,
    }
}

fn create_system_message(content: &str) -> ChatMessage {
    ChatMessage {
        role: MessageRole::System,
        content: Some(MessageContent::Text(content.to_string())),
        name: None,
        tool_calls: None,
        tool_call_id: None,
        function_call: None,
        audio: None,
    }
}

fn create_assistant_message(content: &str) -> ChatMessage {
    ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text(content.to_string())),
        name: None,
        tool_calls: None,
        tool_call_id: None,
        function_call: None,
        audio: None,
    }
}

#[test]
fn test_model_name_validation() {
    let message = create_user_message("test");

    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_ok()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "claude-3.5-sonnet",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_ok()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_err()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "invalid@model",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_err()
    );
}

#[test]
fn test_model_name_special_chars() {
    let message = create_user_message("test");

    // Valid model names with allowed characters
    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4-turbo",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_ok()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "anthropic/claude-3",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_ok()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "model.v1.2",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_ok()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "model_name_v2",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_ok()
    );

    // Invalid model names
    assert!(
        RequestValidator::validate_chat_completion_request(
            "model name",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_err()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "model#name",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_err()
    );
}

#[test]
fn test_empty_messages() {
    let messages: Vec<ChatMessage> = vec![];
    assert!(
        RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, None).is_err()
    );
}

#[test]
fn test_max_tokens_validation() {
    let message = create_user_message("test");

    // Valid max_tokens
    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            Some(100),
            None
        )
        .is_ok()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            Some(100000),
            None
        )
        .is_ok()
    );

    // Invalid max_tokens
    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            Some(0),
            None
        )
        .is_err()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            Some(100001),
            None
        )
        .is_err()
    );
}

#[test]
fn test_temperature_validation() {
    let message = create_user_message("test");

    // Valid temperatures
    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            Some(0.0)
        )
        .is_ok()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            Some(1.0)
        )
        .is_ok()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            Some(2.0)
        )
        .is_ok()
    );

    // Invalid temperatures
    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            Some(-0.1)
        )
        .is_err()
    );
    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            Some(2.1)
        )
        .is_err()
    );
}

#[test]
fn test_multiple_messages() {
    let messages = vec![
        create_system_message("You are a helpful assistant"),
        create_user_message("Hello"),
        create_assistant_message("Hi there!"),
        create_user_message("How are you?"),
    ];

    assert!(
        RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, None).is_ok()
    );
}

#[test]
fn test_message_without_content() {
    let message = ChatMessage {
        role: MessageRole::User,
        content: None,
        name: None,
        tool_calls: None,
        tool_call_id: None,
        function_call: None,
        audio: None,
    };

    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_err()
    );
}

#[test]
fn test_empty_text_content() {
    let message = ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text("   ".to_string())),
        name: None,
        tool_calls: None,
        tool_call_id: None,
        function_call: None,
        audio: None,
    };

    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_err()
    );
}

#[test]
fn test_function_message_validation() {
    // Function message without name
    let message = ChatMessage {
        role: MessageRole::Function,
        content: Some(MessageContent::Text("result".to_string())),
        name: None,
        tool_calls: None,
        tool_call_id: None,
        function_call: None,
        audio: None,
    };

    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_err()
    );

    // Function message with name
    let message = ChatMessage {
        role: MessageRole::Function,
        content: Some(MessageContent::Text("result".to_string())),
        name: Some("get_weather".to_string()),
        tool_calls: None,
        tool_call_id: None,
        function_call: None,
        audio: None,
    };

    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_ok()
    );
}

#[test]
fn test_tool_message_validation() {
    // Tool message without tool_call_id
    let message = ChatMessage {
        role: MessageRole::Tool,
        content: Some(MessageContent::Text("result".to_string())),
        name: None,
        tool_calls: None,
        tool_call_id: None,
        function_call: None,
        audio: None,
    };

    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_err()
    );

    // Tool message with tool_call_id
    let message = ChatMessage {
        role: MessageRole::Tool,
        content: Some(MessageContent::Text("result".to_string())),
        name: None,
        tool_calls: None,
        tool_call_id: Some("call_123".to_string()),
        function_call: None,
        audio: None,
    };

    assert!(
        RequestValidator::validate_chat_completion_request(
            "gpt-4",
            std::slice::from_ref(&message),
            None,
            None
        )
        .is_ok()
    );
}

// ==================== ApiValidator Tests ====================

#[test]
fn test_api_key_validation() {
    assert!(ApiValidator::validate_api_key("valid_api_key_123").is_ok());
    assert!(ApiValidator::validate_api_key("").is_err());
    assert!(ApiValidator::validate_api_key("short").is_err());
}

#[test]
fn test_api_key_length_boundaries() {
    // Too short (< 10)
    assert!(ApiValidator::validate_api_key("123456789").is_err());

    // Exactly 10 characters
    assert!(ApiValidator::validate_api_key("1234567890").is_ok());

    // Very long but valid (< 200)
    let long_key = "a".repeat(199);
    assert!(ApiValidator::validate_api_key(&long_key).is_ok());

    // Exactly 200 characters
    let long_key = "a".repeat(200);
    assert!(ApiValidator::validate_api_key(&long_key).is_ok());

    // Too long (> 200)
    let too_long = "a".repeat(201);
    assert!(ApiValidator::validate_api_key(&too_long).is_err());
}

#[test]
fn test_api_key_whitespace() {
    assert!(ApiValidator::validate_api_key("   ").is_err());
    assert!(ApiValidator::validate_api_key("  \t\n  ").is_err());
}

#[test]
fn test_uuid_validation() {
    let valid_uuid = "550e8400-e29b-41d4-a716-446655440000";
    assert!(ApiValidator::validate_uuid(valid_uuid).is_ok());
    assert!(ApiValidator::validate_uuid("invalid-uuid").is_err());
}

#[test]
fn test_uuid_various_formats() {
    // Standard format
    assert!(ApiValidator::validate_uuid("550e8400-e29b-41d4-a716-446655440000").is_ok());

    // Simple format (without dashes) is also valid for uuid crate
    assert!(ApiValidator::validate_uuid("550e8400e29b41d4a716446655440000").is_ok());

    // Invalid formats
    assert!(ApiValidator::validate_uuid("not-a-uuid").is_err());
    assert!(ApiValidator::validate_uuid("").is_err());
    assert!(ApiValidator::validate_uuid("550e8400-e29b-41d4-a716-44665544000g").is_err()); // Invalid char
    assert!(ApiValidator::validate_uuid("gggggggg-gggg-gggg-gggg-gggggggggggg").is_err()); // All invalid chars
    assert!(ApiValidator::validate_uuid("550e8400-e29b-41d4-a716").is_err()); // Too short
}

#[test]
fn test_pagination_validation() {
    assert!(ApiValidator::validate_pagination(Some(1), Some(20)).is_ok());
    assert!(ApiValidator::validate_pagination(Some(0), Some(20)).is_err());
    assert!(ApiValidator::validate_pagination(Some(1), Some(0)).is_err());
    assert!(ApiValidator::validate_pagination(Some(1), Some(2000)).is_err());
}

#[test]
fn test_pagination_defaults() {
    let result = ApiValidator::validate_pagination(None, None);
    assert!(result.is_ok());
    let (page, limit) = result.unwrap();
    assert_eq!(page, 1);
    assert_eq!(limit, 20);
}

#[test]
fn test_pagination_limit_boundaries() {
    // Valid limit at boundary
    assert!(ApiValidator::validate_pagination(Some(1), Some(1000)).is_ok());

    // Invalid limit
    assert!(ApiValidator::validate_pagination(Some(1), Some(1001)).is_err());
}

#[test]
fn test_date_range_validation() {
    use chrono::{Duration, Utc};

    let now = Utc::now();
    let yesterday = now - Duration::days(1);
    let tomorrow = now + Duration::days(1);

    // Valid date range
    assert!(ApiValidator::validate_date_range(Some(yesterday), Some(tomorrow)).is_ok());

    // None values are valid
    assert!(ApiValidator::validate_date_range(None, None).is_ok());
    assert!(ApiValidator::validate_date_range(Some(now), None).is_ok());
    assert!(ApiValidator::validate_date_range(None, Some(now)).is_ok());

    // Start after end
    assert!(ApiValidator::validate_date_range(Some(tomorrow), Some(yesterday)).is_err());

    // Start equals end
    assert!(ApiValidator::validate_date_range(Some(now), Some(now)).is_err());
}

#[test]
fn test_date_range_max_range() {
    use chrono::{Duration, Utc};

    let now = Utc::now();

    // Just under 365 days
    let start = now - Duration::days(364);
    assert!(ApiValidator::validate_date_range(Some(start), Some(now)).is_ok());

    // Exactly 365 days
    let start = now - Duration::days(365);
    assert!(ApiValidator::validate_date_range(Some(start), Some(now)).is_ok());

    // Over 365 days
    let start = now - Duration::days(366);
    assert!(ApiValidator::validate_date_range(Some(start), Some(now)).is_err());
}

#[test]
fn test_sort_params_validation() {
    let valid_fields = &["name", "created_at", "updated_at"];

    // Valid combinations
    assert!(ApiValidator::validate_sort_params("name", "asc", valid_fields).is_ok());
    assert!(ApiValidator::validate_sort_params("created_at", "desc", valid_fields).is_ok());

    // Invalid field
    assert!(ApiValidator::validate_sort_params("invalid_field", "asc", valid_fields).is_err());

    // Invalid order
    assert!(ApiValidator::validate_sort_params("name", "invalid", valid_fields).is_err());
    assert!(ApiValidator::validate_sort_params("name", "ASC", valid_fields).is_err());
}

#[test]
fn test_filter_validation() {
    let valid_filters = &["status", "type", "user_id"];

    let mut filters = HashMap::new();
    filters.insert("status".to_string(), "active".to_string());
    filters.insert("type".to_string(), "chat".to_string());
    assert!(ApiValidator::validate_filters(&filters, valid_filters).is_ok());

    let mut invalid_filters = HashMap::new();
    invalid_filters.insert("invalid_filter".to_string(), "value".to_string());
    assert!(ApiValidator::validate_filters(&invalid_filters, valid_filters).is_err());
}

#[test]
fn test_filter_validation_empty() {
    let valid_filters = &["status", "type"];
    let filters: HashMap<String, String> = HashMap::new();
    assert!(ApiValidator::validate_filters(&filters, valid_filters).is_ok());
}

// ==================== DataValidator Tests ====================

#[test]
fn test_username_validation() {
    assert!(DataValidator::validate_username("valid_user").is_ok());
    assert!(DataValidator::validate_username("user123").is_ok());
    assert!(DataValidator::validate_username("").is_err());
    assert!(DataValidator::validate_username("ab").is_err());
    assert!(DataValidator::validate_username("invalid@user").is_err());
}

#[test]
fn test_username_length_boundaries() {
    // Too short (< 3)
    assert!(DataValidator::validate_username("ab").is_err());

    // Exactly 3 characters
    assert!(DataValidator::validate_username("abc").is_ok());

    // Valid long username
    let long_name = "a".repeat(50);
    assert!(DataValidator::validate_username(&long_name).is_ok());

    // Too long (> 50)
    let too_long = "a".repeat(51);
    assert!(DataValidator::validate_username(&too_long).is_err());
}

#[test]
fn test_username_special_chars() {
    // Allowed characters
    assert!(DataValidator::validate_username("user_name").is_ok());
    assert!(DataValidator::validate_username("user-name").is_ok());
    assert!(DataValidator::validate_username("User123").is_ok());

    // Disallowed characters
    assert!(DataValidator::validate_username("user@name").is_err());
    assert!(DataValidator::validate_username("user.name").is_err());
    assert!(DataValidator::validate_username("user name").is_err());
    assert!(DataValidator::validate_username("user#name").is_err());
}

#[test]
fn test_password_validation() {
    assert!(DataValidator::validate_password("StrongPass123!").is_ok());
    assert!(DataValidator::validate_password("NoSpecialChars123").is_ok()); // Has 3 types: upper, lower, digit
    assert!(DataValidator::validate_password("weak").is_err()); // Too short
    assert!(DataValidator::validate_password("onlylowercase").is_err()); // Only 1 type
    assert!(DataValidator::validate_password("ONLYUPPERCASE").is_err()); // Only 1 type
    assert!(DataValidator::validate_password("OnlyTwoTypes").is_err()); // Only 2 types: upper, lower
}

#[test]
fn test_password_length_boundaries() {
    // Too short (< 8)
    assert!(DataValidator::validate_password("Pass1!").is_err());

    // Exactly 8 characters with 3 types
    assert!(DataValidator::validate_password("Pass123!").is_ok());

    // Long password
    let long_pass = format!("{}Aa1!", "a".repeat(120));
    assert!(DataValidator::validate_password(&long_pass).is_ok());

    // Too long (> 128)
    let too_long = format!("{}Aa1!", "a".repeat(130));
    assert!(DataValidator::validate_password(&too_long).is_err());
}

#[test]
fn test_password_character_types() {
    // Only lowercase + digit (2 types) - should fail
    assert!(DataValidator::validate_password("password123").is_err());

    // lowercase + uppercase + digit (3 types) - should pass
    assert!(DataValidator::validate_password("Password123").is_ok());

    // lowercase + uppercase + special (3 types) - should pass
    assert!(DataValidator::validate_password("Password!@#").is_ok());

    // All 4 types - should pass
    assert!(DataValidator::validate_password("Password123!").is_ok());

    // Only special chars - should fail
    assert!(DataValidator::validate_password("!@#$%^&*").is_err());
}

#[test]
fn test_team_name_validation() {
    // Valid names
    assert!(DataValidator::validate_team_name("My Team").is_ok());
    assert!(DataValidator::validate_team_name("ab").is_ok());
    assert!(DataValidator::validate_team_name("Team with spaces and 123").is_ok());

    // Invalid names
    assert!(DataValidator::validate_team_name("").is_err());
    assert!(DataValidator::validate_team_name("   ").is_err());
    assert!(DataValidator::validate_team_name("a").is_err());
}

#[test]
fn test_team_name_length_boundaries() {
    // Too short (< 2)
    assert!(DataValidator::validate_team_name("a").is_err());

    // Exactly 2 characters
    assert!(DataValidator::validate_team_name("ab").is_ok());

    // Valid long name (100 chars)
    let long_name = "a".repeat(100);
    assert!(DataValidator::validate_team_name(&long_name).is_ok());

    // Too long (> 100)
    let too_long = "a".repeat(101);
    assert!(DataValidator::validate_team_name(&too_long).is_err());
}

#[test]
fn test_tags_validation() {
    assert!(DataValidator::validate_tags(&["tag1".to_string(), "tag2".to_string()]).is_ok());
    assert!(DataValidator::validate_tags(&["".to_string()]).is_err());
    assert!(DataValidator::validate_tags(&["tag1".to_string(), "tag1".to_string()]).is_err());
}

#[test]
fn test_tags_empty_list() {
    let empty_tags: Vec<String> = vec![];
    assert!(DataValidator::validate_tags(&empty_tags).is_ok());
}

#[test]
fn test_tags_max_count() {
    // Valid: 20 tags
    let tags: Vec<String> = (0..20).map(|i| format!("tag{}", i)).collect();
    assert!(DataValidator::validate_tags(&tags).is_ok());

    // Invalid: 21 tags
    let too_many: Vec<String> = (0..21).map(|i| format!("tag{}", i)).collect();
    assert!(DataValidator::validate_tags(&too_many).is_err());
}

#[test]
fn test_tags_length() {
    // Valid tag length (50 chars)
    let long_tag = "a".repeat(50);
    assert!(DataValidator::validate_tags(&[long_tag]).is_ok());

    // Invalid tag length (> 50)
    let too_long = "a".repeat(51);
    assert!(DataValidator::validate_tags(&[too_long]).is_err());
}

#[test]
fn test_tags_case_insensitive_duplicates() {
    // Same tag with different case should be considered duplicate
    assert!(DataValidator::validate_tags(&["Tag1".to_string(), "tag1".to_string()]).is_err());
    assert!(DataValidator::validate_tags(&["TAG".to_string(), "tag".to_string()]).is_err());
}

#[test]
fn test_tags_whitespace() {
    // Whitespace-only tags should fail
    assert!(DataValidator::validate_tags(&["   ".to_string()]).is_err());
    assert!(DataValidator::validate_tags(&["\t".to_string()]).is_err());
}
