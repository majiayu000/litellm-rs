use super::RequestValidator;
use crate::core::models::openai::{ChatMessage, ContentPart, MessageContent, MessageRole};
use crate::core::providers::ChatMessageContinuation;
use crate::utils::error::gateway_error::{GatewayError, Result};

impl RequestValidator {
    /// Validate chat completion request
    pub fn validate_chat_completion_request(
        model: &str,
        messages: &[ChatMessage],
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> Result<()> {
        Self::validate_chat_completion_request_inner(model, messages, None, max_tokens, temperature)
    }

    /// Validate a chat request carrying typed per-message provider extensions.
    pub(crate) fn validate_chat_completion_request_with_extensions(
        model: &str,
        messages: &[ChatMessage],
        extensions: &[ChatMessageContinuation],
        max_tokens: Option<u32>,
        temperature: Option<f32>,
    ) -> Result<()> {
        if messages.len() != extensions.len() {
            return Err(GatewayError::Validation(format!(
                "Message extension length mismatch: expected {}, got {}",
                messages.len(),
                extensions.len()
            )));
        }
        Self::validate_chat_completion_request_inner(
            model,
            messages,
            Some(extensions),
            max_tokens,
            temperature,
        )
    }

    fn validate_chat_completion_request_inner(
        model: &str,
        messages: &[ChatMessage],
        extensions: Option<&[ChatMessageContinuation]>,
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
            Self::validate_chat_message_with_extension(
                message,
                extensions.and_then(|items| items.get(i)),
                i,
            )?;
        }

        // Validate max_tokens
        if let Some(max_tokens) = max_tokens {
            let max_output_tokens = 100_000;
            if max_tokens == 0 {
                return Err(GatewayError::Validation(
                    "max_tokens must be greater than 0".to_string(),
                ));
            }
            if max_tokens > max_output_tokens {
                return Err(GatewayError::Validation(format!(
                    "max_tokens cannot exceed {max_output_tokens}"
                )));
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
    pub(super) fn validate_chat_message(message: &ChatMessage, index: usize) -> Result<()> {
        Self::validate_chat_message_with_extension(message, None, index)
    }

    fn validate_chat_message_with_extension(
        message: &ChatMessage,
        extension: Option<&ChatMessageContinuation>,
        index: usize,
    ) -> Result<()> {
        // Validate role
        match message.role {
            MessageRole::System | MessageRole::Developer | MessageRole::User => {
                // These roles should have content
                if message.content.is_none() {
                    return Err(GatewayError::Validation(format!(
                        "Message at index {} with role {:?} must have content",
                        index, message.role
                    )));
                }
            }
            MessageRole::Assistant => {
                let has_tools = message
                    .tool_calls
                    .as_ref()
                    .is_some_and(|calls| !calls.is_empty());
                let has_extension = extension.is_some_and(|value| !value.is_empty());
                if message.content.is_none()
                    && !has_tools
                    && message.function_call.is_none()
                    && !has_extension
                {
                    return Err(GatewayError::Validation(format!(
                        "Message at index {} with role {:?} must have content, tool calls, function_call, or a typed continuation",
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

        if extension.is_some_and(|value| !value.is_empty())
            && message.role != MessageRole::Assistant
        {
            return Err(GatewayError::Validation(format!(
                "Typed Anthropic continuation at message index {} requires assistant role",
                index
            )));
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
    pub(super) fn validate_message_content(content: &MessageContent, index: usize) -> Result<()> {
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
    pub(super) fn validate_content_part(
        part: &ContentPart,
        message_index: usize,
        part_index: usize,
    ) -> Result<()> {
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
}
