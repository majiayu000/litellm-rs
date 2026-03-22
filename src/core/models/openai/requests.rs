//! Request types for OpenAI-compatible API
//!
//! This module defines request structures for chat completions, text completions,
//! embeddings, and image generation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::audio::AudioParams;
use super::messages::ChatMessage;
use super::tools::{Function, FunctionCall, Tool, ToolChoice};

/// Chat completion request (OpenAI compatible)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    /// Model to use for completion
    pub model: String,
    /// List of messages
    pub messages: Vec<ChatMessage>,
    /// Temperature (0.0 to 2.0)
    pub temperature: Option<f32>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Maximum completion tokens (newer parameter)
    pub max_completion_tokens: Option<u32>,
    /// Top-p sampling
    pub top_p: Option<f32>,
    /// Number of completions to generate
    pub n: Option<u32>,
    /// Whether to stream the response
    pub stream: Option<bool>,
    /// Stream options
    pub stream_options: Option<StreamOptions>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
    /// Presence penalty
    pub presence_penalty: Option<f32>,
    /// Frequency penalty
    pub frequency_penalty: Option<f32>,
    /// Logit bias
    pub logit_bias: Option<HashMap<String, f32>>,
    /// User identifier
    pub user: Option<String>,
    /// Function calling (legacy)
    pub functions: Option<Vec<Function>>,
    /// Function call (legacy)
    pub function_call: Option<FunctionCall>,
    /// Tools for function calling
    pub tools: Option<Vec<Tool>>,
    /// Tool choice
    pub tool_choice: Option<ToolChoice>,
    /// Response format
    pub response_format: Option<ResponseFormat>,
    /// Seed for deterministic outputs
    pub seed: Option<u32>,
    /// Logprobs
    pub logprobs: Option<bool>,
    /// Top logprobs
    pub top_logprobs: Option<u32>,
    /// Modalities (for multimodal models)
    pub modalities: Option<Vec<String>>,
    /// Audio parameters
    pub audio: Option<AudioParams>,
    /// Reasoning effort for o-series and GPT-5.x models ("low", "medium", "high")
    pub reasoning_effort: Option<String>,
    /// Whether to store the response for model improvement (OpenAI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<bool>,
    /// Key-value metadata to attach to the request (OpenAI)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    /// Service tier for the request (OpenAI, e.g. "auto", "default", "flex")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_tier: Option<String>,
}

impl Default for ChatCompletionRequest {
    fn default() -> Self {
        Self {
            model: "gpt-3.5-turbo".to_string(),
            messages: vec![],
            temperature: None,
            max_tokens: None,
            max_completion_tokens: None,
            top_p: None,
            n: None,
            stream: None,
            stream_options: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            functions: None,
            function_call: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            seed: None,
            logprobs: None,
            top_logprobs: None,
            modalities: None,
            audio: None,
            reasoning_effort: None,
            store: None,
            metadata: None,
            service_tier: None,
        }
    }
}

/// Stream options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamOptions {
    /// Include usage in stream
    pub include_usage: Option<bool>,
}

/// Response format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseFormat {
    /// Format type
    #[serde(rename = "type")]
    pub format_type: String,
    /// JSON schema (for structured outputs)
    pub json_schema: Option<serde_json::Value>,
}

/// Text completion request (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// Model to use
    pub model: String,
    /// Prompt text
    pub prompt: String,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature
    pub temperature: Option<f64>,
    /// Top-p
    pub top_p: Option<f64>,
    /// Number of completions
    pub n: Option<u32>,
    /// Stream response
    pub stream: Option<bool>,
    /// Stop sequences
    pub stop: Option<Vec<String>>,
    /// Presence penalty
    pub presence_penalty: Option<f64>,
    /// Frequency penalty
    pub frequency_penalty: Option<f64>,
    /// Logit bias
    pub logit_bias: Option<HashMap<String, f64>>,
    /// User identifier
    pub user: Option<String>,
    /// Include the log probabilities
    pub logprobs: Option<u32>,
    /// Echo back the prompt
    pub echo: Option<bool>,
}

/// Embedding request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Model to use
    pub model: String,
    /// Input text or array of texts
    pub input: serde_json::Value,
    /// User identifier
    pub user: Option<String>,
}

/// Image generation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationRequest {
    /// Prompt for image generation
    pub prompt: String,
    /// Model to use
    pub model: Option<String>,
    /// Number of images
    pub n: Option<u32>,
    /// Image size
    pub size: Option<String>,
    /// Response format
    pub response_format: Option<String>,
    /// User identifier
    pub user: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== ChatCompletionRequest Tests ====================

    #[test]
    fn test_chat_completion_request_default() {
        let req = ChatCompletionRequest::default();
        assert_eq!(req.model, "gpt-3.5-turbo");
        assert!(req.messages.is_empty());
        assert!(req.temperature.is_none());
        assert!(req.max_tokens.is_none());
        assert!(req.stream.is_none());
    }

    #[test]
    fn test_chat_completion_request_with_model() {
        let req = ChatCompletionRequest {
            model: "gpt-4".to_string(),
            ..Default::default()
        };

        assert_eq!(req.model, "gpt-4");
    }

    #[test]
    fn test_chat_completion_request_with_temperature() {
        let req = ChatCompletionRequest {
            temperature: Some(0.7),
            ..Default::default()
        };

        assert_eq!(req.temperature, Some(0.7));
    }

    #[test]
    fn test_chat_completion_request_with_max_tokens() {
        let req = ChatCompletionRequest {
            max_tokens: Some(100),
            max_completion_tokens: Some(150),
            ..Default::default()
        };

        assert_eq!(req.max_tokens, Some(100));
        assert_eq!(req.max_completion_tokens, Some(150));
    }

    #[test]
    fn test_chat_completion_request_with_sampling() {
        let req = ChatCompletionRequest {
            top_p: Some(0.9),
            n: Some(3),
            ..Default::default()
        };

        assert_eq!(req.top_p, Some(0.9));
        assert_eq!(req.n, Some(3));
    }

    #[test]
    fn test_chat_completion_request_with_stream() {
        let req = ChatCompletionRequest {
            stream: Some(true),
            stream_options: Some(StreamOptions {
                include_usage: Some(true),
            }),
            ..Default::default()
        };

        assert_eq!(req.stream, Some(true));
        assert!(req.stream_options.is_some());
    }

    #[test]
    fn test_chat_completion_request_with_stop() {
        let req = ChatCompletionRequest {
            stop: Some(vec!["END".to_string(), "STOP".to_string()]),
            ..Default::default()
        };

        assert_eq!(req.stop.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_chat_completion_request_with_penalties() {
        let req = ChatCompletionRequest {
            presence_penalty: Some(0.5),
            frequency_penalty: Some(0.3),
            ..Default::default()
        };

        assert_eq!(req.presence_penalty, Some(0.5));
        assert_eq!(req.frequency_penalty, Some(0.3));
    }

    #[test]
    fn test_chat_completion_request_with_logit_bias() {
        let mut logit_bias = HashMap::new();
        logit_bias.insert("123".to_string(), -100.0);
        logit_bias.insert("456".to_string(), 50.0);

        let req = ChatCompletionRequest {
            logit_bias: Some(logit_bias),
            ..Default::default()
        };

        assert!(req.logit_bias.is_some());
        assert_eq!(req.logit_bias.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_chat_completion_request_with_user() {
        let req = ChatCompletionRequest {
            user: Some("user-123".to_string()),
            ..Default::default()
        };

        assert_eq!(req.user, Some("user-123".to_string()));
    }

    #[test]
    fn test_chat_completion_request_with_response_format() {
        let req = ChatCompletionRequest {
            response_format: Some(ResponseFormat {
                format_type: "json_object".to_string(),
                json_schema: None,
            }),
            ..Default::default()
        };

        assert!(req.response_format.is_some());
        assert_eq!(
            req.response_format.as_ref().unwrap().format_type,
            "json_object"
        );
    }

    #[test]
    fn test_chat_completion_request_with_seed() {
        let req = ChatCompletionRequest {
            seed: Some(42),
            ..Default::default()
        };

        assert_eq!(req.seed, Some(42));
    }

    #[test]
    fn test_chat_completion_request_with_logprobs() {
        let req = ChatCompletionRequest {
            logprobs: Some(true),
            top_logprobs: Some(5),
            ..Default::default()
        };

        assert_eq!(req.logprobs, Some(true));
        assert_eq!(req.top_logprobs, Some(5));
    }

    #[test]
    fn test_chat_completion_request_with_modalities() {
        let req = ChatCompletionRequest {
            modalities: Some(vec!["text".to_string(), "audio".to_string()]),
            ..Default::default()
        };

        assert_eq!(req.modalities.as_ref().unwrap().len(), 2);
    }

    #[test]
    fn test_chat_completion_request_serialize() {
        let req = ChatCompletionRequest {
            model: "gpt-4-turbo".to_string(),
            temperature: Some(0.8),
            max_tokens: Some(500),
            ..Default::default()
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("gpt-4-turbo"));
        assert!(json.contains("0.8"));
        assert!(json.contains("500"));
    }

    #[test]
    fn test_chat_completion_request_deserialize() {
        let json = r#"{"model":"gpt-4","messages":[],"temperature":0.5}"#;
        let req: ChatCompletionRequest = serde_json::from_str(json).unwrap();

        assert_eq!(req.model, "gpt-4");
        assert_eq!(req.temperature, Some(0.5));
    }

    #[test]
    fn test_chat_completion_request_clone() {
        let req = ChatCompletionRequest {
            model: "claude-3-opus".to_string(),
            temperature: Some(0.7),
            ..Default::default()
        };

        let cloned = req.clone();
        assert_eq!(req.model, cloned.model);
        assert_eq!(req.temperature, cloned.temperature);
    }

    // ==================== StreamOptions Tests ====================

    #[test]
    fn test_stream_options_with_usage() {
        let options = StreamOptions {
            include_usage: Some(true),
        };

        assert_eq!(options.include_usage, Some(true));
    }

    #[test]
    fn test_stream_options_without_usage() {
        let options = StreamOptions {
            include_usage: Some(false),
        };

        assert_eq!(options.include_usage, Some(false));
    }

    #[test]
    fn test_stream_options_serialize() {
        let options = StreamOptions {
            include_usage: Some(true),
        };

        let json = serde_json::to_string(&options).unwrap();
        assert!(json.contains("include_usage"));
        assert!(json.contains("true"));
    }

    // ==================== ResponseFormat Tests ====================

    #[test]
    fn test_response_format_text() {
        let format = ResponseFormat {
            format_type: "text".to_string(),
            json_schema: None,
        };

        assert_eq!(format.format_type, "text");
        assert!(format.json_schema.is_none());
    }

    #[test]
    fn test_response_format_json_object() {
        let format = ResponseFormat {
            format_type: "json_object".to_string(),
            json_schema: None,
        };

        assert_eq!(format.format_type, "json_object");
    }

    #[test]
    fn test_response_format_with_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "integer"}
            }
        });

        let format = ResponseFormat {
            format_type: "json_schema".to_string(),
            json_schema: Some(schema.clone()),
        };

        assert_eq!(format.format_type, "json_schema");
        assert!(format.json_schema.is_some());
    }

    #[test]
    fn test_response_format_serialize() {
        let format = ResponseFormat {
            format_type: "json_object".to_string(),
            json_schema: None,
        };

        let json = serde_json::to_string(&format).unwrap();
        assert!(json.contains("\"type\":\"json_object\""));
    }

    // ==================== CompletionRequest Tests ====================

    #[test]
    fn test_completion_request_creation() {
        let req = CompletionRequest {
            model: "gpt-3.5-turbo-instruct".to_string(),
            prompt: "Once upon a time".to_string(),
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            logprobs: None,
            echo: None,
        };

        assert_eq!(req.model, "gpt-3.5-turbo-instruct");
        assert_eq!(req.prompt, "Once upon a time");
        assert_eq!(req.max_tokens, Some(100));
    }

    #[test]
    fn test_completion_request_with_all_options() {
        let req = CompletionRequest {
            model: "text-davinci-003".to_string(),
            prompt: "Complete this:".to_string(),
            max_tokens: Some(50),
            temperature: Some(0.5),
            top_p: Some(0.9),
            n: Some(2),
            stream: Some(true),
            stop: Some(vec!["END".to_string()]),
            presence_penalty: Some(0.3),
            frequency_penalty: Some(0.2),
            logit_bias: None,
            user: Some("test-user".to_string()),
            logprobs: Some(3),
            echo: Some(true),
        };

        assert_eq!(req.n, Some(2));
        assert_eq!(req.stream, Some(true));
        assert_eq!(req.logprobs, Some(3));
        assert_eq!(req.echo, Some(true));
    }

    #[test]
    fn test_completion_request_serialize() {
        let req = CompletionRequest {
            model: "gpt-3.5-turbo-instruct".to_string(),
            prompt: "Hello".to_string(),
            max_tokens: Some(10),
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            logprobs: None,
            echo: None,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("gpt-3.5-turbo-instruct"));
        assert!(json.contains("Hello"));
    }

    // ==================== EmbeddingRequest Tests ====================

    #[test]
    fn test_embedding_request_single_input() {
        let req = EmbeddingRequest {
            model: "text-embedding-ada-002".to_string(),
            input: serde_json::json!("Hello world"),
            user: None,
        };

        assert_eq!(req.model, "text-embedding-ada-002");
        assert!(req.input.is_string());
    }

    #[test]
    fn test_embedding_request_array_input() {
        let req = EmbeddingRequest {
            model: "text-embedding-3-small".to_string(),
            input: serde_json::json!(["Hello", "World"]),
            user: Some("user-123".to_string()),
        };

        assert_eq!(req.model, "text-embedding-3-small");
        assert!(req.input.is_array());
        assert_eq!(req.user, Some("user-123".to_string()));
    }

    #[test]
    fn test_embedding_request_serialize() {
        let req = EmbeddingRequest {
            model: "text-embedding-3-large".to_string(),
            input: serde_json::json!("Test input"),
            user: None,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("text-embedding-3-large"));
        assert!(json.contains("Test input"));
    }

    // ==================== ImageGenerationRequest Tests ====================

    #[test]
    fn test_image_generation_request_minimal() {
        let req = ImageGenerationRequest {
            prompt: "A beautiful sunset".to_string(),
            model: None,
            n: None,
            size: None,
            response_format: None,
            user: None,
        };

        assert_eq!(req.prompt, "A beautiful sunset");
        assert!(req.model.is_none());
    }

    #[test]
    fn test_image_generation_request_full() {
        let req = ImageGenerationRequest {
            prompt: "A cat sitting on a chair".to_string(),
            model: Some("dall-e-3".to_string()),
            n: Some(2),
            size: Some("1024x1024".to_string()),
            response_format: Some("url".to_string()),
            user: Some("user-456".to_string()),
        };

        assert_eq!(req.model, Some("dall-e-3".to_string()));
        assert_eq!(req.n, Some(2));
        assert_eq!(req.size, Some("1024x1024".to_string()));
        assert_eq!(req.response_format, Some("url".to_string()));
    }

    #[test]
    fn test_image_generation_request_b64_format() {
        let req = ImageGenerationRequest {
            prompt: "Abstract art".to_string(),
            model: Some("dall-e-2".to_string()),
            n: Some(1),
            size: Some("512x512".to_string()),
            response_format: Some("b64_json".to_string()),
            user: None,
        };

        assert_eq!(req.response_format, Some("b64_json".to_string()));
    }

    #[test]
    fn test_image_generation_request_serialize() {
        let req = ImageGenerationRequest {
            prompt: "Mountain landscape".to_string(),
            model: Some("dall-e-3".to_string()),
            n: Some(1),
            size: Some("1792x1024".to_string()),
            response_format: None,
            user: None,
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("Mountain landscape"));
        assert!(json.contains("dall-e-3"));
        assert!(json.contains("1792x1024"));
    }

    #[test]
    fn test_image_generation_request_clone() {
        let req = ImageGenerationRequest {
            prompt: "Test image".to_string(),
            model: Some("dall-e-3".to_string()),
            n: Some(1),
            size: None,
            response_format: None,
            user: None,
        };

        let cloned = req.clone();
        assert_eq!(req.prompt, cloned.prompt);
        assert_eq!(req.model, cloned.model);
    }
}
