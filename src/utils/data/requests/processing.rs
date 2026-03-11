use crate::core::providers::unified_provider::ProviderError;

use super::types::{MessageContent, RequestUtils};

impl RequestUtils {
    pub fn process_system_message(
        system_message: &str,
        max_tokens: Option<u32>,
        model: &str,
    ) -> Result<String, ProviderError> {
        let mut processed = system_message.to_string();

        if let Some(max_tokens) = max_tokens
            && processed.len() > (max_tokens as usize * 4)
        {
            processed = Self::truncate_message(&processed, max_tokens as usize * 4)?;
        }

        if Self::needs_model_specific_processing(model) {
            processed = Self::apply_model_specific_processing(&processed, model)?;
        }

        Ok(processed)
    }

    pub fn process_messages(
        messages: &mut Vec<MessageContent>,
        max_tokens: Option<u32>,
        model: &str,
    ) -> Result<(), ProviderError> {
        if let Some(token_limit) = max_tokens {
            Self::trim_messages_to_fit_limit(messages, token_limit, model)?;
        }

        for message in messages.iter_mut() {
            if Self::needs_model_specific_processing(model) {
                message.content = Self::apply_model_specific_processing(&message.content, model)?;
            }
        }

        Ok(())
    }

    fn trim_messages_to_fit_limit(
        messages: &mut Vec<MessageContent>,
        max_tokens: u32,
        model: &str,
    ) -> Result<(), ProviderError> {
        let estimated_tokens = Self::estimate_total_tokens(messages, model);

        if estimated_tokens <= max_tokens as usize {
            return Ok(());
        }

        while messages.len() > 1
            && Self::estimate_total_tokens(messages, model) > max_tokens as usize
        {
            messages.remove(0);
        }

        if Self::estimate_total_tokens(messages, model) > max_tokens as usize
            && let Some(last_message) = messages.last_mut()
        {
            let target_length = (max_tokens as usize * 3).saturating_sub(100);
            last_message.content = Self::truncate_message(&last_message.content, target_length)?;
        }

        Ok(())
    }

    fn estimate_total_tokens(messages: &[MessageContent], _model: &str) -> usize {
        messages
            .iter()
            .map(|msg| msg.content.split_whitespace().count() + 10)
            .sum()
    }

    pub(super) fn truncate_message(
        message: &str,
        max_length: usize,
    ) -> Result<String, ProviderError> {
        if message.len() <= max_length {
            return Ok(message.to_string());
        }

        let mut truncated = message.chars().take(max_length - 3).collect::<String>();
        truncated.push_str("...");
        Ok(truncated)
    }

    fn needs_model_specific_processing(model: &str) -> bool {
        let model_lower = model.to_lowercase();
        model_lower.contains("claude") || model_lower.contains("palm")
    }

    fn apply_model_specific_processing(
        content: &str,
        model: &str,
    ) -> Result<String, ProviderError> {
        let model_lower = model.to_lowercase();

        if model_lower.contains("claude") {
            Ok(Self::process_for_claude(content))
        } else if model_lower.contains("palm") {
            Ok(Self::process_for_palm(content))
        } else {
            Ok(content.to_string())
        }
    }

    fn process_for_claude(content: &str) -> String {
        content
            .replace("Assistant:", "")
            .replace("Human:", "")
            .trim()
            .to_string()
    }

    fn process_for_palm(content: &str) -> String {
        content.trim().to_string()
    }
}
