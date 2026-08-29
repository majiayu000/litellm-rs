use super::*;
use crate::core::models::openai::{
    ChatMessage, ContentPart, FunctionCall, ImageUrl, MessageContent, MessageRole, ToolCall,
};
use crate::core::types::anthropic_continuation::{
    AnthropicSignature, AnthropicThinkingBlock, AnthropicThinkingContent, ChatMessageExtensions,
};

// ==================== Helper Functions ====================

fn create_user_message(content: &str) -> ChatMessage {
    ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text(content.to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }
}

fn create_system_message(content: &str) -> ChatMessage {
    ChatMessage {
        role: MessageRole::System,
        content: Some(MessageContent::Text(content.to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }
}

fn create_assistant_message(content: &str) -> ChatMessage {
    ChatMessage {
        role: MessageRole::Assistant,
        content: Some(MessageContent::Text(content.to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }
}

// ==================== Chat Completion Validation Tests ====================

#[test]
fn test_validate_chat_completion_valid() {
    let messages = vec![create_user_message("Hello")];
    let result = RequestValidator::validate_chat_completion_request(
        "gpt-4",
        &messages,
        Some(100),
        Some(0.7),
    );
    assert!(result.is_ok());
}

#[test]
fn test_validate_chat_completion_empty_messages() {
    let messages: Vec<ChatMessage> = vec![];
    let result = RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

#[test]
fn test_validate_chat_completion_multiple_messages() {
    let messages = vec![
        create_system_message("You are helpful"),
        create_user_message("Hello"),
        create_assistant_message("Hi there!"),
    ];
    let result = RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, None);
    assert!(result.is_ok());
}

#[test]
fn assistant_tool_only_is_meaningful_but_truly_empty_stays_rejected() {
    let tool_only = ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        name: None,
        function_call: None,
        tool_calls: Some(vec![ToolCall {
            id: "toolu_1".to_string(),
            tool_type: "function".to_string(),
            function: FunctionCall {
                name: "lookup".to_string(),
                arguments: "{}".to_string(),
            },
        }]),
        tool_call_id: None,
        audio: None,
    };
    assert!(
        RequestValidator::validate_chat_completion_request(
            "claude-opus-5",
            &[tool_only],
            None,
            None,
        )
        .is_ok()
    );

    let empty = ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        name: None,
        function_call: None,
        tool_calls: Some(Vec::new()),
        tool_call_id: None,
        audio: None,
    };
    assert!(
        RequestValidator::validate_chat_completion_request("claude-opus-5", &[empty], None, None,)
            .is_err()
    );
}

#[test]
fn assistant_extension_only_is_meaningful_and_role_bounded() {
    let message = ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    };
    let extension =
        ChatMessageExtensions::new().with_anthropic_thinking(AnthropicThinkingContent::new(vec![
            AnthropicThinkingBlock::Thinking {
                thinking: "plan".to_string(),
                signature: AnthropicSignature::try_from("opaque-signature")
                    .expect("fixture signature is non-empty"),
            },
        ]));
    assert!(
        RequestValidator::validate_chat_completion_request_with_extensions(
            "claude-opus-5",
            std::slice::from_ref(&message),
            std::slice::from_ref(&extension),
            None,
            None,
        )
        .is_ok()
    );

    let mut user = message;
    user.role = MessageRole::User;
    assert!(
        RequestValidator::validate_chat_completion_request_with_extensions(
            "claude-opus-5",
            &[user],
            &[extension],
            None,
            None,
        )
        .is_err()
    );
}

#[test]
fn empty_anthropic_thinking_does_not_make_an_empty_assistant_message_meaningful() {
    let message = ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    };
    let extension = ChatMessageExtensions::new()
        .with_anthropic_thinking(AnthropicThinkingContent::new(Vec::new()));

    assert!(
        RequestValidator::validate_chat_completion_request_with_extensions(
            "claude-fable-5",
            std::slice::from_ref(&message),
            std::slice::from_ref(&extension),
            None,
            None,
        )
        .is_err()
    );
}

// ==================== Max Tokens Validation Tests ====================

#[test]
fn test_validate_max_tokens_zero() {
    let messages = vec![create_user_message("Hello")];
    let result =
        RequestValidator::validate_chat_completion_request("gpt-4", &messages, Some(0), None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("max_tokens"));
}

#[test]
fn test_validate_max_tokens_too_large() {
    let messages = vec![create_user_message("Hello")];
    let result =
        RequestValidator::validate_chat_completion_request("gpt-4", &messages, Some(100001), None);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("100000"));
}

#[test]
fn test_validate_max_tokens_valid_boundary() {
    let messages = vec![create_user_message("Hello")];
    let result =
        RequestValidator::validate_chat_completion_request("gpt-4", &messages, Some(100000), None);
    assert!(result.is_ok());
}

#[test]
fn exact_fable_uses_its_advertised_output_limit() {
    let messages = vec![create_user_message("Hello")];

    for max_tokens in [100_001, 128_000] {
        let result = RequestValidator::validate_chat_completion_request(
            "claude-fable-5",
            &messages,
            Some(max_tokens),
            None,
        );
        assert!(result.is_ok(), "exact Fable should accept {max_tokens}");
    }

    let too_large = RequestValidator::validate_chat_completion_request(
        "claude-fable-5",
        &messages,
        Some(128_001),
        None,
    )
    .expect_err("Fable must reject output above 128k");
    assert!(too_large.to_string().contains("128000"));

    for model in ["gpt-4", "Claude-fable-5", "claude-fable-5-latest"] {
        let error = RequestValidator::validate_chat_completion_request(
            model,
            &messages,
            Some(100_001),
            None,
        )
        .expect_err("the Fable exception must be exact and case-sensitive");
        assert!(error.to_string().contains("100000"));
    }
}

#[test]
fn test_validate_max_tokens_none() {
    let messages = vec![create_user_message("Hello")];
    let result = RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, None);
    assert!(result.is_ok());
}

// ==================== Temperature Validation Tests ====================

#[test]
fn test_validate_temperature_valid() {
    let messages = vec![create_user_message("Hello")];
    let result =
        RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, Some(1.0));
    assert!(result.is_ok());
}

#[test]
fn test_validate_temperature_zero() {
    let messages = vec![create_user_message("Hello")];
    let result =
        RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, Some(0.0));
    assert!(result.is_ok());
}

#[test]
fn test_validate_temperature_max() {
    let messages = vec![create_user_message("Hello")];
    let result =
        RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, Some(2.0));
    assert!(result.is_ok());
}

#[test]
fn test_validate_temperature_too_low() {
    let messages = vec![create_user_message("Hello")];
    let result =
        RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, Some(-0.1));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("temperature"));
}

#[test]
fn test_validate_temperature_too_high() {
    let messages = vec![create_user_message("Hello")];
    let result =
        RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, Some(2.1));
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("temperature"));
}

// ==================== Model Name Validation Tests ====================

#[test]
fn test_validate_model_name_valid() {
    assert!(RequestValidator::validate_model_name("gpt-4").is_ok());
    assert!(RequestValidator::validate_model_name("gpt-4-turbo").is_ok());
    assert!(RequestValidator::validate_model_name("claude-3-opus").is_ok());
    assert!(RequestValidator::validate_model_name("openai/gpt-4").is_ok());
    assert!(RequestValidator::validate_model_name("model.v1.2").is_ok());
    assert!(RequestValidator::validate_model_name("model_name").is_ok());
}

#[test]
fn test_validate_model_name_empty() {
    let messages = vec![create_user_message("Hello")];
    let result = RequestValidator::validate_chat_completion_request("", &messages, None, None);
    assert!(result.is_err());
}

#[test]
fn test_validate_model_name_whitespace() {
    let messages = vec![create_user_message("Hello")];
    let result = RequestValidator::validate_chat_completion_request("   ", &messages, None, None);
    assert!(result.is_err());
}

#[test]
fn test_validate_model_name_invalid_chars() {
    let result = RequestValidator::validate_model_name("model@name");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("invalid characters")
    );
}

// ==================== Function Name Validation Tests ====================

#[test]
fn test_validate_function_name_valid() {
    assert!(RequestValidator::validate_function_name("get_weather").is_ok());
    assert!(RequestValidator::validate_function_name("_private").is_ok());
    assert!(RequestValidator::validate_function_name("function123").is_ok());
    assert!(RequestValidator::validate_function_name("A").is_ok());
}

#[test]
fn test_validate_function_name_empty() {
    let result = RequestValidator::validate_function_name("");
    assert!(result.is_err());
}

#[test]
fn test_validate_function_name_starts_with_number() {
    let result = RequestValidator::validate_function_name("123function");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("identifier"));
}

#[test]
fn test_validate_function_name_special_chars() {
    let result = RequestValidator::validate_function_name("func-name");
    assert!(result.is_err());
}

// ==================== Image URL Validation Tests ====================

#[test]
fn test_validate_image_url_valid_http() {
    let result = RequestValidator::validate_image_url("https://example.com/image.png");
    assert!(result.is_ok());
}

#[test]
fn test_validate_image_url_valid_base64() {
    // Create a valid base64 encoded image (minimal PNG)
    let base64_data = base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
    );
    let data_url = format!("data:image/png;base64,{}", base64_data);
    let result = RequestValidator::validate_image_url(&data_url);
    assert!(result.is_ok());
}

#[test]
fn test_validate_image_url_invalid() {
    let result = RequestValidator::validate_image_url("not-a-url");
    assert!(result.is_err());
}

#[test]
fn test_validate_image_url_invalid_base64() {
    let result = RequestValidator::validate_image_url("data:image/png;base64,invalid!!!");
    assert!(result.is_err());
}

// ==================== Audio Format Validation Tests ====================

#[test]
fn test_validate_audio_format_valid() {
    assert!(RequestValidator::validate_audio_format("mp3").is_ok());
    assert!(RequestValidator::validate_audio_format("wav").is_ok());
    assert!(RequestValidator::validate_audio_format("flac").is_ok());
    assert!(RequestValidator::validate_audio_format("m4a").is_ok());
    assert!(RequestValidator::validate_audio_format("ogg").is_ok());
    assert!(RequestValidator::validate_audio_format("webm").is_ok());
}

#[test]
fn test_validate_audio_format_invalid() {
    let result = RequestValidator::validate_audio_format("mp4");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid audio format")
    );
}

// ==================== Audio Data Validation Tests ====================

#[test]
fn test_validate_audio_data_valid() {
    let valid_base64 =
        base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b"audio data");
    let result = RequestValidator::validate_audio_data(&valid_base64);
    assert!(result.is_ok());
}

#[test]
fn test_validate_audio_data_invalid() {
    let result = RequestValidator::validate_audio_data("not valid base64!!!");
    assert!(result.is_err());
}

// ==================== Message Content Validation Tests ====================

#[test]
fn test_validate_message_content_empty_text() {
    let message = ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text("   ".to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    };
    let result = RequestValidator::validate_chat_message(&message, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

#[test]
fn test_validate_message_content_too_long() {
    let long_text = "a".repeat(1_000_001);
    let message = ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text(long_text)),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    };
    let result = RequestValidator::validate_chat_message(&message, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("too long"));
}

#[test]
fn test_validate_message_content_parts_empty() {
    let message = ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Parts(vec![])),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    };
    let result = RequestValidator::validate_chat_message(&message, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

// ==================== Content Part Validation Tests ====================

#[test]
fn test_validate_content_part_text_valid() {
    let part = ContentPart::Text {
        text: "Hello".to_string(),
    };
    let result = RequestValidator::validate_content_part(&part, 0, 0);
    assert!(result.is_ok());
}

#[test]
fn test_validate_content_part_text_empty() {
    let part = ContentPart::Text {
        text: "   ".to_string(),
    };
    let result = RequestValidator::validate_content_part(&part, 0, 0);
    assert!(result.is_err());
}

#[test]
fn test_validate_content_part_image_valid() {
    let part = ContentPart::ImageUrl {
        image_url: ImageUrl {
            url: "https://example.com/image.png".to_string(),
            detail: Some("auto".to_string()),
        },
    };
    let result = RequestValidator::validate_content_part(&part, 0, 0);
    assert!(result.is_ok());
}

#[test]
fn test_validate_content_part_image_invalid_detail() {
    let part = ContentPart::ImageUrl {
        image_url: ImageUrl {
            url: "https://example.com/image.png".to_string(),
            detail: Some("invalid".to_string()),
        },
    };
    let result = RequestValidator::validate_content_part(&part, 0, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("detail"));
}

#[test]
fn test_validate_content_part_image_valid_details() {
    for detail in ["low", "high", "auto"] {
        let part = ContentPart::ImageUrl {
            image_url: ImageUrl {
                url: "https://example.com/image.png".to_string(),
                detail: Some(detail.to_string()),
            },
        };
        let result = RequestValidator::validate_content_part(&part, 0, 0);
        assert!(result.is_ok(), "Failed for detail: {}", detail);
    }
}

// ==================== Role-Specific Validation Tests ====================

#[test]
fn test_validate_function_message_without_name() {
    let message = ChatMessage {
        role: MessageRole::Function,
        content: Some(MessageContent::Text("result".to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    };
    let result = RequestValidator::validate_chat_message(&message, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("name"));
}

#[test]
fn test_validate_function_message_without_content() {
    let message = ChatMessage {
        role: MessageRole::Function,
        content: None,
        name: Some("get_weather".to_string()),
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    };
    let result = RequestValidator::validate_chat_message(&message, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("content"));
}

#[test]
fn test_validate_tool_message_without_tool_call_id() {
    let message = ChatMessage {
        role: MessageRole::Tool,
        content: Some(MessageContent::Text("result".to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    };
    let result = RequestValidator::validate_chat_message(&message, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("tool_call_id"));
}

#[test]
fn test_validate_tool_message_valid() {
    let message = ChatMessage {
        role: MessageRole::Tool,
        content: Some(MessageContent::Text("result".to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: Some("call_123".to_string()),
        audio: None,
    };
    let result = RequestValidator::validate_chat_message(&message, 0);
    assert!(result.is_ok());
}

#[test]
fn test_validate_user_message_without_content() {
    let message = ChatMessage {
        role: MessageRole::User,
        content: None,
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    };
    let result = RequestValidator::validate_chat_message(&message, 0);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("content"));
}

#[test]
fn test_validate_assistant_message_without_content() {
    let message = ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    };
    let result = RequestValidator::validate_chat_message(&message, 0);
    assert!(result.is_err());
}
