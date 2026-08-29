//! Types integration tests
//!
//! Tests for core types including chat requests, messages, usage, and responses.

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use litellm_rs::core::models::openai::messages::ChatMessageWithExtensions;
    use litellm_rs::core::models::openai::{
        ChatMessage as OpenAiChatMessage, MessageContent as OpenAiMessageContent,
        MessageRole as OpenAiMessageRole,
    };
    use litellm_rs::core::types::anthropic_continuation::{
        AnthropicRedactedData, AnthropicSignature, AnthropicThinkingBlock,
        AnthropicThinkingContent, ChatMessageExtensions,
    };
    use litellm_rs::core::types::chat::{ChatMessage, ChatRequest};
    use litellm_rs::core::types::message::{MessageContent, MessageRole};
    use litellm_rs::core::types::responses::Usage;
    use litellm_rs::core::types::thinking::{
        ThinkingCapabilities, ThinkingConfig, ThinkingContent, ThinkingDelta, ThinkingEffort,
        ThinkingUsage,
    };

    // ==================== MessageRole Tests ====================

    /// Test message role display
    #[test]
    fn test_message_role_display() {
        assert_eq!(format!("{}", MessageRole::System), "system");
        assert_eq!(format!("{}", MessageRole::User), "user");
        assert_eq!(format!("{}", MessageRole::Assistant), "assistant");
        assert_eq!(format!("{}", MessageRole::Tool), "tool");
        assert_eq!(format!("{}", MessageRole::Function), "function");
    }

    /// Test message role serialization
    #[test]
    fn test_message_role_serialization() {
        let role = MessageRole::User;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"user\"");

        let parsed: MessageRole = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, MessageRole::User);
    }

    /// Test message role equality
    #[test]
    fn test_message_role_equality() {
        assert_eq!(MessageRole::System, MessageRole::System);
        assert_ne!(MessageRole::User, MessageRole::Assistant);
    }

    // ==================== MessageContent Tests ====================

    /// Test message content from string
    #[test]
    fn test_message_content_from_string() {
        let content: MessageContent = "Hello, world!".into();
        assert!(matches!(content, MessageContent::Text(ref s) if s == "Hello, world!"));
    }

    /// Test message content display
    #[test]
    fn test_message_content_display() {
        let content = MessageContent::Text("Hello".to_string());
        assert_eq!(format!("{}", content), "Hello");
    }

    // ==================== ChatMessage Tests ====================

    /// Test chat message default
    #[test]
    fn test_chat_message_default() {
        let msg = ChatMessage::default();
        assert_eq!(msg.role, MessageRole::User);
        assert!(msg.content.is_none());
        assert!(msg.thinking.is_none());
        assert!(msg.name.is_none());
        assert!(msg.tool_calls.is_none());
    }

    /// Test chat message has_thinking
    #[test]
    fn test_chat_message_has_thinking() {
        let mut msg = ChatMessage::default();
        assert!(!msg.has_thinking());

        msg.thinking = Some(ThinkingContent::text("reasoning..."));
        assert!(msg.has_thinking());
    }

    /// Test chat message thinking_text
    #[test]
    fn test_chat_message_thinking_text() {
        let mut msg = ChatMessage::default();
        assert!(msg.thinking_text().is_none());

        msg.thinking = Some(ThinkingContent::text("Step 1: Analyze"));
        assert_eq!(msg.thinking_text(), Some("Step 1: Analyze"));
    }

    #[test]
    fn public_chat_message_and_thinking_enum_remain_source_compatible() {
        let message = OpenAiChatMessage {
            role: OpenAiMessageRole::Assistant,
            content: Some(OpenAiMessageContent::Text("done".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        };
        assert!(matches!(
            &message.content,
            Some(OpenAiMessageContent::Text(_))
        ));

        let legacy_json = serde_json::to_value(&message).expect("serialize legacy message");
        let wrapped_json = serde_json::to_value(ChatMessageWithExtensions::new(message))
            .expect("serialize extension wrapper without extensions");
        assert_eq!(wrapped_json, legacy_json);

        fn existing_exhaustive_match(content: ThinkingContent) -> &'static str {
            match content {
                ThinkingContent::Text { .. } => "text",
                ThinkingContent::Block { .. } => "block",
                ThinkingContent::Redacted { .. } => "redacted",
            }
        }

        assert_eq!(
            existing_exhaustive_match(ThinkingContent::redacted(None)),
            "redacted"
        );
    }

    #[test]
    fn typed_anthropic_continuation_has_lossless_cow_and_secret_safe_output() {
        let signature =
            AnthropicSignature::try_from("opaque-signature").expect("non-empty signature is valid");
        let redacted = AnthropicRedactedData::try_from("opaque-redacted-data")
            .expect("non-empty redacted data is valid");
        let single = AnthropicThinkingContent::new(vec![AnthropicThinkingBlock::Thinking {
            thinking: "visible".to_string(),
            signature: signature.clone(),
        }]);
        assert!(matches!(single.as_text(), Some(Cow::Borrowed("visible"))));

        let ordered = AnthropicThinkingContent::new(vec![
            AnthropicThinkingBlock::Thinking {
                thinking: "first ".to_string(),
                signature: signature.clone(),
            },
            AnthropicThinkingBlock::RedactedThinking {
                data: redacted.clone(),
            },
            AnthropicThinkingBlock::Thinking {
                thinking: "second".to_string(),
                signature: signature.clone(),
            },
        ]);
        assert!(matches!(ordered.as_text(), Some(Cow::Owned(ref text)) if text == "first second"));

        let omitted = AnthropicThinkingContent::new(vec![
            AnthropicThinkingBlock::Thinking {
                thinking: String::new(),
                signature: signature.clone(),
            },
            AnthropicThinkingBlock::RedactedThinking {
                data: redacted.clone(),
            },
        ]);
        assert_eq!(omitted.as_text(), None);

        for rendered in [
            format!("{signature:?}"),
            signature.to_string(),
            format!("{redacted:?}"),
            redacted.to_string(),
            format!("{ordered:?}"),
        ] {
            assert!(!rendered.contains("opaque-signature"));
            assert!(!rendered.contains("opaque-redacted-data"));
        }
    }

    #[test]
    fn extended_chat_message_roundtrips_validated_anthropic_blocks() {
        let content = AnthropicThinkingContent::new(vec![
            AnthropicThinkingBlock::Thinking {
                thinking: "plan".to_string(),
                signature: AnthropicSignature::try_from("opaque-signature")
                    .expect("non-empty signature is valid"),
            },
            AnthropicThinkingBlock::RedactedThinking {
                data: AnthropicRedactedData::try_from("opaque-redacted-data")
                    .expect("non-empty redacted data is valid"),
            },
        ]);
        let extensions = ChatMessageExtensions::new().with_anthropic_thinking(content.clone());
        let message = OpenAiChatMessage {
            role: OpenAiMessageRole::Assistant,
            content: None,
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        };
        let extended = ChatMessageWithExtensions::new(message).with_extensions(extensions);

        let encoded = serde_json::to_value(&extended).expect("serialize extension DTO");
        assert_eq!(encoded["role"], "assistant");
        assert_eq!(
            encoded["extensions"]["anthropic_thinking"][0]["type"],
            "thinking"
        );
        let decoded: ChatMessageWithExtensions =
            serde_json::from_value(encoded).expect("deserialize extension DTO");

        assert_eq!(decoded.extensions().anthropic_thinking(), Some(&content));
        assert_eq!(decoded.message().role, OpenAiMessageRole::Assistant);

        let missing_signature = serde_json::from_str::<AnthropicThinkingContent>(
            r#"[{"type":"thinking","thinking":"visible","signature":""}]"#,
        );
        assert!(missing_signature.is_err());
        let missing_redacted_data = serde_json::from_str::<AnthropicThinkingContent>(
            r#"[{"type":"redacted_thinking","data":""}]"#,
        );
        assert!(missing_redacted_data.is_err());

        assert!(
            serde_json::from_str::<AnthropicThinkingContent>(
                r#"[{"type":"thinking","thinking":"visible"}]"#,
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<AnthropicThinkingContent>(r#"[{"type":"redacted_thinking"}]"#,)
                .is_err()
        );
        assert!(AnthropicSignature::try_from("").is_err());
        assert!(AnthropicRedactedData::try_from("").is_err());

        let unknown_block_integrity = serde_json::from_str::<AnthropicThinkingContent>(
            r#"[{"type":"thinking","thinking":"visible","signature":"opaque","future_integrity":"must-not-drop"}]"#,
        );
        assert!(unknown_block_integrity.is_err());

        let unknown_provider_extension =
            serde_json::from_value::<ChatMessageWithExtensions>(serde_json::json!({
                "role": "assistant",
                "content": null,
                "extensions": {
                    "future_provider": {"opaque": "must-not-drop"}
                }
            }));
        assert!(unknown_provider_extension.is_err());

        for thinking in [None, Some(serde_json::json!([]))] {
            let mut extensions = serde_json::json!({
                "anthropic_block_order": [{"type":"text", "start":0, "end":7}]
            });
            if let Some(thinking) = thinking {
                extensions["anthropic_thinking"] = thinking;
            }
            let malformed_order =
                serde_json::from_value::<ChatMessageWithExtensions>(serde_json::json!({
                    "role": "assistant",
                    "content": "visible",
                    "extensions": extensions
                }));
            assert!(
                malformed_order.is_err(),
                "private order metadata must not survive without non-empty thinking"
            );
        }

        let legacy_with_unknown_top_level =
            serde_json::from_value::<OpenAiChatMessage>(serde_json::json!({
                "role": "assistant",
                "content": "done",
                "future_top_level": "legacy-compatible"
            }));
        assert!(legacy_with_unknown_top_level.is_ok());
    }

    // ==================== ChatRequest Tests ====================

    /// Test chat request creation
    #[test]
    fn test_chat_request_new() {
        let request = ChatRequest::new("gpt-4");
        assert_eq!(request.model, "gpt-4");
        assert!(request.messages.is_empty());
        assert!(!request.stream);
    }

    /// Test chat request builder pattern
    #[test]
    fn test_chat_request_builder() {
        let request = ChatRequest::new("gpt-4")
            .add_system_message("You are a helpful assistant")
            .add_user_message("Hello!")
            .with_temperature(0.7)
            .with_max_tokens(100);

        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.max_tokens, Some(100));
    }

    /// Test chat request add messages
    #[test]
    fn test_chat_request_add_messages() {
        let request = ChatRequest::new("gpt-4")
            .add_system_message("System prompt")
            .add_user_message("User input")
            .add_assistant_message("Assistant response");

        assert_eq!(request.messages.len(), 3);
        assert_eq!(request.messages[0].role, MessageRole::System);
        assert_eq!(request.messages[1].role, MessageRole::User);
        assert_eq!(request.messages[2].role, MessageRole::Assistant);
    }

    /// Test chat request streaming
    #[test]
    fn test_chat_request_streaming() {
        let request = ChatRequest::new("gpt-4").with_streaming();
        assert!(request.stream);
    }

    /// Test chat request with thinking
    #[test]
    fn test_chat_request_with_thinking() {
        let request = ChatRequest::new("o1-preview")
            .add_user_message("Solve this step by step")
            .with_thinking(ThinkingConfig::high_effort());

        assert!(request.thinking.is_some());
        let thinking = request.thinking.unwrap();
        assert!(thinking.enabled);
        assert_eq!(thinking.effort, Some(ThinkingEffort::High));
    }

    /// Test chat request enable_thinking
    #[test]
    fn test_chat_request_enable_thinking() {
        let request = ChatRequest::new("deepseek-r1").enable_thinking();

        assert!(request.thinking.is_some());
        let thinking = request.thinking.unwrap();
        assert!(thinking.enabled);
        assert_eq!(thinking.effort, Some(ThinkingEffort::Medium));
    }

    /// Test chat request estimate_input_tokens
    #[test]
    fn test_chat_request_estimate_tokens() {
        let request = ChatRequest::new("gpt-4").add_user_message("Hello, how are you?");

        let tokens = request.estimate_input_tokens();
        assert!(tokens > 0);
    }

    /// Test chat request serialization
    #[test]
    fn test_chat_request_serialization() {
        let request = ChatRequest::new("gpt-4")
            .add_user_message("Hello")
            .with_temperature(0.5);

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"gpt-4\""));
        assert!(json.contains("\"temperature\":0.5"));

        let parsed: ChatRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.model, "gpt-4");
        assert_eq!(parsed.temperature, Some(0.5));
    }

    // ==================== Usage Tests ====================

    /// Test usage creation
    #[test]
    fn test_usage_new() {
        let usage = Usage::new(100, 50);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    /// Test usage default
    #[test]
    fn test_usage_default() {
        let usage = Usage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
        assert!(usage.thinking_usage.is_none());
    }

    /// Test usage with thinking
    #[test]
    fn test_usage_with_thinking() {
        let thinking_usage = ThinkingUsage::new(500)
            .with_budget(1000)
            .with_provider("openai");

        let usage = Usage::new(100, 50).with_thinking(thinking_usage);

        assert!(usage.thinking_usage.is_some());
        assert_eq!(usage.thinking_tokens(), Some(500));
    }

    /// Test usage thinking_tokens convenience method
    #[test]
    fn test_usage_thinking_tokens() {
        // No thinking usage
        let usage = Usage::new(100, 50);
        assert!(usage.thinking_tokens().is_none());

        // With thinking usage
        let usage = Usage::new(100, 50).with_thinking(ThinkingUsage::new(200));
        assert_eq!(usage.thinking_tokens(), Some(200));
    }

    // ==================== ThinkingConfig Tests ====================

    /// Test thinking config presets
    #[test]
    fn test_thinking_config_presets() {
        let low = ThinkingConfig::low_effort();
        assert!(low.enabled);
        assert_eq!(low.effort, Some(ThinkingEffort::Low));

        let medium = ThinkingConfig::medium_effort();
        assert!(medium.enabled);
        assert_eq!(medium.effort, Some(ThinkingEffort::Medium));

        let high = ThinkingConfig::high_effort();
        assert!(high.enabled);
        assert_eq!(high.effort, Some(ThinkingEffort::High));
    }

    /// Test thinking config builder
    #[test]
    fn test_thinking_config_builder() {
        let config = ThinkingConfig::new()
            .enabled()
            .with_budget(10000)
            .with_effort(ThinkingEffort::High)
            .include_in_response(false);

        assert!(config.enabled);
        assert_eq!(config.budget_tokens, Some(10000));
        assert_eq!(config.effort, Some(ThinkingEffort::High));
        assert!(!config.include_thinking);
    }

    // ==================== ThinkingEffort Tests ====================

    /// Test thinking effort as_str
    #[test]
    fn test_thinking_effort_as_str() {
        assert_eq!(ThinkingEffort::Low.as_str(), "low");
        assert_eq!(ThinkingEffort::Medium.as_str(), "medium");
        assert_eq!(ThinkingEffort::High.as_str(), "high");
    }

    /// Test thinking effort suggested_budget
    #[test]
    fn test_thinking_effort_suggested_budget() {
        assert!(ThinkingEffort::Low.suggested_budget() < ThinkingEffort::Medium.suggested_budget());
        assert!(
            ThinkingEffort::Medium.suggested_budget() < ThinkingEffort::High.suggested_budget()
        );
    }

    /// Test thinking effort display
    #[test]
    fn test_thinking_effort_display() {
        assert_eq!(format!("{}", ThinkingEffort::Low), "low");
        assert_eq!(format!("{}", ThinkingEffort::Medium), "medium");
        assert_eq!(format!("{}", ThinkingEffort::High), "high");
    }

    // ==================== ThinkingContent Tests ====================

    /// Test thinking content text
    #[test]
    fn test_thinking_content_text() {
        let content = ThinkingContent::text("Let me think...");
        assert_eq!(content.as_text(), Some("Let me think..."));
        assert!(!content.is_redacted());
    }

    /// Test thinking content block
    #[test]
    fn test_thinking_content_block() {
        let content = ThinkingContent::block("Analyzing the problem");
        assert_eq!(content.as_text(), Some("Analyzing the problem"));
    }

    /// Test thinking content redacted
    #[test]
    fn test_thinking_content_redacted() {
        let content = ThinkingContent::redacted(Some(100));
        assert!(content.is_redacted());
        assert!(content.as_text().is_none());
    }

    /// Test thinking content with signature
    #[test]
    fn test_thinking_content_with_signature() {
        let content = ThinkingContent::text_with_signature("thinking", "sig123");
        match content {
            ThinkingContent::Text { text, signature } => {
                assert_eq!(text, "thinking");
                assert_eq!(signature, Some("sig123".to_string()));
            }
            _ => panic!("Expected Text variant"),
        }
    }

    // ==================== ThinkingUsage Tests ====================

    /// Test thinking usage creation
    #[test]
    fn test_thinking_usage_new() {
        let usage = ThinkingUsage::new(1000);
        assert_eq!(usage.thinking_tokens, Some(1000));
        assert!(usage.budget_tokens.is_none());
    }

    /// Test thinking usage builder
    #[test]
    fn test_thinking_usage_builder() {
        let usage = ThinkingUsage::new(1000)
            .with_budget(2000)
            .with_cost(0.01)
            .with_provider("anthropic");

        assert_eq!(usage.thinking_tokens, Some(1000));
        assert_eq!(usage.budget_tokens, Some(2000));
        assert_eq!(usage.thinking_cost, Some(0.01));
        assert_eq!(usage.provider, Some("anthropic".to_string()));
    }

    // ==================== ThinkingCapabilities Tests ====================

    /// Test thinking capabilities supported
    #[test]
    fn test_thinking_capabilities_supported() {
        let caps = ThinkingCapabilities::supported()
            .with_max_tokens(20000)
            .with_streaming()
            .with_models(vec!["o1-preview".to_string()]);

        assert!(caps.supports_thinking);
        assert!(caps.supports_streaming_thinking);
        assert_eq!(caps.max_thinking_tokens, Some(20000));
        assert!(!caps.thinking_models.is_empty());
    }

    /// Test thinking capabilities unsupported
    #[test]
    fn test_thinking_capabilities_unsupported() {
        let caps = ThinkingCapabilities::unsupported();
        assert!(!caps.supports_thinking);
        assert!(!caps.supports_streaming_thinking);
    }

    // ==================== ThinkingDelta Tests ====================

    /// Test thinking delta new
    #[test]
    fn test_thinking_delta_new() {
        let delta = ThinkingDelta::new("partial thinking");
        assert_eq!(delta.content, Some("partial thinking".to_string()));
        assert!(delta.is_start.is_none());
        assert!(delta.is_complete.is_none());
    }

    /// Test thinking delta start
    #[test]
    fn test_thinking_delta_start() {
        let delta = ThinkingDelta::start();
        assert_eq!(delta.is_start, Some(true));
        assert!(delta.content.is_none());
    }

    /// Test thinking delta complete
    #[test]
    fn test_thinking_delta_complete() {
        let delta = ThinkingDelta::complete();
        assert_eq!(delta.is_complete, Some(true));
        assert!(delta.content.is_none());
    }

    // ==================== Complex Serialization Tests ====================

    /// Test full chat request serialization roundtrip
    #[test]
    fn test_chat_request_full_roundtrip() {
        let request = ChatRequest::new("gpt-4-turbo")
            .add_system_message("You are helpful")
            .add_user_message("Explain quantum computing")
            .with_temperature(0.8)
            .with_max_tokens(2048)
            .with_streaming()
            .with_thinking(ThinkingConfig::medium_effort());

        let json = serde_json::to_string(&request).unwrap();
        let parsed: ChatRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.model, "gpt-4-turbo");
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(parsed.temperature, Some(0.8));
        assert_eq!(parsed.max_tokens, Some(2048));
        assert!(parsed.stream);
        assert!(parsed.thinking.is_some());
    }

    /// Test usage serialization roundtrip
    #[test]
    fn test_usage_serialization_roundtrip() {
        let usage = Usage::new(100, 200).with_thinking(ThinkingUsage::new(50));

        let json = serde_json::to_string(&usage).unwrap();
        let parsed: Usage = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.prompt_tokens, 100);
        assert_eq!(parsed.completion_tokens, 200);
        assert_eq!(parsed.total_tokens, 300);
        assert!(parsed.thinking_usage.is_some());
    }
}
