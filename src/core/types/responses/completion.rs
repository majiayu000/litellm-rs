//! Completion response types (non-chat)

use serde::{Deserialize, Serialize};

use super::logprobs::{FinishReason, LogProbs};
use super::usage::Usage;

/// Completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// Response ID
    pub id: String,

    /// Object type
    pub object: String,

    /// Creation timestamp
    pub created: i64,

    /// Model used
    pub model: String,

    /// Choice list
    pub choices: Vec<CompletionChoice>,

    /// Usage statistics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,

    /// System fingerprint
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
}

/// Completion choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionChoice {
    /// Choice index
    pub index: u32,

    /// Generated text
    pub text: String,

    /// Finish reason
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<FinishReason>,

    /// Log probability information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<LogProbs>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== CompletionResponse Tests ====================

    #[test]
    fn test_completion_response_creation() {
        let response = CompletionResponse {
            id: "cmpl-123".to_string(),
            object: "text_completion".to_string(),
            created: 1700000000,
            model: "gpt-3.5-turbo-instruct".to_string(),
            choices: vec![],
            usage: None,
            system_fingerprint: None,
        };
        assert_eq!(response.id, "cmpl-123");
        assert_eq!(response.object, "text_completion");
    }

    #[test]
    fn test_completion_response_with_choices() {
        let response = CompletionResponse {
            id: "cmpl-456".to_string(),
            object: "text_completion".to_string(),
            created: 1700000000,
            model: "text-davinci-003".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                text: "Hello, world!".to_string(),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].text, "Hello, world!");
    }

    #[test]
    fn test_completion_response_with_usage() {
        let response = CompletionResponse {
            id: "cmpl-789".to_string(),
            object: "text_completion".to_string(),
            created: 1700000000,
            model: "gpt-3.5-turbo-instruct".to_string(),
            choices: vec![],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
                cost: None,
            }),
            system_fingerprint: Some("fp_abc123".to_string()),
        };
        assert!(response.usage.is_some());
        assert!(response.system_fingerprint.is_some());
    }

    #[test]
    fn test_completion_response_serialization() {
        let response = CompletionResponse {
            id: "cmpl-test".to_string(),
            object: "text_completion".to_string(),
            created: 1699999999,
            model: "model".to_string(),
            choices: vec![],
            usage: None,
            system_fingerprint: None,
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("cmpl-test"));
        assert!(json.contains("text_completion"));
        assert!(json.contains("1699999999"));
        assert!(!json.contains("usage"));
        assert!(!json.contains("system_fingerprint"));
    }

    #[test]
    fn test_completion_response_deserialization() {
        let json = r#"{
            "id": "cmpl-abc",
            "object": "text_completion",
            "created": 1700000000,
            "model": "gpt-3.5-turbo-instruct",
            "choices": [
                {"index": 0, "text": "Response text", "finish_reason": "stop"}
            ]
        }"#;
        let response: CompletionResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "cmpl-abc");
        assert_eq!(response.choices.len(), 1);
        assert_eq!(response.choices[0].text, "Response text");
    }

    // ==================== CompletionChoice Tests ====================

    #[test]
    fn test_completion_choice_creation() {
        let choice = CompletionChoice {
            index: 0,
            text: "Generated text".to_string(),
            finish_reason: Some(FinishReason::Stop),
            logprobs: None,
        };
        assert_eq!(choice.index, 0);
        assert_eq!(choice.text, "Generated text");
    }

    #[test]
    fn test_completion_choice_with_length_finish() {
        let choice = CompletionChoice {
            index: 0,
            text: "Truncated...".to_string(),
            finish_reason: Some(FinishReason::Length),
            logprobs: None,
        };
        assert_eq!(choice.finish_reason, Some(FinishReason::Length));
    }

    #[test]
    fn test_completion_choice_with_logprobs() {
        let choice = CompletionChoice {
            index: 0,
            text: "Hello".to_string(),
            finish_reason: Some(FinishReason::Stop),
            logprobs: Some(LogProbs {
                content: vec![],
                refusal: None,
            }),
        };
        assert!(choice.logprobs.is_some());
    }

    #[test]
    fn test_completion_choice_serialization() {
        let choice = CompletionChoice {
            index: 1,
            text: "Test output".to_string(),
            finish_reason: Some(FinishReason::Stop),
            logprobs: None,
        };
        let json = serde_json::to_string(&choice).unwrap();
        assert!(json.contains("1"));
        assert!(json.contains("Test output"));
        assert!(json.contains("stop"));
        assert!(!json.contains("logprobs"));
    }

    #[test]
    fn test_completion_choice_serialization_minimal() {
        let choice = CompletionChoice {
            index: 0,
            text: "".to_string(),
            finish_reason: None,
            logprobs: None,
        };
        let json = serde_json::to_string(&choice).unwrap();
        assert!(!json.contains("finish_reason"));
        assert!(!json.contains("logprobs"));
    }

    #[test]
    fn test_completion_choice_deserialization() {
        let json = r#"{"index": 2, "text": "Parsed text", "finish_reason": "length"}"#;
        let choice: CompletionChoice = serde_json::from_str(json).unwrap();
        assert_eq!(choice.index, 2);
        assert_eq!(choice.text, "Parsed text");
        assert_eq!(choice.finish_reason, Some(FinishReason::Length));
    }

    #[test]
    fn test_completion_choice_deserialization_minimal() {
        let json = r#"{"index": 0, "text": "Only required fields"}"#;
        let choice: CompletionChoice = serde_json::from_str(json).unwrap();
        assert_eq!(choice.index, 0);
        assert!(choice.finish_reason.is_none());
        assert!(choice.logprobs.is_none());
    }

    // ==================== Clone and Debug Tests ====================

    #[test]
    fn test_completion_response_clone() {
        let response = CompletionResponse {
            id: "cmpl-clone".to_string(),
            object: "text_completion".to_string(),
            created: 1700000000,
            model: "model".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                text: "Cloned".to_string(),
                finish_reason: None,
                logprobs: None,
            }],
            usage: None,
            system_fingerprint: None,
        };
        let cloned = response.clone();
        assert_eq!(cloned.id, "cmpl-clone");
        assert_eq!(cloned.choices.len(), 1);
    }

    #[test]
    fn test_completion_choice_clone() {
        let choice = CompletionChoice {
            index: 0,
            text: "Clone test".to_string(),
            finish_reason: Some(FinishReason::Stop),
            logprobs: None,
        };
        let cloned = choice.clone();
        assert_eq!(cloned.text, "Clone test");
        assert_eq!(cloned.finish_reason, Some(FinishReason::Stop));
    }

    #[test]
    fn test_completion_response_debug() {
        let response = CompletionResponse {
            id: "cmpl-debug".to_string(),
            object: "text_completion".to_string(),
            created: 1700000000,
            model: "model".to_string(),
            choices: vec![],
            usage: None,
            system_fingerprint: None,
        };
        let debug = format!("{:?}", response);
        assert!(debug.contains("CompletionResponse"));
        assert!(debug.contains("cmpl-debug"));
    }

    #[test]
    fn test_completion_choice_debug() {
        let choice = CompletionChoice {
            index: 0,
            text: "Debug".to_string(),
            finish_reason: None,
            logprobs: None,
        };
        let debug = format!("{:?}", choice);
        assert!(debug.contains("CompletionChoice"));
    }

    // ==================== Edge Cases ====================

    #[test]
    fn test_completion_response_empty_id() {
        let response = CompletionResponse {
            id: "".to_string(),
            object: "text_completion".to_string(),
            created: 0,
            model: "".to_string(),
            choices: vec![],
            usage: None,
            system_fingerprint: None,
        };
        assert!(response.id.is_empty());
        assert_eq!(response.created, 0);
    }

    #[test]
    fn test_completion_choice_empty_text() {
        let choice = CompletionChoice {
            index: 0,
            text: "".to_string(),
            finish_reason: Some(FinishReason::Stop),
            logprobs: None,
        };
        assert!(choice.text.is_empty());
    }

    #[test]
    fn test_completion_response_multiple_choices() {
        let response = CompletionResponse {
            id: "cmpl-multi".to_string(),
            object: "text_completion".to_string(),
            created: 1700000000,
            model: "model".to_string(),
            choices: vec![
                CompletionChoice {
                    index: 0,
                    text: "First".to_string(),
                    finish_reason: Some(FinishReason::Stop),
                    logprobs: None,
                },
                CompletionChoice {
                    index: 1,
                    text: "Second".to_string(),
                    finish_reason: Some(FinishReason::Stop),
                    logprobs: None,
                },
                CompletionChoice {
                    index: 2,
                    text: "Third".to_string(),
                    finish_reason: Some(FinishReason::Length),
                    logprobs: None,
                },
            ],
            usage: None,
            system_fingerprint: None,
        };
        assert_eq!(response.choices.len(), 3);
        assert_eq!(
            response.choices[2].finish_reason,
            Some(FinishReason::Length)
        );
    }

    #[test]
    fn test_completion_response_roundtrip() {
        let original = CompletionResponse {
            id: "cmpl-roundtrip".to_string(),
            object: "text_completion".to_string(),
            created: 1700000000,
            model: "gpt-3.5-turbo-instruct".to_string(),
            choices: vec![CompletionChoice {
                index: 0,
                text: "Roundtrip test".to_string(),
                finish_reason: Some(FinishReason::Stop),
                logprobs: None,
            }],
            usage: Some(Usage {
                prompt_tokens: 5,
                completion_tokens: 10,
                total_tokens: 15,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
                cost: None,
            }),
            system_fingerprint: Some("fp_test".to_string()),
        };
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: CompletionResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, original.id);
        assert_eq!(deserialized.choices.len(), original.choices.len());
        assert!(deserialized.usage.is_some());
    }

    #[test]
    fn test_completion_choice_large_index() {
        let choice = CompletionChoice {
            index: u32::MAX,
            text: "Large index".to_string(),
            finish_reason: None,
            logprobs: None,
        };
        assert_eq!(choice.index, u32::MAX);
    }

    #[test]
    fn test_completion_response_negative_timestamp() {
        let response = CompletionResponse {
            id: "cmpl-neg".to_string(),
            object: "text_completion".to_string(),
            created: -1000,
            model: "model".to_string(),
            choices: vec![],
            usage: None,
            system_fingerprint: None,
        };
        assert_eq!(response.created, -1000);
    }
}
