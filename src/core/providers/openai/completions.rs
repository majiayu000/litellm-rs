//! OpenAI Text Completions Module
//!
//! Legacy text completions API support following the unified architecture

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::image::CompletionRequest;
use crate::core::types::responses::{
    CompletionChoice, CompletionResponse, FinishReason, LogProbs, Usage,
};

/// OpenAI Text Completion request (legacy)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAICompletionRequest {
    /// ID of the model to use
    pub model: String,

    /// The prompt to generate completions for
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,

    /// The suffix that comes after a completion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,

    /// The maximum number of tokens to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    /// Sampling temperature to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Nucleus sampling parameter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// How many completions to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,

    /// Whether to stream back partial progress
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    /// Include the log probabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<u32>,

    /// Echo back the prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub echo: Option<bool>,

    /// Up to 4 sequences where the API will stop generating
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// Penalty for new tokens based on their frequency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,

    /// Penalty for new tokens based on their frequency
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,

    /// Generates best_of completions server-side
    #[serde(skip_serializing_if = "Option::is_none")]
    pub best_of: Option<u32>,

    /// Modify the likelihood of specified tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logit_bias: Option<HashMap<String, f32>>,

    /// A unique identifier representing your end-user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// OpenAI Completion Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAICompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAICompletionChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<OpenAICompletionUsage>,
}

/// OpenAI Completion Choice
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAICompletionChoice {
    pub text: String,
    pub index: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

/// OpenAI Completion Usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAICompletionUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Text completion model information
#[derive(Debug, Clone)]
pub struct CompletionModelInfo {
    pub id: String,
    pub max_tokens: u32,
    pub training_data_cutoff: &'static str,
    pub input_cost_per_1k: f64,
    pub output_cost_per_1k: f64,
}

/// Completion request transformer
pub struct OpenAICompletionTransformer;

impl OpenAICompletionTransformer {
    /// Transform unified CompletionRequest to OpenAI format
    pub fn transform_request(
        request: CompletionRequest,
    ) -> Result<OpenAICompletionRequest, ProviderError> {
        Ok(OpenAICompletionRequest {
            model: request.model,
            prompt: Some(request.prompt),
            suffix: None, // Not available in unified CompletionRequest
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            n: request.n,
            stream: Some(request.stream),
            logprobs: None, // Not available in unified CompletionRequest
            stop: request.stop,
            presence_penalty: request.presence_penalty,
            frequency_penalty: request.frequency_penalty,
            user: request.user,
            echo: None,       // Not available in unified CompletionRequest
            best_of: None,    // Not available in unified CompletionRequest
            logit_bias: None, // Not available in unified CompletionRequest
        })
    }

    /// Transform OpenAI response to unified format
    pub fn transform_response(
        response: OpenAICompletionResponse,
    ) -> Result<CompletionResponse, ProviderError> {
        let choices = response
            .choices
            .into_iter()
            .map(|choice| CompletionChoice {
                text: choice.text,
                index: choice.index,
                logprobs: choice.logprobs.map(|_lp| LogProbs {
                    content: Vec::new(), // Would need to transform from OpenAI format
                    refusal: None,
                }),
                finish_reason: choice.finish_reason.map(|fr| match fr.as_str() {
                    "stop" => FinishReason::Stop,
                    "length" => FinishReason::Length,
                    "content_filter" => FinishReason::ContentFilter,
                    "function_call" => FinishReason::FunctionCall,
                    "tool_calls" => FinishReason::ToolCalls,
                    _ => FinishReason::Stop,
                }),
            })
            .collect();

        Ok(CompletionResponse {
            id: response.id,
            object: response.object,
            created: response.created,
            model: response.model,
            choices,
            usage: response.usage.map(|usage| Usage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
                prompt_tokens_details: None,
                completion_tokens_details: None,
                thinking_usage: None,
            }),
            system_fingerprint: None, // Not available in OpenAI completions API
        })
    }
}

/// Get text completion model information
pub fn get_completion_models() -> Vec<CompletionModelInfo> {
    vec![
        CompletionModelInfo {
            id: "gpt-3.5-turbo-instruct".to_string(),
            max_tokens: 4096,
            training_data_cutoff: "Sep 2021",
            input_cost_per_1k: 0.0015,
            output_cost_per_1k: 0.002,
        },
        CompletionModelInfo {
            id: "babbage-002".to_string(),
            max_tokens: 16384,
            training_data_cutoff: "Sep 2021",
            input_cost_per_1k: 0.0004,
            output_cost_per_1k: 0.0004,
        },
        CompletionModelInfo {
            id: "davinci-002".to_string(),
            max_tokens: 16384,
            training_data_cutoff: "Sep 2021",
            input_cost_per_1k: 0.002,
            output_cost_per_1k: 0.002,
        },
    ]
}

/// Check if model supports text completions
pub fn is_completion_model(model_id: &str) -> bool {
    matches!(
        model_id,
        "gpt-3.5-turbo-instruct" | "babbage-002" | "davinci-002"
    )
}

/// Create simple completion request
pub fn create_simple_completion(
    model: impl Into<String>,
    prompt: impl Into<String>,
    max_tokens: Option<u32>,
) -> OpenAICompletionRequest {
    OpenAICompletionRequest {
        model: model.into(),
        prompt: Some(prompt.into()),
        suffix: None,
        max_tokens,
        temperature: Some(0.7),
        top_p: None,
        n: Some(1),
        stream: None,
        logprobs: None,
        echo: None,
        stop: None,
        presence_penalty: None,
        frequency_penalty: None,
        best_of: None,
        logit_bias: None,
        user: None,
    }
}

/// Validate completion request
pub fn validate_completion_request(request: &OpenAICompletionRequest) -> Result<(), ProviderError> {
    // Check if model supports completions
    if !is_completion_model(&request.model) {
        return Err(ProviderError::ModelNotFound {
            provider: "openai",
            model: request.model.clone(),
        });
    }

    // Check prompt
    if let Some(prompt) = &request.prompt
        && prompt.is_empty()
    {
        return Err(ProviderError::InvalidRequest {
            provider: "openai",
            message: "Prompt cannot be empty".to_string(),
        });
    }

    // Check parameters
    if let Some(n) = request.n
        && (n == 0 || n > 128)
    {
        return Err(ProviderError::InvalidRequest {
            provider: "openai",
            message: "n must be between 1 and 128".to_string(),
        });
    }

    if let Some(temp) = request.temperature
        && !(0.0..=2.0).contains(&temp)
    {
        return Err(ProviderError::InvalidRequest {
            provider: "openai",
            message: "temperature must be between 0.0 and 2.0".to_string(),
        });
    }

    if let Some(max_tokens) = request.max_tokens {
        let model_info = get_completion_models()
            .into_iter()
            .find(|m| m.id == request.model);

        if let Some(info) = model_info
            && max_tokens > info.max_tokens
        {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: format!("max_tokens exceeds model limit of {}", info.max_tokens),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_completion_model() {
        assert!(is_completion_model("gpt-3.5-turbo-instruct"));
        assert!(is_completion_model("babbage-002"));
        assert!(is_completion_model("davinci-002"));
        assert!(!is_completion_model("gpt-4"));
        assert!(!is_completion_model("gpt-3.5-turbo"));
    }

    #[test]
    fn test_create_simple_completion() {
        let request = create_simple_completion("gpt-3.5-turbo-instruct", "Hello world", Some(50));

        assert_eq!(request.model, "gpt-3.5-turbo-instruct");
        assert_eq!(request.prompt, Some("Hello world".to_string()));
        assert_eq!(request.max_tokens, Some(50));
        assert_eq!(request.temperature, Some(0.7));
        assert_eq!(request.n, Some(1));
    }

    #[test]
    fn test_validate_completion_request() {
        let valid_request = create_simple_completion("gpt-3.5-turbo-instruct", "Test", Some(100));
        assert!(validate_completion_request(&valid_request).is_ok());

        let invalid_model = create_simple_completion("gpt-4", "Test", Some(100));
        assert!(validate_completion_request(&invalid_model).is_err());

        let mut empty_prompt = valid_request.clone();
        empty_prompt.prompt = Some("".to_string());
        assert!(validate_completion_request(&empty_prompt).is_err());
    }

    #[test]
    fn test_transformation() {
        let completion_request = CompletionRequest {
            model: "gpt-3.5-turbo-instruct".to_string(),
            prompt: "Hello world".to_string(),
            max_tokens: Some(50),
            temperature: Some(0.8),
            top_p: None,
            n: Some(1),
            stream: false,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            user: None,
        };

        let openai_request =
            OpenAICompletionTransformer::transform_request(completion_request).unwrap();
        assert_eq!(openai_request.model, "gpt-3.5-turbo-instruct");
        assert_eq!(openai_request.prompt, Some("Hello world".to_string()));
        assert_eq!(openai_request.max_tokens, Some(50));
        assert_eq!(openai_request.temperature, Some(0.8));
    }
}
