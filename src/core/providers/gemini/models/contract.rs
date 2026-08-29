use super::{GeminiModelFamily, get_gemini_registry};
use crate::core::types::{
    chat::{ChatMessage, ChatRequest},
    content::ContentPart,
    message::{MessageContent, MessageRole},
};

/// Whether Google requires the July 2026 fixed-sampling request contract.
pub(crate) fn uses_fixed_sampling_contract(model_id: &str) -> bool {
    matches!(
        get_gemini_registry().get_model_family(model_id),
        Some(
            GeminiModelFamily::Gemini37Flash
                | GeminiModelFamily::Gemini36Flash
                | GeminiModelFamily::Gemini35FlashLite
        )
    )
}

pub(crate) fn has_trailing_assistant_prefill(request: &ChatRequest) -> bool {
    request
        .messages
        .iter()
        .rev()
        .filter(|message| !matches!(message.role, MessageRole::System | MessageRole::Developer))
        .find(|message| message_has_payload(message))
        .is_some_and(|message| message.role == MessageRole::Assistant)
}

fn message_has_payload(message: &ChatMessage) -> bool {
    let has_content = match message.content.as_ref() {
        Some(MessageContent::Text(text)) => !text.trim().is_empty(),
        Some(MessageContent::Parts(parts)) => parts.iter().any(|part| match part {
            ContentPart::Text { text } => !text.trim().is_empty(),
            _ => true,
        }),
        None => false,
    };
    has_content
        || message.audio.is_some()
        || message.thinking.is_some()
        || message.function_call.is_some()
        || message
            .tool_calls
            .as_ref()
            .is_some_and(|calls| !calls.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_sampling_contract_is_limited_to_july_2026_models() {
        assert!(uses_fixed_sampling_contract("gemini-3.7-flash"));
        assert!(uses_fixed_sampling_contract("gemini-3.6-flash"));
        assert!(uses_fixed_sampling_contract("gemini-3.5-flash-lite"));
        assert!(!uses_fixed_sampling_contract("gemini-3.5-flash"));
    }

    #[test]
    fn system_instruction_after_assistant_does_not_hide_prefill() {
        let request = ChatRequest {
            messages: vec![
                ChatMessage {
                    role: MessageRole::Assistant,
                    content: Some(MessageContent::Text("prefill".to_string())),
                    ..Default::default()
                },
                ChatMessage {
                    role: MessageRole::System,
                    content: Some(MessageContent::Text("instruction".to_string())),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert!(has_trailing_assistant_prefill(&request));
    }
}
