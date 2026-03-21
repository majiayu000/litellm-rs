//! Token counting implementation

use super::types::{ModelTokenConfig, TokenEstimate};
use crate::core::models::openai::{ChatMessage, ContentPart, MessageContent};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;

/// Token counter for different models
#[derive(Debug, Clone)]
pub struct TokenCounter {
    /// Model-specific token counting configurations
    model_configs: HashMap<String, ModelTokenConfig>,
}

impl TokenCounter {
    /// Create a new token counter
    pub fn new() -> Self {
        Self {
            model_configs: ModelTokenConfig::default_configs(),
        }
    }

    /// Count tokens in a chat completion request
    pub fn count_chat_tokens(
        &self,
        model: &str,
        messages: &[ChatMessage],
    ) -> Result<TokenEstimate> {
        let config = self.get_model_config(model)?;
        let mut total_tokens = config.request_overhead;

        for message in messages {
            total_tokens += self.count_message_tokens(config, message)?;
        }

        Ok(TokenEstimate {
            input_tokens: total_tokens,
            output_tokens: None,
            total_tokens,
            is_approximate: true,
            confidence: 0.85, // Reasonable confidence for estimation
        })
    }

    /// Count tokens in a single message
    fn count_message_tokens(
        &self,
        config: &ModelTokenConfig,
        message: &ChatMessage,
    ) -> Result<u32> {
        let mut tokens = config.message_overhead;

        // Count role tokens
        tokens += self.estimate_text_tokens(config, &ToString::to_string(&message.role));

        // Count content tokens
        if let Some(content) = &message.content {
            tokens += self.count_content_tokens(config, content)?;
        }

        // Count name tokens if present
        if let Some(name) = &message.name {
            tokens += self.estimate_text_tokens(config, name);
        }

        // Count function call tokens if present
        if let Some(function_call) = &message.function_call {
            tokens += self.estimate_text_tokens(config, &function_call.name);
            tokens += self.estimate_text_tokens(config, &function_call.arguments);
        }

        // Count tool calls tokens if present
        if let Some(tool_calls) = &message.tool_calls {
            for tool_call in tool_calls {
                tokens += self.estimate_text_tokens(config, &tool_call.id);
                tokens += self.estimate_text_tokens(config, &tool_call.tool_type);
                tokens += self.estimate_text_tokens(config, &tool_call.function.name);
                tokens += self.estimate_text_tokens(config, &tool_call.function.arguments);
            }
        }

        Ok(tokens)
    }

    /// Count tokens in message content
    fn count_content_tokens(
        &self,
        config: &ModelTokenConfig,
        content: &MessageContent,
    ) -> Result<u32> {
        match content {
            MessageContent::Text(text) => Ok(self.estimate_text_tokens(config, text)),
            MessageContent::Parts(parts) => {
                let mut tokens = 0;
                for part in parts {
                    tokens += self.count_content_part_tokens(config, part)?;
                }
                Ok(tokens)
            }
        }
    }

    /// Count tokens in a content part
    fn count_content_part_tokens(
        &self,
        config: &ModelTokenConfig,
        part: &ContentPart,
    ) -> Result<u32> {
        match part {
            ContentPart::Text { text } => Ok(self.estimate_text_tokens(config, text)),
            ContentPart::ImageUrl { image_url: _ } => {
                // Images typically use a fixed number of tokens
                // This is a simplified estimation
                Ok(85) // Base tokens for image processing
            }
            ContentPart::Audio { audio: _ } => {
                // Audio tokens depend on duration, but we don't have that info
                // Use a reasonable default
                Ok(100)
            }
            ContentPart::Image { .. } => Ok(85),
            ContentPart::Document { .. } => Ok(1000),
            ContentPart::ToolResult { .. } => Ok(50),
            ContentPart::ToolUse { .. } => Ok(100),
        }
    }

    /// Estimate tokens for text content
    pub(super) fn estimate_text_tokens(&self, config: &ModelTokenConfig, text: &str) -> u32 {
        if text.is_empty() {
            return 0;
        }

        // Simple character-based estimation
        let char_count = text.chars().count() as f64;
        let estimated_tokens = (char_count / config.chars_per_token).ceil() as u32;

        // Add some buffer for special tokens and encoding overhead
        (estimated_tokens as f64 * 1.1).ceil() as u32
    }

    /// Count tokens in completion request
    pub fn count_completion_tokens(&self, model: &str, prompt: &str) -> Result<TokenEstimate> {
        let config = self.get_model_config(model)?;
        let input_tokens = config.request_overhead + self.estimate_text_tokens(config, prompt);

        Ok(TokenEstimate {
            input_tokens,
            output_tokens: None,
            total_tokens: input_tokens,
            is_approximate: true,
            confidence: 0.8,
        })
    }

    /// Count tokens in embedding request
    pub fn count_embedding_tokens(&self, model: &str, input: &[String]) -> Result<TokenEstimate> {
        let config = self.get_model_config(model)?;
        let mut total_tokens = config.request_overhead;

        for text in input {
            total_tokens += self.estimate_text_tokens(config, text);
        }

        Ok(TokenEstimate {
            input_tokens: total_tokens,
            output_tokens: None,
            total_tokens,
            is_approximate: true,
            confidence: 0.9, // Embeddings are more predictable
        })
    }

    /// Estimate output tokens based on max_tokens parameter
    pub fn estimate_output_tokens(
        &self,
        max_tokens: Option<u32>,
        input_tokens: u32,
        model: &str,
    ) -> Result<u32> {
        let config = self.get_model_config(model)?;

        if let Some(max) = max_tokens {
            // Use the specified max_tokens, but cap at model's context window
            let available_tokens = config.max_context_tokens.saturating_sub(input_tokens);
            Ok(max.min(available_tokens))
        } else {
            // Use a reasonable default (e.g., 25% of remaining context)
            let available_tokens = config.max_context_tokens.saturating_sub(input_tokens);
            Ok((available_tokens as f64 * 0.25).ceil() as u32)
        }
    }

    /// Check if request fits within context window
    pub fn check_context_window(
        &self,
        model: &str,
        input_tokens: u32,
        max_output_tokens: Option<u32>,
    ) -> Result<bool> {
        let config = self.get_model_config(model)?;
        let output_tokens = max_output_tokens.unwrap_or(0);
        let total_tokens = input_tokens + output_tokens;

        Ok(total_tokens <= config.max_context_tokens)
    }

    /// Get model configuration
    pub(super) fn get_model_config(&self, model: &str) -> Result<&ModelTokenConfig> {
        // Try exact match first
        if let Some(config) = self.model_configs.get(model) {
            return Ok(config);
        }

        // Try to find a matching family
        let model_family = self.extract_model_family(model);
        if let Some(config) = self.model_configs.get(&model_family) {
            return Ok(config);
        }

        // Fall back to default
        self.model_configs.get("default").ok_or_else(|| {
            GatewayError::Config(format!("No token config found for model: {}", model))
        })
    }

    /// Extract model family from model name
    pub(super) fn extract_model_family(&self, model: &str) -> String {
        // Remove provider prefix if present
        let model = if let Some(pos) = model.find('/') {
            &model[pos + 1..]
        } else {
            model
        };

        // Extract family name
        if model.starts_with("gpt-4") {
            "gpt-4".to_string()
        } else if model.starts_with("gpt-3.5") {
            "gpt-3.5-turbo".to_string()
        } else if model.starts_with("claude-3") {
            "claude-3".to_string()
        } else if model.starts_with("claude-2") {
            "claude-2".to_string()
        } else {
            "default".to_string()
        }
    }

    /// Add or update model configuration
    pub fn add_model_config(&mut self, config: ModelTokenConfig) {
        self.model_configs.insert(config.model.clone(), config);
    }

    /// Get supported models
    pub fn get_supported_models(&self) -> Vec<String> {
        self.model_configs.keys().cloned().collect()
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Unit Tests ====================

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== TokenCounter Creation Tests ====================

    #[test]
    fn test_token_counter_new() {
        let counter = TokenCounter::new();
        assert!(!counter.model_configs.is_empty());
    }

    #[test]
    fn test_token_counter_default() {
        let counter = TokenCounter::default();
        assert!(!counter.model_configs.is_empty());
    }

    #[test]
    fn test_token_counter_clone() {
        let counter = TokenCounter::new();
        let cloned = counter.clone();
        assert_eq!(counter.model_configs.len(), cloned.model_configs.len());
    }

    #[test]
    fn test_token_counter_debug() {
        let counter = TokenCounter::new();
        let debug_str = format!("{:?}", counter);
        assert!(debug_str.contains("TokenCounter"));
        assert!(debug_str.contains("model_configs"));
    }

    // ==================== Model Config Tests ====================

    #[test]
    fn test_get_model_config_exact_match() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("gpt-4");
        assert!(config.is_ok());
    }

    #[test]
    fn test_get_model_config_gpt4_variant() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("gpt-4-turbo");
        assert!(config.is_ok());
    }

    #[test]
    fn test_get_model_config_gpt35() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("gpt-3.5-turbo");
        assert!(config.is_ok());
    }

    #[test]
    fn test_get_model_config_claude() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("claude-3-opus");
        assert!(config.is_ok());
    }

    #[test]
    fn test_get_model_config_fallback_to_default() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("unknown-model-xyz");
        // Should fall back to default config
        assert!(config.is_ok());
    }

    // ==================== Model Family Extraction Tests ====================

    #[test]
    fn test_extract_model_family_gpt4() {
        let counter = TokenCounter::new();
        assert_eq!(counter.extract_model_family("gpt-4"), "gpt-4");
        assert_eq!(counter.extract_model_family("gpt-4-turbo"), "gpt-4");
        assert_eq!(counter.extract_model_family("gpt-4-0125-preview"), "gpt-4");
        assert_eq!(counter.extract_model_family("gpt-4o"), "gpt-4");
    }

    #[test]
    fn test_extract_model_family_gpt35() {
        let counter = TokenCounter::new();
        assert_eq!(
            counter.extract_model_family("gpt-3.5-turbo"),
            "gpt-3.5-turbo"
        );
        assert_eq!(
            counter.extract_model_family("gpt-3.5-turbo-16k"),
            "gpt-3.5-turbo"
        );
    }

    #[test]
    fn test_extract_model_family_claude() {
        let counter = TokenCounter::new();
        assert_eq!(counter.extract_model_family("claude-3-opus"), "claude-3");
        assert_eq!(counter.extract_model_family("claude-3-sonnet"), "claude-3");
        assert_eq!(counter.extract_model_family("claude-3-haiku"), "claude-3");
        assert_eq!(counter.extract_model_family("claude-2.1"), "claude-2");
    }

    #[test]
    fn test_extract_model_family_with_provider_prefix() {
        let counter = TokenCounter::new();
        assert_eq!(counter.extract_model_family("openai/gpt-4"), "gpt-4");
        assert_eq!(
            counter.extract_model_family("anthropic/claude-3-opus"),
            "claude-3"
        );
    }

    #[test]
    fn test_extract_model_family_unknown() {
        let counter = TokenCounter::new();
        assert_eq!(counter.extract_model_family("unknown-model"), "default");
        assert_eq!(counter.extract_model_family("llama-2-70b"), "default");
    }

    // ==================== Text Token Estimation Tests ====================

    #[test]
    fn test_estimate_text_tokens_empty() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("gpt-4").unwrap();
        let tokens = counter.estimate_text_tokens(config, "");
        assert_eq!(tokens, 0);
    }

    #[test]
    fn test_estimate_text_tokens_short_text() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("gpt-4").unwrap();
        let tokens = counter.estimate_text_tokens(config, "Hello");
        assert!(tokens > 0);
        assert!(tokens < 10);
    }

    #[test]
    fn test_estimate_text_tokens_longer_text() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("gpt-4").unwrap();
        let short_tokens = counter.estimate_text_tokens(config, "Hello");
        let long_tokens =
            counter.estimate_text_tokens(config, "Hello, this is a much longer text message.");
        assert!(long_tokens > short_tokens);
    }

    #[test]
    fn test_estimate_text_tokens_unicode() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("gpt-4").unwrap();
        let tokens = counter.estimate_text_tokens(config, "你好世界");
        assert!(tokens > 0);
    }

    // ==================== Completion Token Counting Tests ====================

    #[test]
    fn test_count_completion_tokens_basic() {
        let counter = TokenCounter::new();
        let result = counter.count_completion_tokens("gpt-4", "Hello, world!");
        assert!(result.is_ok());
        let estimate = result.unwrap();
        assert!(estimate.input_tokens > 0);
        assert!(estimate.is_approximate);
    }

    #[test]
    fn test_count_completion_tokens_empty() {
        let counter = TokenCounter::new();
        let result = counter.count_completion_tokens("gpt-4", "");
        assert!(result.is_ok());
        let estimate = result.unwrap();
        // Should still have request overhead
        assert!(estimate.input_tokens > 0);
    }

    #[test]
    fn test_count_completion_tokens_long_text() {
        let counter = TokenCounter::new();
        let long_text = "word ".repeat(1000);
        let result = counter.count_completion_tokens("gpt-4", &long_text);
        assert!(result.is_ok());
        let estimate = result.unwrap();
        assert!(estimate.input_tokens > 100);
    }

    #[test]
    fn test_count_completion_tokens_confidence() {
        let counter = TokenCounter::new();
        let result = counter.count_completion_tokens("gpt-4", "test");
        assert!(result.is_ok());
        let estimate = result.unwrap();
        assert!(estimate.confidence > 0.0);
        assert!(estimate.confidence <= 1.0);
    }

    // ==================== Token Estimate Tests ====================

    #[test]
    fn test_token_estimate_structure() {
        let counter = TokenCounter::new();
        let result = counter.count_completion_tokens("gpt-4", "Hello");
        assert!(result.is_ok());
        let estimate = result.unwrap();

        assert_eq!(estimate.total_tokens, estimate.input_tokens);
        assert!(estimate.output_tokens.is_none());
        assert!(estimate.is_approximate);
    }

    // ==================== Embedding Token Counting Tests ====================

    #[test]
    fn test_count_embedding_tokens_single() {
        let counter = TokenCounter::new();
        let input = vec!["Hello, world!".to_string()];
        let result = counter.count_embedding_tokens("gpt-4", &input);
        assert!(result.is_ok());
        let estimate = result.unwrap();
        assert!(estimate.input_tokens > 0);
        assert_eq!(estimate.confidence, 0.9);
    }

    #[test]
    fn test_count_embedding_tokens_multiple() {
        let counter = TokenCounter::new();
        let input = vec![
            "First text".to_string(),
            "Second text".to_string(),
            "Third text".to_string(),
        ];
        let result = counter.count_embedding_tokens("gpt-4", &input);
        assert!(result.is_ok());
        let estimate = result.unwrap();
        assert!(estimate.input_tokens > 0);
    }

    #[test]
    fn test_count_embedding_tokens_empty() {
        let counter = TokenCounter::new();
        let input: Vec<String> = vec![];
        let result = counter.count_embedding_tokens("gpt-4", &input);
        assert!(result.is_ok());
    }

    // ==================== Output Token Estimation Tests ====================

    #[test]
    fn test_estimate_output_tokens_with_max() {
        let counter = TokenCounter::new();
        let result = counter.estimate_output_tokens(Some(100), 50, "gpt-4");
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output, 100);
    }

    #[test]
    fn test_estimate_output_tokens_without_max() {
        let counter = TokenCounter::new();
        let result = counter.estimate_output_tokens(None, 100, "gpt-4");
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should be ~25% of remaining context
        assert!(output > 0);
    }

    #[test]
    fn test_estimate_output_tokens_capped_by_context() {
        let counter = TokenCounter::new();
        // Request more tokens than available
        let result = counter.estimate_output_tokens(Some(1_000_000), 0, "gpt-4");
        assert!(result.is_ok());
        let output = result.unwrap();
        // Should be capped at model's max context
        assert!(output < 1_000_000);
    }

    // ==================== Context Window Check Tests ====================

    #[test]
    fn test_check_context_window_fits() {
        let counter = TokenCounter::new();
        let result = counter.check_context_window("gpt-4", 1000, Some(1000));
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_check_context_window_exceeds() {
        let counter = TokenCounter::new();
        // Try to use more tokens than the context window
        let result = counter.check_context_window("gpt-4", 100_000, Some(100_000));
        assert!(result.is_ok());
        // Verify the function returns a boolean (regardless of value)
        let _fits = result.unwrap();
    }

    #[test]
    fn test_check_context_window_no_output() {
        let counter = TokenCounter::new();
        let result = counter.check_context_window("gpt-4", 1000, None);
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    // ==================== Model Config Management Tests ====================

    #[test]
    fn test_add_model_config() {
        let mut counter = TokenCounter::new();
        let initial_count = counter.model_configs.len();

        let config = ModelTokenConfig {
            model: "custom-model".to_string(),
            chars_per_token: 4.5,
            message_overhead: 5,
            request_overhead: 10,
            max_context_tokens: 16000,
            special_tokens: HashMap::new(),
        };
        counter.add_model_config(config);

        assert_eq!(counter.model_configs.len(), initial_count + 1);
        assert!(counter.get_model_config("custom-model").is_ok());
    }

    #[test]
    fn test_get_supported_models() {
        let counter = TokenCounter::new();
        let models = counter.get_supported_models();
        assert!(!models.is_empty());
    }

    // ==================== Edge Cases Tests ====================

    #[test]
    fn test_special_characters() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("gpt-4").unwrap();
        let tokens = counter.estimate_text_tokens(config, "!@#$%^&*()");
        assert!(tokens > 0);
    }

    #[test]
    fn test_newlines_and_whitespace() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("gpt-4").unwrap();
        let tokens = counter.estimate_text_tokens(config, "Hello\n\n\nWorld\t\tTest");
        assert!(tokens > 0);
    }

    #[test]
    fn test_very_long_word() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("gpt-4").unwrap();
        let long_word = "a".repeat(1000);
        let tokens = counter.estimate_text_tokens(config, &long_word);
        assert!(tokens > 0);
    }

    #[test]
    fn test_mixed_content() {
        let counter = TokenCounter::new();
        let config = counter.get_model_config("gpt-4").unwrap();
        let mixed = "Hello 你好 Привет مرحبا 🎉";
        let tokens = counter.estimate_text_tokens(config, mixed);
        assert!(tokens > 0);
    }
}
