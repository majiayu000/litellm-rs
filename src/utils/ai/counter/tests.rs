//! Tests for token counter functionality

#[cfg(test)]
use crate::core::models::openai::{
    ChatMessage, ContentPart, FunctionCall, ImageUrl, MessageContent, MessageRole, ToolCall,
};
use crate::utils::ai::counter::token_counter::TokenCounter;

#[test]
fn test_text_token_estimation() {
    let counter = TokenCounter::new();
    let config = counter.get_model_config("gpt-3.5-turbo").unwrap();

    let tokens = counter.estimate_text_tokens(config, "Hello, world!");
    assert!(tokens > 0);
    assert!(tokens < 10); // Should be reasonable for short text
}

#[test]
fn test_chat_token_counting() {
    let counter = TokenCounter::new();
    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text("Hello, how are you?".to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }];

    let estimate = counter
        .count_chat_tokens("gpt-3.5-turbo", &messages)
        .unwrap();
    assert!(estimate.input_tokens > 0);
    assert!(!estimate.is_approximate);
    assert_eq!(estimate.input_tokens, 13);
}

#[test]
fn test_openai_completion_token_count_uses_tiktoken() {
    let counter = TokenCounter::new();

    let estimate = counter
        .count_completion_tokens("gpt-3.5-turbo", "Hello world")
        .unwrap();

    assert_eq!(estimate.input_tokens, 2);
    assert_eq!(estimate.total_tokens, 2);
    assert!(!estimate.is_approximate);
    assert_eq!(estimate.confidence, 1.0);
}

#[test]
fn test_openai_chat_system_message_uses_tiktoken() {
    let counter = TokenCounter::new();
    let messages = vec![ChatMessage {
        role: MessageRole::System,
        content: Some(MessageContent::Text("You are a bot.".to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }];

    let estimate = counter
        .count_chat_tokens("openai/gpt-3.5-turbo", &messages)
        .unwrap();

    assert_eq!(estimate.input_tokens, 12);
    assert!(!estimate.is_approximate);
}

#[test]
fn test_multimodal_chat_token_count_remains_marked_approximate() {
    let counter = TokenCounter::new();
    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Parts(vec![ContentPart::ImageUrl {
            image_url: ImageUrl {
                url: "https://example.com/image.png".to_string(),
                detail: Some("high".to_string()),
            },
        }])),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }];

    let estimate = counter.count_chat_tokens("gpt-4o", &messages).unwrap();

    assert!(estimate.is_approximate);
    assert!(estimate.input_tokens >= 85);
}

#[test]
fn test_text_part_chat_token_count_remains_marked_approximate() {
    let counter = TokenCounter::new();
    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Parts(vec![ContentPart::Text {
            text: "Hello".to_string(),
        }])),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }];

    let estimate = counter.count_chat_tokens("gpt-4o", &messages).unwrap();

    assert!(estimate.is_approximate);
    assert!(estimate.confidence < 1.0);
}

#[test]
fn test_non_openai_token_count_remains_marked_approximate() {
    let counter = TokenCounter::new();

    let estimate = counter
        .count_completion_tokens("claude-3-opus", "Hello world")
        .unwrap();

    assert!(estimate.is_approximate);
    assert!(estimate.input_tokens > 0);
}

#[test]
fn test_unknown_openai_like_model_remains_marked_approximate() {
    let counter = TokenCounter::new();

    let estimate = counter
        .count_completion_tokens("gpt-future-unknown", "Hello world")
        .unwrap();

    assert!(estimate.is_approximate);
    assert!(estimate.confidence < 1.0);
}

#[test]
fn test_explicit_unknown_openai_model_returns_tokenizer_error() {
    let counter = TokenCounter::new();

    let error = counter
        .count_completion_tokens("openai/gpt-future-unknown", "Hello world")
        .expect_err("an explicit unknown OpenAI tokenizer must not be approximated");

    let message = error.to_string();
    assert!(message.contains("unknown OpenAI catalog model"));
    assert!(message.contains("openai/gpt-future-unknown"));
}

#[test]
fn test_fake_gpt5_prefix_is_rejected_as_unknown_catalog_model() {
    let counter = TokenCounter::new();

    let error = counter
        .count_completion_tokens("openai/gpt-5-definitely-fake", "Hello world")
        .expect_err("a tokenizer family prefix must not establish catalog identity");

    let message = error.to_string();
    assert!(
        message.contains("unknown OpenAI catalog model"),
        "{message}"
    );
    assert!(
        message.contains("openai/gpt-5-definitely-fake"),
        "{message}"
    );
}

#[test]
fn test_catalog_audio_models_report_tokenizer_unavailable() {
    let counter = TokenCounter::new();

    for model in ["openai/gpt-audio-1.5", "openai/gpt-realtime-1.5"] {
        let error = counter
            .count_completion_tokens(model, "Hello world")
            .expect_err("catalog audio/realtime models without a BPE must fail clearly");
        let message = error.to_string();
        assert!(
            message.contains("tokenizer unavailable"),
            "{model}: {message}"
        );
        assert!(message.contains(model), "{model}: {message}");
    }
}

#[test]
fn test_explicit_unknown_openai_chat_model_returns_tokenizer_error() {
    let counter = TokenCounter::new();
    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text("hello".to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }];

    let error = counter
        .count_chat_tokens("OPENAI/gpt-future-unknown", &messages)
        .expect_err("explicit OpenAI chat tokenization must resolve exactly");

    assert!(error.to_string().contains("OPENAI/gpt-future-unknown"));
}

#[test]
fn test_provider_native_and_custom_deployments_remain_approximate() {
    let counter = TokenCounter::new();

    for model in [
        "azure/custom-deployment",
        "azure_ai/Phi-4",
        "anthropic/claude-private-deployment",
        "unknown-provider/BAAI/bge-m3",
        "openai_like/gpt-future-unknown",
    ] {
        let estimate = counter
            .count_completion_tokens(model, "Hello world")
            .unwrap_or_else(|error| panic!("{model} should permit approximation: {error}"));

        assert!(estimate.is_approximate, "{model}");
        assert!(estimate.confidence < 1.0, "{model}");
    }
}

#[test]
fn test_explicit_unknown_openai_output_estimation_returns_error() {
    let counter = TokenCounter::new();

    let error = counter
        .estimate_output_tokens(None, 10, "openai/gpt-future-unknown")
        .expect_err("output reservation must not hide an explicit tokenizer error");

    assert!(error.to_string().contains("unknown OpenAI catalog model"));
}

#[test]
fn test_tool_call_chat_token_count_remains_marked_approximate() {
    let counter = TokenCounter::new();
    let messages = vec![ChatMessage {
        role: MessageRole::Assistant,
        content: None,
        name: None,
        function_call: None,
        tool_calls: Some(vec![ToolCall {
            id: "call_123".to_string(),
            tool_type: "function".to_string(),
            function: FunctionCall {
                name: "lookup".to_string(),
                arguments: r#"{"query":"hello"}"#.to_string(),
            },
        }]),
        tool_call_id: None,
        audio: None,
    }];

    let estimate = counter.count_chat_tokens("gpt-4o", &messages).unwrap();

    assert!(estimate.is_approximate);
    assert!(estimate.confidence < 1.0);
}

#[test]
fn test_tool_result_chat_token_count_remains_marked_approximate() {
    let counter = TokenCounter::new();
    let messages = vec![ChatMessage {
        role: MessageRole::Tool,
        content: Some(MessageContent::Text("done".to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: Some("call_123".to_string()),
        audio: None,
    }];

    let estimate = counter.count_chat_tokens("gpt-4o", &messages).unwrap();

    assert!(estimate.is_approximate);
    assert!(estimate.confidence < 1.0);
}

#[test]
fn test_context_window_check() {
    let counter = TokenCounter::new();

    // Should fit
    assert!(
        counter
            .check_context_window("gpt-3.5-turbo", 1000, Some(1000))
            .unwrap()
    );

    // Should not fit
    assert!(
        !counter
            .check_context_window("gpt-3.5-turbo", 3000, Some(2000))
            .unwrap()
    );
}

#[test]
fn test_model_family_extraction() {
    let counter = TokenCounter::new();

    assert_eq!(counter.extract_model_family("gpt-4-turbo"), "gpt-4");
    assert_eq!(
        counter.extract_model_family("gpt-3.5-turbo-16k"),
        "gpt-3.5-turbo"
    );
    assert_eq!(counter.extract_model_family("claude-3-opus"), "claude-3");
    assert_eq!(counter.extract_model_family("unknown-model"), "default");
}
