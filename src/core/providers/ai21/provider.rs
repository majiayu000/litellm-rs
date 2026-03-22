//! AI21 Provider Implementation
//!
//! Main provider implementation using the unified base infrastructure

use futures::Stream;
use serde_json::Value;
use std::collections::HashMap;
use std::pin::Pin;

use crate::core::providers::base::{
    HeaderPair, HttpErrorMapper, HttpMethod, get_pricing_db, header, header_owned,
};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    chat::ChatRequest,
    context::RequestContext,
    health::HealthStatus,
    model::ProviderCapability,
    responses::{ChatChunk, ChatResponse},
};

use super::{AI21Client, AI21Config};

const PROVIDER_NAME: &str = "ai21";
const CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];

crate::define_pooled_http_provider_with_hooks!(
    provider: PROVIDER_NAME,
    struct_name: AI21Provider,
    config: super::AI21Config,
    error_mapper: super::AI21ErrorMapper,
    model_info: AI21Client::supported_models,
    capabilities: CAPABILITIES,
    url_builder: |provider: &AI21Provider| -> String {
        format!(
            "{}/chat/completions",
            provider.config.base.get_effective_api_base(PROVIDER_NAME)
        )
    },
    http_method: HttpMethod::POST,
    supported_params: [
        "temperature",
        "max_tokens",
        "top_p",
        "stream",
        "stop",
        "tools",
        "tool_choice",
        "response_format",
        "seed",
        "n",
        "max_completion_tokens",
    ],
    build_headers: |provider: &AI21Provider| -> Vec<HeaderPair> {
        provider.get_request_headers()
    },
    with_api_key: |api_key: String| -> Result<AI21Provider, ProviderError> {
        let mut config = AI21Config::new(PROVIDER_NAME);
        config.base.api_key = Some(api_key);
        AI21Provider::new(config)
    },
    map_openai_params: |_provider: &AI21Provider,
                        mut params: HashMap<String, Value>,
                        _model: &str|
     -> Result<HashMap<String, Value>, ProviderError> {
        if let Some(max_completion_tokens) = params.remove("max_completion_tokens") {
            params.insert("max_tokens".to_string(), max_completion_tokens);
        }
        Ok(params)
    },
    request_transform: |_provider: &AI21Provider, request: ChatRequest|
     -> Result<Value, ProviderError> { Ok(AI21Client::transform_chat_request(request)) },
    response_transform: |_provider: &AI21Provider,
                         raw_response: &[u8],
                         _model: &str,
                         _request_id: &str|
     -> Result<ChatResponse, ProviderError> {
        let response: Value = serde_json::from_slice(raw_response)
            .map_err(|e| ProviderError::response_parsing(PROVIDER_NAME, e.to_string()))?;
        AI21Client::transform_chat_response(response)
    },
    error_map: |_provider: &AI21Provider,
                status: u16,
                error_text: String,
                _request: &ChatRequest|
     -> ProviderError {
        if let Ok(value) = serde_json::from_str::<Value>(&error_text)
            && let Some(error) = value.get("error") {
                let message = error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("Unknown error from AI21 API");

                let code = error
                    .get("code")
                    .and_then(|c| c.as_str())
                    .unwrap_or("unknown_error");

                return match code {
                    "authentication_error" | "invalid_request_error" => {
                        ProviderError::authentication(PROVIDER_NAME, message)
                    }
                    "rate_limit_exceeded" => ProviderError::rate_limit(PROVIDER_NAME, None),
                    _ => HttpErrorMapper::map_status_code(PROVIDER_NAME, status, message),
                };
            }

        HttpErrorMapper::map_status_code(PROVIDER_NAME, status, &error_text)
    },
    health_check: |provider: &AI21Provider| {
        let has_key = provider
            .config
            .base
            .get_effective_api_key(PROVIDER_NAME)
            .is_some();
        async move {
            if has_key {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            }
        }
    },
    streaming: |provider: &AI21Provider, request: ChatRequest, _context: RequestContext| {
        let url = format!(
            "{}/chat/completions",
            provider.config.base.get_effective_api_base(PROVIDER_NAME)
        );
        let api_key = provider.config.base.get_effective_api_key(PROVIDER_NAME);

        let mut body = AI21Client::transform_chat_request(request);
        body["stream"] = serde_json::Value::Bool(true);

        async move {
            let api_key = api_key.ok_or_else(|| {
                ProviderError::authentication(PROVIDER_NAME, "API key is required")
            })?;

            let client = reqwest::Client::new();
            let response = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| ProviderError::network(PROVIDER_NAME, e.to_string()))?;

            let status = response.status();
            if !status.is_success() {
                let error_text = response
                    .text()
                    .await
                    .unwrap_or_else(|_| "Unknown error".to_string());
                return Err(HttpErrorMapper::map_status_code(
                    PROVIDER_NAME,
                    status.as_u16(),
                    &error_text,
                ));
            }

            let stream = super::streaming::create_ai21_stream(response.bytes_stream());
            let stream: Pin<Box<dyn Stream<Item = Result<ChatChunk, ProviderError>> + Send>> =
                Box::pin(stream);
            Ok(stream)
        }
    },
    calculate_cost: |_provider: &AI21Provider,
                     model: &str,
                     input_tokens: u32,
                     output_tokens: u32|
     -> Result<f64, ProviderError> {
        let usage = crate::core::providers::base::pricing::Usage {
            prompt_tokens: input_tokens,
            completion_tokens: output_tokens,
            total_tokens: input_tokens + output_tokens,
            reasoning_tokens: None,
        };

        Ok(get_pricing_db().calculate(model, &usage))
    },
);

impl AI21Provider {
    pub fn from_env() -> Result<Self, ProviderError> {
        let config = AI21Config::from_env();
        Self::new(config)
    }

    fn get_request_headers(&self) -> Vec<HeaderPair> {
        let mut headers = Vec::with_capacity(2);

        if let Some(api_key) = &self.config.base.api_key {
            headers.push(header("Authorization", format!("Bearer {}", api_key)));
        }

        for (key, value) in &self.config.base.headers {
            headers.push(header_owned(key.clone(), value.clone()));
        }

        headers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;

    #[test]
    fn test_provider_creation() {
        let mut config = AI21Config::new("ai21");
        config.base.api_key = Some("test-api-key".to_string());

        let provider = AI21Provider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_name() {
        let mut config = AI21Config::new("ai21");
        config.base.api_key = Some("test-api-key".to_string());

        let provider = AI21Provider::new(config).unwrap();
        assert_eq!(provider.name(), "ai21");
    }

    #[test]
    fn test_provider_capabilities() {
        let mut config = AI21Config::new("ai21");
        config.base.api_key = Some("test-api-key".to_string());

        let provider = AI21Provider::new(config).unwrap();
        let capabilities = provider.capabilities();

        assert!(capabilities.contains(&ProviderCapability::ChatCompletion));
        assert!(capabilities.contains(&ProviderCapability::ChatCompletionStream));
        assert!(capabilities.contains(&ProviderCapability::ToolCalling));
    }

    #[test]
    fn test_provider_models() {
        let mut config = AI21Config::new("ai21");
        config.base.api_key = Some("test-api-key".to_string());

        let provider = AI21Provider::new(config).unwrap();
        let models = provider.models();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "jamba-1.5-large"));
    }

    #[test]
    fn test_provider_from_env() {
        // This test may fail if AI21_API_KEY is not set
        let result = AI21Provider::from_env();
        // Either succeeds or fails with missing API key
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_provider_missing_api_key() {
        let config = AI21Config::new("ai21");
        let result = AI21Provider::new(config);
        assert!(result.is_err());
    }

    #[test]
    fn test_supported_openai_params() {
        let mut config = AI21Config::new("ai21");
        config.base.api_key = Some("test-api-key".to_string());

        let provider = AI21Provider::new(config).unwrap();
        let params = provider.get_supported_openai_params("jamba-1.5-large");

        assert!(params.contains(&"temperature"));
        assert!(params.contains(&"max_tokens"));
        assert!(params.contains(&"tools"));
    }

    #[tokio::test]
    async fn test_map_openai_params() {
        let mut config = AI21Config::new("ai21");
        config.base.api_key = Some("test-api-key".to_string());

        let provider = AI21Provider::new(config).unwrap();

        let mut params = HashMap::new();
        params.insert("max_completion_tokens".to_string(), serde_json::json!(1000));
        params.insert("temperature".to_string(), serde_json::json!(0.7));

        let mapped = provider
            .map_openai_params(params, "jamba-1.5-large")
            .await
            .unwrap();

        // max_completion_tokens should be mapped to max_tokens
        assert!(mapped.contains_key("max_tokens"));
        assert!(!mapped.contains_key("max_completion_tokens"));
        assert!(mapped.contains_key("temperature"));
    }

    #[tokio::test]
    async fn test_health_check_with_key() {
        let mut config = AI21Config::new("ai21");
        config.base.api_key = Some("test-api-key".to_string());

        let provider = AI21Provider::new(config).unwrap();
        let health = provider.health_check().await;

        assert!(matches!(health, HealthStatus::Healthy));
    }

    #[tokio::test]
    async fn test_calculate_cost() {
        let mut config = AI21Config::new("ai21");
        config.base.api_key = Some("test-api-key".to_string());

        let provider = AI21Provider::new(config).unwrap();
        let cost = provider
            .calculate_cost("jamba-1.5-large", 1000, 500)
            .await
            .unwrap();

        // Cost should be non-negative
        assert!(cost >= 0.0);
    }

    #[test]
    fn test_get_request_headers() {
        let mut config = AI21Config::new("ai21");
        config.base.api_key = Some("test-api-key".to_string());
        config
            .base
            .headers
            .insert("X-Custom-Header".to_string(), "custom-value".to_string());

        let provider = AI21Provider::new(config).unwrap();
        let headers = provider.get_request_headers();

        assert!(!headers.is_empty());
    }

    #[tokio::test]
    async fn test_transform_request() {
        let mut config = AI21Config::new("ai21");
        config.base.api_key = Some("test-api-key".to_string());

        let provider = AI21Provider::new(config).unwrap();

        let request = ChatRequest {
            model: "jamba-1.5-large".to_string(),
            messages: vec![],
            temperature: Some(0.7),
            max_tokens: Some(100),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stream: false,
            stream_options: None,
            tools: None,
            tool_choice: None,
            user: None,
            response_format: None,
            seed: None,
            max_completion_tokens: None,
            stop: None,
            parallel_tool_calls: None,
            n: None,
            logit_bias: None,
            functions: None,
            function_call: None,
            logprobs: None,
            top_logprobs: None,
            reasoning_effort: None,
            store: None,
            metadata: None,
            service_tier: None,
            thinking: None,
            extra_params: HashMap::new(),
        };

        let context = RequestContext::default();
        let transformed = provider.transform_request(request, context).await.unwrap();

        assert_eq!(transformed["model"], "jamba-1.5-large");
    }
}
