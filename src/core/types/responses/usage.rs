//! Usage statistics types

use serde::{Deserialize, Serialize};

use super::super::thinking::ThinkingUsage;

/// Usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Usage {
    /// Prompt token count
    pub prompt_tokens: u32,

    /// Completion token count
    pub completion_tokens: u32,

    /// Total token count
    pub total_tokens: u32,

    /// Prompt token details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_tokens_details: Option<PromptTokensDetails>,

    /// Completion token details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion_tokens_details: Option<CompletionTokensDetails>,

    /// Thinking/reasoning usage statistics
    ///
    /// Contains detailed breakdown of thinking tokens and costs
    /// for thinking-enabled models (OpenAI o-series, Claude thinking,
    /// DeepSeek R1, Gemini thinking).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_usage: Option<ThinkingUsage>,
}

impl Usage {
    pub fn new(prompt_tokens: u32, completion_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: None,
        }
    }

    /// Create usage with thinking statistics
    pub fn with_thinking(mut self, thinking: ThinkingUsage) -> Self {
        self.thinking_usage = Some(thinking);
        self
    }

    /// Get thinking tokens count (convenience method)
    pub fn thinking_tokens(&self) -> Option<u32> {
        self.thinking_usage
            .as_ref()
            .and_then(|t| t.thinking_tokens)
            .or_else(|| {
                // Fallback to completion_tokens_details.reasoning_tokens
                self.completion_tokens_details
                    .as_ref()
                    .and_then(|d| d.reasoning_tokens)
            })
    }
}

/// Prompt token details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTokensDetails {
    /// Cached token count
    pub cached_tokens: Option<u32>,

    /// Tokens written into provider-side prompt cache
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_tokens: Option<u32>,

    /// Tokens read from provider-side prompt cache
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_tokens: Option<u32>,

    /// Audio token count
    pub audio_tokens: Option<u32>,
}

/// Completion token details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionTokensDetails {
    /// Reasoning token count
    pub reasoning_tokens: Option<u32>,

    /// Audio token count
    pub audio_tokens: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_default() {
        let usage = Usage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
        assert!(usage.thinking_usage.is_none());
    }

    #[test]
    fn test_usage_new() {
        let usage = Usage::new(100, 50);
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_usage_with_thinking() {
        let thinking = ThinkingUsage {
            thinking_tokens: Some(200),
            budget_tokens: Some(1000),
            thinking_cost: Some(0.05),
            provider: Some("openai".to_string()),
        };

        let usage = Usage::new(100, 300).with_thinking(thinking);
        assert!(usage.thinking_usage.is_some());
        assert_eq!(usage.thinking_tokens(), Some(200));
    }

    #[test]
    fn test_usage_thinking_tokens_none() {
        let usage = Usage::new(100, 50);
        assert_eq!(usage.thinking_tokens(), None);
    }

    #[test]
    fn test_usage_thinking_tokens_from_thinking_usage() {
        let usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            prompt_tokens_details: None,
            completion_tokens_details: None,
            thinking_usage: Some(ThinkingUsage {
                thinking_tokens: Some(300),
                budget_tokens: None,
                thinking_cost: None,
                provider: None,
            }),
        };

        assert_eq!(usage.thinking_tokens(), Some(300));
    }

    #[test]
    fn test_usage_thinking_tokens_fallback_to_reasoning() {
        let usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            prompt_tokens_details: None,
            completion_tokens_details: Some(CompletionTokensDetails {
                reasoning_tokens: Some(150),
                audio_tokens: None,
            }),
            thinking_usage: None,
        };

        assert_eq!(usage.thinking_tokens(), Some(150));
    }

    #[test]
    fn test_usage_serialization() {
        let usage = Usage::new(100, 50);
        let json = serde_json::to_value(&usage).unwrap();

        assert_eq!(json["prompt_tokens"], 100);
        assert_eq!(json["completion_tokens"], 50);
        assert_eq!(json["total_tokens"], 150);
    }

    #[test]
    fn test_usage_deserialization() {
        let json = r#"{"prompt_tokens": 200, "completion_tokens": 100, "total_tokens": 300}"#;
        let usage: Usage = serde_json::from_str(json).unwrap();

        assert_eq!(usage.prompt_tokens, 200);
        assert_eq!(usage.completion_tokens, 100);
        assert_eq!(usage.total_tokens, 300);
    }

    #[test]
    fn test_usage_clone() {
        let usage = Usage::new(100, 50);
        let cloned = usage.clone();

        assert_eq!(usage.prompt_tokens, cloned.prompt_tokens);
        assert_eq!(usage.total_tokens, cloned.total_tokens);
    }

    #[test]
    fn test_prompt_tokens_details() {
        let details = PromptTokensDetails {
            cached_tokens: Some(50),
            cache_creation_tokens: Some(5),
            cache_read_tokens: Some(45),
            audio_tokens: Some(10),
        };

        assert_eq!(details.cached_tokens, Some(50));
        assert_eq!(details.cache_creation_tokens, Some(5));
        assert_eq!(details.cache_read_tokens, Some(45));
        assert_eq!(details.audio_tokens, Some(10));
    }

    #[test]
    fn test_completion_tokens_details() {
        let details = CompletionTokensDetails {
            reasoning_tokens: Some(100),
            audio_tokens: Some(20),
        };

        assert_eq!(details.reasoning_tokens, Some(100));
        assert_eq!(details.audio_tokens, Some(20));
    }

    #[test]
    fn test_usage_with_all_details() {
        let usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 200,
            total_tokens: 300,
            prompt_tokens_details: Some(PromptTokensDetails {
                cached_tokens: Some(30),
                cache_creation_tokens: Some(7),
                cache_read_tokens: Some(23),
                audio_tokens: None,
            }),
            completion_tokens_details: Some(CompletionTokensDetails {
                reasoning_tokens: Some(50),
                audio_tokens: Some(10),
            }),
            thinking_usage: None,
        };

        assert_eq!(
            usage.prompt_tokens_details.as_ref().unwrap().cached_tokens,
            Some(30)
        );
        assert_eq!(
            usage
                .prompt_tokens_details
                .as_ref()
                .unwrap()
                .cache_creation_tokens,
            Some(7)
        );
        assert_eq!(
            usage
                .prompt_tokens_details
                .as_ref()
                .unwrap()
                .cache_read_tokens,
            Some(23)
        );
        assert_eq!(
            usage
                .completion_tokens_details
                .as_ref()
                .unwrap()
                .reasoning_tokens,
            Some(50)
        );
    }

    #[test]
    fn test_usage_serialization_skip_none() {
        let usage = Usage::new(100, 50);
        let json = serde_json::to_string(&usage).unwrap();

        // None fields should be skipped
        assert!(!json.contains("prompt_tokens_details"));
        assert!(!json.contains("completion_tokens_details"));
        assert!(!json.contains("thinking_usage"));
    }
}
