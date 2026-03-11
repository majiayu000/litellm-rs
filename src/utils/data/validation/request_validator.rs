//! Request validation utilities

use crate::core::models::openai::MessageContent;
use crate::utils::error::gateway_error::{GatewayError, Result};
use regex::Regex;

/// Request validation utilities
pub struct RequestValidator;

impl RequestValidator {
    /// Validate chat completion request
    pub fn validate_chat_completion_request(
        model: &str,
        messages: &[crate::core::models::openai::ChatMessage],
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> Result<()> {
        // Validate model
        Self::validate_model_name(model)?;

        // Validate messages
        if messages.is_empty() {
            return Err(GatewayError::Validation(
                "Messages cannot be empty".to_string(),
            ));
        }

        for (i, message) in messages.iter().enumerate() {
            Self::validate_chat_message(message, i)?;
        }

        // Validate max_tokens
        if let Some(max_tokens) = max_tokens {
            if max_tokens == 0 {
                return Err(GatewayError::Validation(
                    "max_tokens must be greater than 0".to_string(),
                ));
            }
            if max_tokens > 100000 {
                return Err(GatewayError::Validation(
                    "max_tokens cannot exceed 100000".to_string(),
                ));
            }
        }

        // Validate temperature
        if let Some(temperature) = temperature
            && !(0.0..=2.0).contains(&temperature)
        {
            return Err(GatewayError::Validation(
                "temperature must be between 0.0 and 2.0".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate chat message
    fn validate_chat_message(
        message: &crate::core::models::openai::ChatMessage,
        index: usize,
    ) -> Result<()> {
        use crate::core::models::openai::MessageRole;

        // Validate role
        match message.role {
            MessageRole::System | MessageRole::User | MessageRole::Assistant => {
                // These roles should have content
                if message.content.is_none() {
                    return Err(GatewayError::Validation(format!(
                        "Message at index {} with role {:?} must have content",
                        index, message.role
                    )));
                }
            }
            MessageRole::Function => {
                // Function messages should have name and content
                if message.name.is_none() {
                    return Err(GatewayError::Validation(format!(
                        "Function message at index {} must have a name",
                        index
                    )));
                }
                if message.content.is_none() {
                    return Err(GatewayError::Validation(format!(
                        "Function message at index {} must have content",
                        index
                    )));
                }
            }
            MessageRole::Tool => {
                // Tool messages should have tool_call_id and content
                if message.tool_call_id.is_none() {
                    return Err(GatewayError::Validation(format!(
                        "Tool message at index {} must have tool_call_id",
                        index
                    )));
                }
                if message.content.is_none() {
                    return Err(GatewayError::Validation(format!(
                        "Tool message at index {} must have content",
                        index
                    )));
                }
            }
        }

        // Validate content if present
        if let Some(content) = &message.content {
            Self::validate_message_content(content, index)?;
        }

        // Validate name if present
        if let Some(name) = &message.name {
            Self::validate_function_name(name)?;
        }

        Ok(())
    }

    /// Validate message content
    fn validate_message_content(
        content: &crate::core::models::openai::MessageContent,
        index: usize,
    ) -> Result<()> {
        match content {
            MessageContent::Text(text) => {
                if text.trim().is_empty() {
                    return Err(GatewayError::Validation(format!(
                        "Text content at message index {} cannot be empty",
                        index
                    )));
                }
                if text.len() > 1_000_000 {
                    return Err(GatewayError::Validation(format!(
                        "Text content at message index {} is too long (max 1M characters)",
                        index
                    )));
                }
            }
            MessageContent::Parts(parts) => {
                if parts.is_empty() {
                    return Err(GatewayError::Validation(format!(
                        "Content parts at message index {} cannot be empty",
                        index
                    )));
                }
                for (part_index, part) in parts.iter().enumerate() {
                    Self::validate_content_part(part, index, part_index)?;
                }
            }
        }

        Ok(())
    }

    /// Validate content part
    fn validate_content_part(
        part: &crate::core::models::openai::ContentPart,
        message_index: usize,
        part_index: usize,
    ) -> Result<()> {
        use crate::core::models::openai::ContentPart;

        match part {
            ContentPart::Text { text } => {
                if text.trim().is_empty() {
                    return Err(GatewayError::Validation(format!(
                        "Text part at message {} part {} cannot be empty",
                        message_index, part_index
                    )));
                }
            }
            ContentPart::ImageUrl { image_url } => {
                Self::validate_image_url(&image_url.url)?;
                if let Some(detail) = &image_url.detail
                    && !["low", "high", "auto"].contains(&detail.as_str())
                {
                    return Err(GatewayError::Validation(
                        "Image detail must be 'low', 'high', or 'auto'".to_string(),
                    ));
                }
            }
            ContentPart::Audio { audio } => {
                Self::validate_audio_data(&audio.data)?;
                Self::validate_audio_format(&audio.format)?;
            }
            ContentPart::Image {
                source,
                detail,
                image_url,
            } => {
                if source.media_type.trim().is_empty()
                    || !source.media_type.to_ascii_lowercase().starts_with("image/")
                {
                    return Err(GatewayError::Validation(format!(
                        "Image part at message {} part {} must have image/* media_type",
                        message_index, part_index
                    )));
                }
                Self::validate_base64_payload(&source.data, "image")?;
                if let Some(detail) = detail
                    && !["low", "high", "auto"].contains(&detail.as_str())
                {
                    return Err(GatewayError::Validation(
                        "Image detail must be 'low', 'high', or 'auto'".to_string(),
                    ));
                }
                if let Some(url) = image_url {
                    Self::validate_image_url(&url.url)?;
                }
            }
            ContentPart::Document { source, .. } => {
                if source.media_type.trim().is_empty() {
                    return Err(GatewayError::Validation(format!(
                        "Document part at message {} part {} must have media_type",
                        message_index, part_index
                    )));
                }
                Self::validate_base64_payload(&source.data, "document")?;
            }
            ContentPart::ToolResult { tool_use_id, .. } => {
                if tool_use_id.trim().is_empty() {
                    return Err(GatewayError::Validation(format!(
                        "Tool result at message {} part {} must have non-empty tool_use_id",
                        message_index, part_index
                    )));
                }
            }
            ContentPart::ToolUse { id, name, .. } => {
                if id.trim().is_empty() || name.trim().is_empty() {
                    return Err(GatewayError::Validation(format!(
                        "Tool use at message {} part {} must have non-empty id/name",
                        message_index, part_index
                    )));
                }
            }
        }

        Ok(())
    }

    /// Validate model name
    fn validate_model_name(model: &str) -> Result<()> {
        if model.trim().is_empty() {
            return Err(GatewayError::Validation(
                "Model name cannot be empty".to_string(),
            ));
        }

        // Check for valid characters
        let model_regex = Regex::new(r"^[a-zA-Z0-9._/-]+$")
            .map_err(|e| GatewayError::Internal(format!("Regex error: {}", e)))?;

        if !model_regex.is_match(model) {
            return Err(GatewayError::Validation(
                "Model name contains invalid characters".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate function name
    fn validate_function_name(name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(GatewayError::Validation(
                "Function name cannot be empty".to_string(),
            ));
        }

        // Function names should follow identifier rules
        let name_regex = Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$")
            .map_err(|e| GatewayError::Internal(format!("Regex error: {}", e)))?;

        if !name_regex.is_match(name) {
            return Err(GatewayError::Validation(
                "Function name must be a valid identifier".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate image URL
    fn validate_image_url(url: &str) -> Result<()> {
        if url.starts_with("data:image/") {
            // Base64 encoded image
            Self::validate_base64_image(url)?;
        } else {
            // Regular URL
            url::Url::parse(url)
                .map_err(|e| GatewayError::Validation(format!("Invalid image URL: {}", e)))?;
        }
        Ok(())
    }

    /// Validate base64 image data
    fn validate_base64_image(data_url: &str) -> Result<()> {
        if !data_url.starts_with("data:image/") {
            return Err(GatewayError::Validation(
                "Invalid image data URL format".to_string(),
            ));
        }

        let parts: Vec<&str> = data_url.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(GatewayError::Validation(
                "Invalid image data URL format".to_string(),
            ));
        }

        // Validate base64 data
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, parts[1])
            .map_err(|e| GatewayError::Validation(format!("Invalid base64 image data: {}", e)))?;

        Ok(())
    }

    /// Validate audio data
    fn validate_audio_data(data: &str) -> Result<()> {
        Self::validate_base64_payload(data, "audio")?;
        Ok(())
    }

    fn validate_base64_payload(data: &str, kind: &str) -> Result<()> {
        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, data).map_err(|e| {
            GatewayError::Validation(format!("Invalid base64 {} data: {}", kind, e))
        })?;
        Ok(())
    }

    /// Validate audio format
    fn validate_audio_format(format: &str) -> Result<()> {
        let valid_formats = ["mp3", "wav", "flac", "m4a", "ogg", "webm"];
        if !valid_formats.contains(&format) {
            return Err(GatewayError::Validation(format!(
                "Invalid audio format: {}. Supported formats: {:?}",
                format, valid_formats
            )));
        }
        Ok(())
    }
}

// ==================== Unit Tests ====================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::models::openai::{
        ChatMessage, ContentPart, ImageUrl, MessageContent, MessageRole,
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
        let result =
            RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, None);
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
        let result =
            RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, None);
        assert!(result.is_ok());
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
        let result = RequestValidator::validate_chat_completion_request(
            "gpt-4",
            &messages,
            Some(100001),
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("100000"));
    }

    #[test]
    fn test_validate_max_tokens_valid_boundary() {
        let messages = vec![create_user_message("Hello")];
        let result = RequestValidator::validate_chat_completion_request(
            "gpt-4",
            &messages,
            Some(100000),
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_max_tokens_none() {
        let messages = vec![create_user_message("Hello")];
        let result =
            RequestValidator::validate_chat_completion_request("gpt-4", &messages, None, None);
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
        let result = RequestValidator::validate_chat_completion_request(
            "gpt-4",
            &messages,
            None,
            Some(-0.1),
        );
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
        let result =
            RequestValidator::validate_chat_completion_request("   ", &messages, None, None);
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
}
