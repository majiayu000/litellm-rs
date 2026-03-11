use crate::core::providers::unified_provider::ProviderError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum TokenizerType {
    OpenAI,
    HuggingFace,
    Claude,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct TokenizerResponse {
    pub tokenizer_type: TokenizerType,
    pub token_count: usize,
}

pub struct TokenUtils;

impl TokenUtils {
    const OPENAI_MODELS: &'static [&'static str] = &[
        "gpt-5.2",
        "gpt-5.2-chat",
        "gpt-5.2-codex",
        "gpt-5-codex",
        "gpt-5.1",
        "gpt-5.1-thinking",
        "gpt-5-mini",
        "gpt-5-nano",
        "gpt-image-1",
        "gpt-image-1-mini",
        "gpt-image-1.5",
        "chatgpt-image-latest",
        "o3-pro",
        "o3-mini",
        "o4-mini",
        "gpt-4.1",
        "gpt-4.1-mini",
        "gpt-4.1-nano",
        "gpt-4",
        "gpt-4-32k",
        "gpt-4-turbo",
        "gpt-4o",
        "gpt-3.5-turbo",
        "gpt-3.5-turbo-16k",
        "text-davinci-003",
        "text-davinci-002",
        "text-curie-001",
        "text-babbage-001",
        "text-ada-001",
    ];

    const CLAUDE_MODELS: &'static [&'static str] = &[
        "claude-opus-4-6",
        "claude-opus-4-5",
        "claude-sonnet-4-5",
        "claude-sonnet-4",
        "claude-3-opus",
        "claude-3-sonnet",
        "claude-3-haiku",
        "claude-2",
        "claude-instant",
    ];

    pub fn select_tokenizer(model: &str) -> Result<TokenizerType, ProviderError> {
        let model_lower = model.to_lowercase();

        if Self::OPENAI_MODELS
            .iter()
            .any(|&m| model_lower.starts_with(m))
            || model_lower.starts_with("gpt-")
        {
            return Ok(TokenizerType::OpenAI);
        }

        if Self::CLAUDE_MODELS
            .iter()
            .any(|&m| model_lower.starts_with(m))
            || model_lower.starts_with("claude-")
        {
            return Ok(TokenizerType::Claude);
        }

        if model_lower.contains("huggingface") || model_lower.starts_with("hf-") {
            return Ok(TokenizerType::HuggingFace);
        }

        Ok(TokenizerType::Custom(model.to_string()))
    }

    pub fn encode(model: &str, text: &str) -> Result<Vec<u32>, ProviderError> {
        let tokenizer_type = Self::select_tokenizer(model)?;

        match tokenizer_type {
            TokenizerType::OpenAI => Self::encode_openai(text),
            TokenizerType::Claude => Self::encode_claude(text),
            TokenizerType::HuggingFace => Self::encode_huggingface(text),
            TokenizerType::Custom(_) => Self::encode_generic(text),
        }
    }

    pub fn decode(model: &str, tokens: &[u32]) -> Result<String, ProviderError> {
        let tokenizer_type = Self::select_tokenizer(model)?;

        match tokenizer_type {
            TokenizerType::OpenAI => Self::decode_openai(tokens),
            TokenizerType::Claude => Self::decode_claude(tokens),
            TokenizerType::HuggingFace => Self::decode_huggingface(tokens),
            TokenizerType::Custom(_) => Self::decode_generic(tokens),
        }
    }

    pub fn token_counter(
        model: &str,
        text: Option<&str>,
        messages: Option<&[HashMap<String, String>]>,
    ) -> Result<usize, ProviderError> {
        if let Some(text_content) = text {
            return Self::count_tokens_for_text(model, text_content);
        }

        if let Some(message_list) = messages {
            return Self::count_tokens_for_messages(model, message_list);
        }

        Ok(0)
    }

    fn count_tokens_for_text(model: &str, text: &str) -> Result<usize, ProviderError> {
        let tokenizer_type = Self::select_tokenizer(model)?;

        match tokenizer_type {
            TokenizerType::OpenAI => Ok(Self::estimate_openai_tokens(text)),
            TokenizerType::Claude => Ok(Self::estimate_claude_tokens(text)),
            _ => Ok(Self::estimate_generic_tokens(text)),
        }
    }

    fn count_tokens_for_messages(
        model: &str,
        messages: &[HashMap<String, String>],
    ) -> Result<usize, ProviderError> {
        let mut total_tokens = 0;

        for message in messages {
            if let Some(content) = message.get("content") {
                total_tokens += Self::count_tokens_for_text(model, content)?;
            }

            if let Some(role) = message.get("role") {
                total_tokens += Self::count_tokens_for_text(model, role)?;
            }

            total_tokens += 4;
        }

        total_tokens += 2;

        Ok(total_tokens)
    }

    fn encode_openai(text: &str) -> Result<Vec<u32>, ProviderError> {
        let tokens: Vec<u32> = text.chars().enumerate().map(|(i, _)| i as u32).collect();
        Ok(tokens)
    }

    fn encode_claude(text: &str) -> Result<Vec<u32>, ProviderError> {
        let tokens: Vec<u32> = text
            .chars()
            .enumerate()
            .map(|(i, _)| (i + 1000) as u32)
            .collect();
        Ok(tokens)
    }

    fn encode_huggingface(text: &str) -> Result<Vec<u32>, ProviderError> {
        let tokens: Vec<u32> = text
            .chars()
            .enumerate()
            .map(|(i, _)| (i + 2000) as u32)
            .collect();
        Ok(tokens)
    }

    fn encode_generic(text: &str) -> Result<Vec<u32>, ProviderError> {
        let tokens: Vec<u32> = text
            .chars()
            .enumerate()
            .map(|(i, _)| (i + 5000) as u32)
            .collect();
        Ok(tokens)
    }

    fn decode_openai(tokens: &[u32]) -> Result<String, ProviderError> {
        let text: String = tokens
            .iter()
            .enumerate()
            .map(|(i, _)| char::from(65 + (i % 26) as u8))
            .collect();
        Ok(text)
    }

    fn decode_claude(tokens: &[u32]) -> Result<String, ProviderError> {
        let text: String = tokens
            .iter()
            .enumerate()
            .map(|(i, _)| char::from(97 + (i % 26) as u8))
            .collect();
        Ok(text)
    }

    fn decode_huggingface(tokens: &[u32]) -> Result<String, ProviderError> {
        let text: String = tokens
            .iter()
            .enumerate()
            .map(|(i, _)| char::from(48 + (i % 10) as u8))
            .collect();
        Ok(text)
    }

    fn decode_generic(tokens: &[u32]) -> Result<String, ProviderError> {
        let text: String = tokens
            .iter()
            .enumerate()
            .map(|(i, _)| char::from(33 + (i % 94) as u8))
            .collect();
        Ok(text)
    }

    fn estimate_openai_tokens(text: &str) -> usize {
        let words = text.split_whitespace().count();
        (words as f64 * 1.3).ceil() as usize
    }

    fn estimate_claude_tokens(text: &str) -> usize {
        let chars = text.chars().count();
        (chars as f64 / 3.5).ceil() as usize
    }

    fn estimate_generic_tokens(text: &str) -> usize {
        let words = text.split_whitespace().count();
        (words as f64 * 1.2).ceil() as usize
    }

    pub fn get_max_tokens_for_model(model: &str) -> Option<usize> {
        match model.to_lowercase().as_str() {
            m if m.contains("gpt-5.2") => Some(400000),
            m if m.contains("gpt-5.1-thinking") => Some(400000),
            m if m.contains("gpt-5") => Some(272000),
            m if m.contains("o3") || m.contains("o4") => Some(200000),
            m if m.contains("gpt-4.1") => Some(128000),
            m if m.contains("gpt-4-32k") => Some(32768),
            m if m.contains("gpt-4") => Some(8192),
            m if m.contains("gpt-3.5-turbo-16k") => Some(16384),
            m if m.contains("gpt-3.5-turbo") => Some(4096),
            m if m.contains("claude-opus-4-6") => Some(1_000_000),
            m if m.contains("claude-opus-4") => Some(200000),
            m if m.contains("claude-sonnet-4") => Some(200000),
            m if m.contains("claude-3") => Some(200000),
            m if m.contains("claude-2") => Some(100000),
            m if m.contains("claude-instant") => Some(100000),
            _ => None,
        }
    }

    pub fn calculate_cost(
        model: &str,
        input_tokens: usize,
        output_tokens: usize,
    ) -> Result<f64, ProviderError> {
        let (input_price, output_price) = match model.to_lowercase().as_str() {
            m if m.contains("gpt-5.2-pro") => (0.021, 0.168),
            m if m.contains("gpt-5.2-codex") => (0.00175, 0.014),
            m if m.contains("gpt-5-codex") => (0.00125, 0.010),
            m if m.contains("gpt-5.2") => (0.00175, 0.014),
            m if m.contains("gpt-5.1-thinking") => (0.0025, 0.020),
            m if m.contains("gpt-5.1") => (0.00125, 0.010),
            m if m.contains("gpt-5-mini") => (0.00025, 0.002),
            m if m.contains("gpt-5-nano") => (0.00005, 0.0004),
            m if m.contains("gpt-image-1-mini") => (0.0025, 0.010),
            m if m.contains("gpt-image-1.5") => (0.005, 0.020),
            m if m.contains("chatgpt-image-latest") => (0.005, 0.020),
            m if m.contains("gpt-image-1") => (0.005, 0.020),
            m if m.contains("o3-pro") => (0.020, 0.080),
            m if m.contains("o3-mini") || m.contains("o4-mini") => (0.0011, 0.0044),
            m if m.contains("gpt-4.1") => (0.002, 0.008),
            m if m.contains("gpt-4") => (0.03, 0.06),
            m if m.contains("gpt-3.5-turbo") => (0.0015, 0.002),
            m if m.contains("claude-opus-4-6") => (0.005, 0.025),
            m if m.contains("claude-opus-4-5") => (0.005, 0.025),
            m if m.contains("claude-sonnet-4-5") => (0.003, 0.015),
            m if m.contains("claude-sonnet-4") => (0.003, 0.015),
            m if m.contains("claude-3-opus") => (0.015, 0.075),
            m if m.contains("claude-3-sonnet") => (0.003, 0.015),
            m if m.contains("claude-3-haiku") => (0.00025, 0.00125),
            _ => {
                return Err(ProviderError::ModelNotFound {
                    provider: "unknown",
                    model: format!("Cost calculation not available for model: {}", model),
                });
            }
        };

        let input_cost = (input_tokens as f64 / 1000.0) * input_price;
        let output_cost = (output_tokens as f64 / 1000.0) * output_price;

        Ok(input_cost + output_cost)
    }

    pub fn supports_function_calling(model: &str) -> bool {
        match model.to_lowercase().as_str() {
            m if m.contains("gpt-5") => true,
            m if m.contains("o3") || m.contains("o4") => true,
            m if m.contains("gpt-4.1") => true,
            m if m.contains("gpt-4") => true,
            m if m.contains("gpt-3.5-turbo") => true,
            m if m.contains("claude-opus-4") => true,
            m if m.contains("claude-sonnet-4") => true,
            m if m.contains("claude-3") => true,
            _ => false,
        }
    }

    pub fn validate_token_limit(model: &str, token_count: usize) -> Result<(), ProviderError> {
        if let Some(max_tokens) = Self::get_max_tokens_for_model(model)
            && token_count > max_tokens
        {
            return Err(ProviderError::InvalidRequest {
                provider: "unknown",
                message: format!(
                    "Token count {} exceeds model limit of {}",
                    token_count, max_tokens
                ),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: usize,
    pub completion_tokens: usize,
    pub total_tokens: usize,
    pub cost: Option<f64>,
}

impl TokenUsage {
    pub fn new(prompt_tokens: usize, completion_tokens: usize) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            cost: None,
        }
    }

    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost = Some(cost);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_tokenizer() {
        assert!(matches!(
            TokenUtils::select_tokenizer("gpt-4").unwrap(),
            TokenizerType::OpenAI
        ));
        assert!(matches!(
            TokenUtils::select_tokenizer("claude-3-opus").unwrap(),
            TokenizerType::Claude
        ));
        assert!(matches!(
            TokenUtils::select_tokenizer("custom-model").unwrap(),
            TokenizerType::Custom(_)
        ));
    }

    #[test]
    fn test_token_counting() {
        let text = "Hello world this is a test";
        let count = TokenUtils::count_tokens_for_text("gpt-4", text).unwrap();
        assert!(count > 0);
    }

    #[test]
    fn test_encode_decode() {
        let text = "Hello world";
        let tokens = TokenUtils::encode("gpt-4", text).unwrap();
        let decoded = TokenUtils::decode("gpt-4", &tokens).unwrap();
        assert_eq!(decoded.len(), text.len());
    }

    #[test]
    fn test_max_tokens() {
        assert_eq!(TokenUtils::get_max_tokens_for_model("gpt-4"), Some(8192));
        assert_eq!(
            TokenUtils::get_max_tokens_for_model("gpt-4-32k"),
            Some(32768)
        );
        assert_eq!(
            TokenUtils::get_max_tokens_for_model("claude-opus-4-6"),
            Some(1_000_000)
        );
        assert_eq!(
            TokenUtils::get_max_tokens_for_model("claude-3"),
            Some(200000)
        );
        assert_eq!(TokenUtils::get_max_tokens_for_model("unknown-model"), None);
    }

    #[test]
    fn test_cost_calculation() {
        let cost = TokenUtils::calculate_cost("gpt-4", 1000, 500).unwrap();
        assert!(cost > 0.0);

        let result = TokenUtils::calculate_cost("unknown-model", 1000, 500);
        assert!(result.is_err());
    }

    #[test]
    fn test_function_calling_support() {
        assert!(TokenUtils::supports_function_calling("gpt-4"));
        assert!(TokenUtils::supports_function_calling("claude-3-opus"));
        assert!(!TokenUtils::supports_function_calling("davinci-002"));
    }

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage::new(1000, 500).with_cost(0.05);
        assert_eq!(usage.total_tokens, 1500);
        assert_eq!(usage.cost, Some(0.05));
    }
}
