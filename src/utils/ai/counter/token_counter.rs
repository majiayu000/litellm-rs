//! Token counting implementation

use super::types::{ModelTokenConfig, TokenEstimate};
use crate::core::models::openai::{ChatMessage, ContentPart, MessageContent};
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::collections::HashMap;
use tiktoken_rs::{ChatCompletionRequestMessage, bpe_for_model, num_tokens_from_messages};

/// Token counter for different models
#[derive(Debug, Clone)]
pub struct TokenCounter {
    /// Model-specific token counting configurations
    model_configs: HashMap<String, ModelTokenConfig>,
}

/// Explicit tokenizer contract selected for a provider attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenizerIdentity {
    ExactOpenAi(String),
    Approximate { provider: String, model: String },
}

impl TokenizerIdentity {
    pub fn exact_openai(model: impl Into<String>) -> Self {
        Self::ExactOpenAi(model.into())
    }

    pub fn approximate(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self::Approximate {
            provider: provider.into(),
            model: model.into(),
        }
    }

    pub fn provider(&self) -> &str {
        match self {
            Self::ExactOpenAi(_) => "openai",
            Self::Approximate { provider, .. } => provider,
        }
    }

    pub fn model(&self) -> &str {
        match self {
            Self::ExactOpenAi(model) | Self::Approximate { model, .. } => model,
        }
    }

    fn tokenizer_model(&self) -> Result<&str> {
        if matches!(self, Self::ExactOpenAi(_)) && exact_bpe_for_model(self.model()).is_err() {
            return Err(GatewayError::Config(format!(
                "tokenizer unavailable for exact identity '{}/{}'",
                self.provider(),
                self.model()
            )));
        }
        Ok(self.model())
    }
}

impl TokenCounter {
    /// Create a new token counter
    pub fn new() -> Self {
        Self {
            model_configs: ModelTokenConfig::default_configs(),
        }
    }

    /// Count tokens in a completion request using an explicit tokenizer contract.
    pub fn count_completion_tokens(
        &self,
        identity: &TokenizerIdentity,
        prompt: &str,
    ) -> Result<TokenEstimate> {
        match identity {
            TokenizerIdentity::ExactOpenAi(_) => {
                self.exact_completion_tokens(identity.tokenizer_model()?, prompt)
            }
            TokenizerIdentity::Approximate { .. } => self.approximate_completion_tokens(prompt),
        }
    }

    /// Count tokens in chat messages using an explicit tokenizer contract.
    pub fn count_chat_tokens(
        &self,
        identity: &TokenizerIdentity,
        messages: &[ChatMessage],
    ) -> Result<TokenEstimate> {
        match identity {
            TokenizerIdentity::ExactOpenAi(_) => {
                let model = identity.tokenizer_model()?;
                let Some(total_tokens) = self.exact_chat_tokens(model, messages)? else {
                    return self.approximate_chat_tokens(messages);
                };
                Ok(TokenEstimate {
                    input_tokens: total_tokens,
                    output_tokens: None,
                    total_tokens,
                    is_approximate: false,
                    confidence: 1.0,
                })
            }
            TokenizerIdentity::Approximate { .. } => self.approximate_chat_tokens(messages),
        }
    }

    fn approximate_chat_tokens(&self, messages: &[ChatMessage]) -> Result<TokenEstimate> {
        let config = self.approximate_model_config()?;
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

    fn exact_completion_tokens(&self, model: &str, prompt: &str) -> Result<TokenEstimate> {
        let input_tokens = exact_text_tokens(model, prompt)?;
        Ok(TokenEstimate {
            input_tokens,
            output_tokens: None,
            total_tokens: input_tokens,
            is_approximate: false,
            confidence: 1.0,
        })
    }

    fn approximate_completion_tokens(&self, prompt: &str) -> Result<TokenEstimate> {
        let config = self.approximate_model_config()?;
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
    pub fn count_embedding_tokens(
        &self,
        identity: &TokenizerIdentity,
        input: &[String],
    ) -> Result<TokenEstimate> {
        if let TokenizerIdentity::ExactOpenAi(_) = identity {
            let model = identity.tokenizer_model()?;
            let total_tokens = input
                .iter()
                .map(|text| exact_text_tokens(model, text))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .sum();
            return Ok(TokenEstimate {
                input_tokens: total_tokens,
                output_tokens: None,
                total_tokens,
                is_approximate: false,
                confidence: 1.0,
            });
        }

        let config = self.approximate_model_config()?;
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
        identity: &TokenizerIdentity,
    ) -> Result<u32> {
        let config = self.config_for_identity(identity)?;

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
        identity: &TokenizerIdentity,
        input_tokens: u32,
        max_output_tokens: Option<u32>,
    ) -> Result<bool> {
        let config = self.config_for_identity(identity)?;
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

    fn approximate_model_config(&self) -> Result<&ModelTokenConfig> {
        self.model_configs
            .get("default")
            .ok_or_else(|| GatewayError::Config("No default token config found".to_string()))
    }

    fn config_for_identity(&self, identity: &TokenizerIdentity) -> Result<&ModelTokenConfig> {
        match identity {
            TokenizerIdentity::ExactOpenAi(_) => self.get_model_config(identity.tokenizer_model()?),
            TokenizerIdentity::Approximate { .. } => self.approximate_model_config(),
        }
    }

    /// Extract model family from model name
    pub(super) fn extract_model_family(&self, model: &str) -> String {
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

    fn exact_chat_tokens(&self, model: &str, messages: &[ChatMessage]) -> Result<Option<u32>> {
        let mut tiktoken_messages = Vec::with_capacity(messages.len());
        for message in messages {
            if message.tool_call_id.is_some() {
                return Ok(None);
            }
            if message
                .tool_calls
                .as_ref()
                .is_some_and(|calls| !calls.is_empty())
            {
                return Ok(None);
            }
            let Some(content) = plain_text_message_content(message) else {
                return Ok(None);
            };
            tiktoken_messages.push(ChatCompletionRequestMessage {
                role: ToString::to_string(&message.role),
                content,
                name: message.name.clone(),
                function_call: message.function_call.as_ref().map(|function_call| {
                    tiktoken_rs::FunctionCall {
                        name: function_call.name.clone(),
                        arguments: function_call.arguments.clone(),
                    }
                }),
                tool_calls: message
                    .tool_calls
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .map(|tool_call| tiktoken_rs::FunctionCall {
                        name: tool_call.function.name.clone(),
                        arguments: tool_call.function.arguments.clone(),
                    })
                    .collect(),
                refusal: None,
            });
        }
        num_tokens_from_messages(model, &tiktoken_messages)
            .map_err(|_| {
                GatewayError::Config(format!(
                    "tokenizer unavailable for exact identity 'openai/{model}'"
                ))
            })
            .and_then(usize_to_u32)
            .map(Some)
    }
}

impl Default for TokenCounter {
    fn default() -> Self {
        Self::new()
    }
}

fn exact_text_tokens(model: &str, text: &str) -> Result<u32> {
    exact_bpe_for_model(model)
        .map_err(|_| {
            GatewayError::Config(format!(
                "tokenizer unavailable for exact identity 'openai/{model}'"
            ))
        })
        .and_then(|bpe| usize_to_u32(bpe.count_with_special_tokens(text)))
}

fn exact_bpe_for_model(model: &str) -> std::result::Result<&'static tiktoken_rs::CoreBPE, ()> {
    bpe_for_model(model).map_err(|_| ())
}

fn plain_text_message_content(message: &ChatMessage) -> Option<Option<String>> {
    match &message.content {
        None => Some(None),
        Some(MessageContent::Text(text)) => Some(Some(text.clone())),
        Some(MessageContent::Parts(_)) => None,
    }
}

fn usize_to_u32(value: usize) -> Result<u32> {
    u32::try_from(value)
        .map_err(|_| GatewayError::Config(format!("token count {value} exceeds u32 range")))
}

#[cfg(test)]
#[path = "token_counter_tests.rs"]
mod tests;
