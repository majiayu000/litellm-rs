//! Utility functions for semantic caching

use crate::core::models::openai::{ChatMessage, ContentPart, MessageContent};

/// Extract prompt text from messages
pub fn extract_prompt_text(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .filter_map(|msg| match &msg.content {
            Some(MessageContent::Text(text)) => Some(text.clone()),
            Some(MessageContent::Parts(parts)) => {
                let text = parts
                    .iter()
                    .filter_map(|part| match part {
                        ContentPart::Text { text } => Some(text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<String>>()
                    .join(" ");
                if text.is_empty() { None } else { Some(text) }
            }
            None => None,
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// Hash a prompt for quick lookup
pub fn hash_prompt(prompt: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(prompt.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::openai::{ImageUrl, MessageRole};

    // ==================== hash_prompt Tests ====================

    #[test]
    fn test_hash_prompt_simple() {
        let hash = hash_prompt("Hello, world!");
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA256 produces 64 hex chars
    }

    #[test]
    fn test_hash_prompt_deterministic() {
        let hash1 = hash_prompt("test prompt");
        let hash2 = hash_prompt("test prompt");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_prompt_different_inputs() {
        let hash1 = hash_prompt("prompt 1");
        let hash2 = hash_prompt("prompt 2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_prompt_empty() {
        let hash = hash_prompt("");
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_prompt_unicode() {
        let hash = hash_prompt("你好世界");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_prompt_long_text() {
        let long_text = "a".repeat(10000);
        let hash = hash_prompt(&long_text);
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_hash_prompt_hex_format() {
        let hash = hash_prompt("test");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_prompt_case_sensitive() {
        let hash1 = hash_prompt("Hello");
        let hash2 = hash_prompt("hello");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_prompt_whitespace_sensitive() {
        let hash1 = hash_prompt("hello world");
        let hash2 = hash_prompt("hello  world");
        assert_ne!(hash1, hash2);
    }

    // ==================== extract_prompt_text Tests ====================

    #[test]
    fn test_extract_prompt_text_empty() {
        let messages: Vec<ChatMessage> = vec![];
        let result = extract_prompt_text(&messages);
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_prompt_text_single_text_message() {
        let messages = vec![ChatMessage {
            thinking: None,
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }];

        let result = extract_prompt_text(&messages);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_extract_prompt_text_multiple_messages() {
        let messages = vec![
            ChatMessage {
                thinking: None,
                role: MessageRole::System,
                content: Some(MessageContent::Text("You are a helper.".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            },
            ChatMessage {
                thinking: None,
                role: MessageRole::User,
                content: Some(MessageContent::Text("Help me.".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            },
        ];

        let result = extract_prompt_text(&messages);
        assert_eq!(result, "You are a helper.\nHelp me.");
    }

    #[test]
    fn test_extract_prompt_text_with_none_content() {
        let messages = vec![
            ChatMessage {
                thinking: None,
                role: MessageRole::User,
                content: Some(MessageContent::Text("Hello".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            },
            ChatMessage {
                thinking: None,
                role: MessageRole::Assistant,
                content: None,
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            },
        ];

        let result = extract_prompt_text(&messages);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn test_extract_prompt_text_with_parts() {
        let messages = vec![ChatMessage {
            thinking: None,
            role: MessageRole::User,
            content: Some(MessageContent::Parts(vec![
                ContentPart::Text {
                    text: "Part 1".to_string(),
                },
                ContentPart::Text {
                    text: "Part 2".to_string(),
                },
            ])),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }];

        let result = extract_prompt_text(&messages);
        assert_eq!(result, "Part 1 Part 2");
    }

    #[test]
    fn test_extract_prompt_text_with_image_parts() {
        let messages = vec![ChatMessage {
            thinking: None,
            role: MessageRole::User,
            content: Some(MessageContent::Parts(vec![
                ContentPart::Text {
                    text: "Describe this image:".to_string(),
                },
                ContentPart::ImageUrl {
                    image_url: ImageUrl {
                        url: "https://example.com/image.png".to_string(),
                        detail: None,
                    },
                },
            ])),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }];

        let result = extract_prompt_text(&messages);
        // Image parts should be filtered out
        assert_eq!(result, "Describe this image:");
    }

    #[test]
    fn test_extract_prompt_text_empty_parts() {
        let messages = vec![ChatMessage {
            thinking: None,
            role: MessageRole::User,
            content: Some(MessageContent::Parts(vec![])),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }];

        let result = extract_prompt_text(&messages);
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_prompt_text_only_image_parts() {
        let messages = vec![ChatMessage {
            thinking: None,
            role: MessageRole::User,
            content: Some(MessageContent::Parts(vec![ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: "https://example.com/image.png".to_string(),
                    detail: None,
                },
            }])),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }];

        let result = extract_prompt_text(&messages);
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_prompt_text_mixed_content() {
        let messages = vec![
            ChatMessage {
                thinking: None,
                role: MessageRole::System,
                content: Some(MessageContent::Text("System prompt".to_string())),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            },
            ChatMessage {
                thinking: None,
                role: MessageRole::User,
                content: Some(MessageContent::Parts(vec![ContentPart::Text {
                    text: "User text".to_string(),
                }])),
                name: None,
                function_call: None,
                tool_calls: None,
                tool_call_id: None,
                audio: None,
            },
        ];

        let result = extract_prompt_text(&messages);
        assert_eq!(result, "System prompt\nUser text");
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_extract_and_hash() {
        let messages = vec![ChatMessage {
            thinking: None,
            role: MessageRole::User,
            content: Some(MessageContent::Text("What is 2+2?".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }];

        let text = extract_prompt_text(&messages);
        let hash = hash_prompt(&text);

        assert_eq!(text, "What is 2+2?");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_same_messages_same_hash() {
        let messages1 = vec![ChatMessage {
            thinking: None,
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }];

        let messages2 = vec![ChatMessage {
            thinking: None,
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }];

        let hash1 = hash_prompt(&extract_prompt_text(&messages1));
        let hash2 = hash_prompt(&extract_prompt_text(&messages2));

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_different_messages_different_hash() {
        let messages1 = vec![ChatMessage {
            thinking: None,
            role: MessageRole::User,
            content: Some(MessageContent::Text("Hello".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }];

        let messages2 = vec![ChatMessage {
            thinking: None,
            role: MessageRole::User,
            content: Some(MessageContent::Text("Goodbye".to_string())),
            name: None,
            function_call: None,
            tool_calls: None,
            tool_call_id: None,
            audio: None,
        }];

        let hash1 = hash_prompt(&extract_prompt_text(&messages1));
        let hash2 = hash_prompt(&extract_prompt_text(&messages2));

        assert_ne!(hash1, hash2);
    }
}
