//! Tests for token counter functionality

#[cfg(test)]
use crate::core::models::openai::{
    ChatMessage, ContentPart, FunctionCall, ImageUrl, MessageContent, MessageRole, ToolCall,
};
use crate::utils::ai::counter::token_counter::{TokenCounter, TokenizerIdentity};

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
        .count_chat_tokens(&TokenizerIdentity::exact_openai("gpt-3.5-turbo"), &messages)
        .unwrap();
    assert!(estimate.input_tokens > 0);
    assert!(!estimate.is_approximate);
    assert_eq!(estimate.input_tokens, 13);
}

#[test]
fn test_openai_completion_token_count_uses_tiktoken() {
    let counter = TokenCounter::new();

    let estimate = counter
        .count_completion_tokens(
            &TokenizerIdentity::exact_openai("gpt-3.5-turbo"),
            "Hello world",
        )
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
        .count_chat_tokens(&TokenizerIdentity::exact_openai("gpt-3.5-turbo"), &messages)
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

    let estimate = counter
        .count_chat_tokens(
            &TokenizerIdentity::approximate("openai", "gpt-4o"),
            &messages,
        )
        .unwrap();

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

    let estimate = counter
        .count_chat_tokens(
            &TokenizerIdentity::approximate("openai", "gpt-4o"),
            &messages,
        )
        .unwrap();

    assert!(estimate.is_approximate);
    assert!(estimate.confidence < 1.0);
}

#[test]
fn test_non_openai_token_count_remains_marked_approximate() {
    let counter = TokenCounter::new();

    let estimate = counter
        .count_completion_tokens(
            &TokenizerIdentity::approximate("anthropic", "claude-3-opus"),
            "Hello world",
        )
        .unwrap();

    assert!(estimate.is_approximate);
    assert!(estimate.input_tokens > 0);
}

#[test]
fn public_counter_requires_explicit_approximation_for_non_openai_providers() {
    let counter = TokenCounter::new();

    for provider in ["anthropic", "unknown"] {
        let identity = TokenizerIdentity::approximate(provider, "gpt-4");
        let estimate = counter
            .count_completion_tokens(&identity, "Hello world")
            .expect("an explicit approximate identity may use approximation");

        assert!(
            estimate.is_approximate,
            "{provider}/gpt-4 must not select OpenAI BPE"
        );
    }
}

#[test]
fn approximate_provider_identity_does_not_select_a_prefix_family_config() {
    let counter = TokenCounter::new();

    for provider in ["anthropic", "unknown"] {
        let identity = TokenizerIdentity::approximate(provider, "gpt-4");
        let fits = counter
            .check_context_window(&identity, 5_000, None)
            .expect("an approximate contract should have a default config");

        assert!(!fits, "{provider}/gpt-4 must use the default 4k window");
    }
}

#[test]
fn public_exact_openai_failure_is_not_converted_to_approximation() {
    let counter = TokenCounter::new();

    let error = counter
        .count_completion_tokens(
            &TokenizerIdentity::exact_openai("gpt-audio-1.5"),
            "Hello world",
        )
        .expect_err("an explicit OpenAI model without a tokenizer must fail closed");

    assert!(error.to_string().contains("tokenizer"));
    assert!(error.to_string().contains("gpt-audio-1.5"));
}

#[test]
fn exact_openai_identity_rejects_provider_qualified_model_names() {
    let counter = TokenCounter::new();

    let error = counter
        .count_completion_tokens(
            &TokenizerIdentity::exact_openai("openai/gpt-4"),
            "Hello world",
        )
        .expect_err("an exact identity must contain a canonical tokenizer model");

    assert!(error.to_string().contains("tokenizer unavailable"));
    assert!(error.to_string().contains("openai/gpt-4"));
}

#[test]
fn test_unknown_openai_like_model_remains_marked_approximate() {
    let counter = TokenCounter::new();

    let estimate = counter
        .count_completion_tokens(
            &TokenizerIdentity::approximate("openai", "gpt-future-unknown"),
            "Hello world",
        )
        .unwrap();

    assert!(estimate.is_approximate);
    assert!(estimate.confidence < 1.0);
}

#[test]
fn test_typed_openai_identity_requires_an_exact_tokenizer() {
    let counter = TokenCounter::new();
    let identity = TokenizerIdentity::exact_openai("gpt-audio-1.5");

    let error = counter
        .count_completion_tokens(&identity, "Hello world")
        .expect_err("a selected exact OpenAI identity without a BPE must fail closed");

    assert!(error.to_string().contains("tokenizer unavailable"));
    assert!(error.to_string().contains("openai/gpt-audio-1.5"));
}

#[test]
fn test_typed_openai_chat_identity_requires_an_exact_tokenizer() {
    let counter = TokenCounter::new();
    let identity = TokenizerIdentity::exact_openai("gpt-audio-1.5");
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
        .count_chat_tokens(&identity, &messages)
        .expect_err("exact chat tokenization must not silently approximate a missing BPE");

    assert!(error.to_string().contains("tokenizer unavailable"));
}

#[test]
fn test_typed_approximate_identity_stays_explicitly_approximate() {
    let counter = TokenCounter::new();
    let identity = TokenizerIdentity::approximate("azure_ai", "Phi-4");

    let estimate = counter
        .count_completion_tokens(&identity, "Hello world")
        .expect("an explicitly approximate identity may use estimation");

    assert!(estimate.is_approximate);
    assert!(estimate.confidence < 1.0);
}

#[test]
fn test_typed_approximate_identity_does_not_infer_from_openai_like_model_name() {
    let counter = TokenCounter::new();
    let identity = TokenizerIdentity::approximate("azure", "gpt-4o");

    let estimate = counter
        .count_completion_tokens(&identity, "Hello world")
        .expect("explicit approximate contract should remain available");

    assert!(estimate.is_approximate);
    assert!(estimate.confidence < 1.0);
}

#[test]
fn test_typed_approximate_chat_does_not_infer_from_openai_like_model_name() {
    let counter = TokenCounter::new();
    let identity = TokenizerIdentity::approximate("azure", "gpt-4o");
    let messages = vec![ChatMessage {
        role: MessageRole::User,
        content: Some(MessageContent::Text("hello".to_string())),
        name: None,
        function_call: None,
        tool_calls: None,
        tool_call_id: None,
        audio: None,
    }];

    let estimate = counter
        .count_chat_tokens(&identity, &messages)
        .expect("explicit approximate chat contract should remain available");

    assert!(estimate.is_approximate);
    assert!(estimate.confidence < 1.0);
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

    let estimate = counter
        .count_chat_tokens(
            &TokenizerIdentity::approximate("openai", "gpt-4o"),
            &messages,
        )
        .unwrap();

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

    let estimate = counter
        .count_chat_tokens(
            &TokenizerIdentity::approximate("openai", "gpt-4o"),
            &messages,
        )
        .unwrap();

    assert!(estimate.is_approximate);
    assert!(estimate.confidence < 1.0);
}

#[test]
fn test_context_window_check() {
    let counter = TokenCounter::new();

    // Should fit
    assert!(
        counter
            .check_context_window(
                &TokenizerIdentity::exact_openai("gpt-3.5-turbo"),
                1000,
                Some(1000),
            )
            .unwrap()
    );

    // Should not fit
    assert!(
        !counter
            .check_context_window(
                &TokenizerIdentity::exact_openai("gpt-3.5-turbo"),
                3000,
                Some(2000),
            )
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
