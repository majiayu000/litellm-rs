//! Shared HTTP client and canonical provider error mapping utilities for providers.

use std::collections::HashMap;
use std::time::Duration;

use reqwest::{IntoUrl, Method};
use serde_json::Value;

use crate::core::net::{ProviderEndpointAccess, ProviderEndpointPolicy};
use crate::core::providers::base::BaseConfig;
use crate::core::providers::base::connection_pool::HeaderPair;
use crate::core::providers::unified_provider::ProviderError;
use crate::utils::net::http::{ProviderHttpClient, ProviderRequestBuilder};

const ENDPOINT_POLICY_ERROR_MESSAGE: &str = "Provider endpoint rejected by SSRF protection";

/// Base HTTP client wrapper used by provider implementations.
#[derive(Debug, Clone)]
pub struct BaseHttpClient {
    client: ProviderHttpClient,
    config: BaseConfig,
    provider: &'static str,
}

#[derive(Clone, Copy)]
enum BaseRedirectMode {
    Policy,
    Disabled,
    Streaming,
    StreamingDisabled,
}

impl BaseHttpClient {
    /// Create a new HTTP client with common configuration.
    pub fn new(config: BaseConfig) -> Result<Self, ProviderError> {
        Self::new_for_provider("provider", config)
    }

    /// Create a policy-aware HTTP client for a named provider.
    pub fn new_for_provider(
        provider: &'static str,
        config: BaseConfig,
    ) -> Result<Self, ProviderError> {
        Self::build(provider, config, BaseRedirectMode::Policy)
    }

    /// Create a policy-aware client that never follows redirects.
    pub fn new_for_provider_no_redirect(
        provider: &'static str,
        config: BaseConfig,
    ) -> Result<Self, ProviderError> {
        Self::build(provider, config, BaseRedirectMode::Disabled)
    }

    /// Create a policy-aware client without a total response-lifetime timeout.
    pub fn new_for_provider_streaming(
        provider: &'static str,
        config: BaseConfig,
    ) -> Result<Self, ProviderError> {
        Self::build(provider, config, BaseRedirectMode::Streaming)
    }

    pub(crate) fn new_for_provider_streaming_no_redirect(
        provider: &'static str,
        config: BaseConfig,
    ) -> Result<Self, ProviderError> {
        Self::build(provider, config, BaseRedirectMode::StreamingDisabled)
    }

    fn build(
        provider: &'static str,
        config: BaseConfig,
        redirect_mode: BaseRedirectMode,
    ) -> Result<Self, ProviderError> {
        let policy = match config.api_base.as_deref() {
            Some(api_base) => {
                ProviderEndpointPolicy::for_base_url(config.endpoint_access, api_base).map_err(
                    |error| {
                        ProviderError::configuration(
                            provider,
                            format!("invalid provider API base: {error}"),
                        )
                    },
                )?
            }
            None if config.endpoint_access == ProviderEndpointAccess::PrivateNetwork => {
                return Err(ProviderError::configuration(
                    provider,
                    "private_network endpoint access requires an API base",
                ));
            }
            None => ProviderEndpointPolicy::public_only(),
        };
        let timeout = Duration::from_secs(config.timeout);
        let client_result = match redirect_mode {
            BaseRedirectMode::Policy => ProviderHttpClient::new(policy, timeout),
            BaseRedirectMode::Disabled => ProviderHttpClient::no_redirect(policy, timeout),
            BaseRedirectMode::Streaming => ProviderHttpClient::streaming(policy),
            BaseRedirectMode::StreamingDisabled => {
                ProviderHttpClient::streaming_no_redirect(policy)
            }
        };
        let client = client_result.map_err(|error| {
            ProviderError::initialization(
                provider,
                format!("failed to create policy-aware HTTP client: {error}"),
            )
        })?;
        Ok(Self {
            client,
            config,
            provider,
        })
    }

    /// Create a policy-checked request builder.
    pub fn request<U: IntoUrl>(
        &self,
        method: Method,
        url: U,
    ) -> Result<ProviderRequestBuilder, ProviderError> {
        self.client
            .request(method, url)
            .map_err(|error| ProviderError::network(self.provider, error.to_string()))
    }

    pub(crate) fn request_preserving_endpoint_policy<U: IntoUrl>(
        &self,
        method: Method,
        url: U,
    ) -> Result<ProviderRequestBuilder, ProviderError> {
        self.client.request(method, url).map_err(|error| {
            if error.is_endpoint_policy() {
                ProviderError::configuration(self.provider, ENDPOINT_POLICY_ERROR_MESSAGE)
            } else {
                ProviderError::network(self.provider, error.to_string())
            }
        })
    }

    pub(crate) fn map_preserved_request_error(&self, error: reqwest::Error) -> ProviderError {
        if ProviderHttpClient::request_error_is_endpoint_policy(&error) {
            ProviderError::configuration(self.provider, ENDPOINT_POLICY_ERROR_MESSAGE)
        } else if error.is_builder() {
            ProviderError::configuration(self.provider, "provider request could not be constructed")
        } else if error.is_timeout() {
            ProviderError::timeout(self.provider, "Provider request timed out")
        } else {
            ProviderError::network(self.provider, error.to_string())
        }
    }

    #[cfg(feature = "providers-extended")]
    pub(crate) fn request_error_may_have_been_dispatched(error: &reqwest::Error) -> bool {
        !ProviderHttpClient::request_error_is_endpoint_policy(error)
            && !error.is_builder()
            && !error.is_connect()
    }

    /// Create a policy-checked GET request builder.
    pub fn get<U: IntoUrl>(&self, url: U) -> Result<ProviderRequestBuilder, ProviderError> {
        self.request(Method::GET, url)
    }

    /// Create a policy-checked POST request builder.
    pub fn post<U: IntoUrl>(&self, url: U) -> Result<ProviderRequestBuilder, ProviderError> {
        self.request(Method::POST, url)
    }

    /// Get configuration.
    pub fn config(&self) -> &BaseConfig {
        &self.config
    }
}

/// Apply provider headers without exposing the policy-bound raw client.
#[inline]
pub fn apply_provider_headers(
    mut builder: ProviderRequestBuilder,
    headers: Vec<HeaderPair>,
) -> ProviderRequestBuilder {
    for (key, value) in headers {
        builder = builder.header(key.as_ref(), value.as_ref());
    }
    builder
}

/// Canonical HTTP status/body -> ProviderError mapper.
pub struct HttpErrorMapper;

impl HttpErrorMapper {
    /// Map HTTP status code to provider error.
    pub fn map_status_code(provider: &'static str, status: u16, body: &str) -> ProviderError {
        let message = extract_error_message(body).unwrap_or_else(|| body.to_string());

        match status {
            400 => {
                if is_context_length_error(body) {
                    ProviderError::context_length_exceeded(provider, 0, 0)
                } else if is_content_filter_error(body) {
                    ProviderError::content_filtered(provider, message, None, Some(false))
                } else {
                    ProviderError::invalid_request(provider, message)
                }
            }
            401 => ProviderError::authentication(provider, message),
            402 => ProviderError::quota_exceeded(provider, message),
            // Keep upstream 403 as an api_error so clients see permission
            // failure (403), not an authentication failure (401).
            403 => ProviderError::api_error(provider, status, message),
            404 => ProviderError::model_not_found(provider, message),
            408 | 504 => ProviderError::timeout(provider, message),
            413 => ProviderError::context_length_exceeded(provider, 0, 0),
            429 => ProviderError::rate_limit(
                provider,
                crate::core::providers::shared::parse_retry_after_from_body(body),
            ),
            502 | 503 => ProviderError::provider_unavailable(provider, message),
            500..=599 => ProviderError::api_error(provider, status, message),
            _ => ProviderError::api_error(provider, status, message),
        }
    }

    /// Map a known error status when its response body could not be read.
    #[cfg(feature = "providers-extended")]
    pub(crate) fn map_status_without_body(provider: &'static str, status: u16) -> ProviderError {
        Self::map_status_code(
            provider,
            status,
            &format!("Provider returned HTTP {status}, but its error body was unavailable"),
        )
    }

    /// Parse JSON error response.
    pub fn parse_json_error(provider: &'static str, json: &Value) -> ProviderError {
        let message = extract_error_message_from_json(json).unwrap_or("Unknown error");

        let error_type = json
            .get("error")
            .and_then(|e| e.get("type"))
            .and_then(|t| t.as_str())
            .or_else(|| json.get("type").and_then(|t| t.as_str()));
        let error_code = json
            .get("error")
            .and_then(|e| e.get("code"))
            .and_then(|c| c.as_str())
            .or_else(|| json.get("code").and_then(|c| c.as_str()));

        match (error_type, error_code) {
            (Some("authentication_error" | "permission_error"), _)
            | (_, Some("invalid_api_key" | "authentication_failed")) => {
                ProviderError::authentication(provider, message.to_string())
            }
            (Some("rate_limit_error"), _) | (_, Some("rate_limit_exceeded")) => {
                ProviderError::rate_limit(
                    provider,
                    json.get("retry_after")
                        .and_then(|v| v.as_u64())
                        .or_else(|| {
                            json.get("error")
                                .and_then(|e| e.get("retry_after"))
                                .and_then(|v| v.as_u64())
                        }),
                )
            }
            (Some("insufficient_quota"), _)
            | (_, Some("insufficient_quota" | "quota_exceeded")) => {
                ProviderError::quota_exceeded(provider, message.to_string())
            }
            (_, Some("model_not_found")) => ProviderError::model_not_found(provider, message),
            (Some("invalid_request_error" | "validation_error"), _) => {
                ProviderError::invalid_request(provider, message.to_string())
            }
            (Some("context_length_exceeded"), _) => {
                ProviderError::context_length_exceeded(provider, 0, 0)
            }
            (Some("content_filter" | "content_filter_error"), _) => {
                ProviderError::content_filtered(provider, message.to_string(), None, Some(false))
            }
            (Some("overloaded_error"), _) => {
                ProviderError::provider_unavailable(provider, message.to_string())
            }
            (Some("api_error" | "server_error"), _) => {
                ProviderError::api_error(provider, 500, message.to_string())
            }
            _ => ProviderError::api_error(provider, 500, message.to_string()),
        }
    }
}

fn extract_error_message(body: &str) -> Option<String> {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|json| extract_error_message_from_json(&json).map(str::to_string))
}

fn extract_error_message_from_json(json: &Value) -> Option<&str> {
    json.get("error")
        .and_then(|e| e.get("message"))
        .and_then(|m| m.as_str())
        .or_else(|| json.get("message").and_then(|m| m.as_str()))
        .or_else(|| json.get("detail").and_then(|m| m.as_str()))
        .or_else(|| json.get("error").and_then(|e| e.as_str()))
}

fn is_context_length_error(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("context_length")
        || lower.contains("context length")
        || lower.contains("maximum context")
        || lower.contains("too many tokens")
}

fn is_content_filter_error(body: &str) -> bool {
    let lower = body.to_ascii_lowercase();
    lower.contains("content_filter")
        || lower.contains("content filter")
        || lower.contains("safety")
        || lower.contains("policy")
}

/// Common URL builder.
pub struct UrlBuilder {
    base: String,
    path: String,
    query_params: HashMap<String, String>,
}

impl UrlBuilder {
    pub fn new(base: &str) -> Self {
        Self {
            base: base.trim_end_matches('/').to_string(),
            path: String::new(),
            query_params: HashMap::new(),
        }
    }

    pub fn with_path(mut self, path: &str) -> Self {
        self.path = path.trim_start_matches('/').to_string();
        self
    }

    pub fn with_query(mut self, key: &str, value: &str) -> Self {
        self.query_params.insert(key.to_string(), value.to_string());
        self
    }

    pub fn with_optional_query(mut self, key: &str, value: Option<&str>) -> Self {
        if let Some(v) = value {
            self.query_params.insert(key.to_string(), v.to_string());
        }
        self
    }

    pub fn build(self) -> String {
        let mut url = format!("{}/{}", self.base, self.path);

        if !self.query_params.is_empty() {
            let query_string: Vec<String> = self
                .query_params
                .iter()
                .map(|(k, v)| format!("{}={}", k, v.replace(" ", "%20")))
                .collect();

            url.push('?');
            url.push_str(&query_string.join("&"));
        }

        url
    }
}

/// Common request transformer for OpenAI-compatible APIs.
pub struct OpenAIRequestTransformer;

impl OpenAIRequestTransformer {
    pub fn transform_chat_request(request: &crate::core::types::chat::ChatRequest) -> Value {
        let mut body = serde_json::json!({
            "model": request.model,
            "messages": request.messages,
        });

        if let Some(temperature) = request.temperature {
            body["temperature"] = serde_json::json!(temperature);
        }
        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = serde_json::json!(max_tokens);
        }
        if let Some(top_p) = request.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        if let Some(frequency_penalty) = request.frequency_penalty {
            body["frequency_penalty"] = serde_json::json!(frequency_penalty);
        }
        if let Some(presence_penalty) = request.presence_penalty {
            body["presence_penalty"] = serde_json::json!(presence_penalty);
        }
        if let Some(stop) = &request.stop {
            body["stop"] = serde_json::json!(stop);
        }

        body["stream"] = serde_json::json!(request.stream);

        if let Some(user) = &request.user {
            body["user"] = serde_json::json!(user);
        }
        if let Some(tools) = &request.tools {
            body["tools"] = serde_json::json!(tools);
        }
        if let Some(tool_choice) = &request.tool_choice {
            body["tool_choice"] = serde_json::json!(tool_choice);
        }
        if let Some(response_format) = &request.response_format {
            body["response_format"] = serde_json::json!(response_format);
        }
        if let Some(seed) = request.seed {
            body["seed"] = serde_json::json!(seed);
        }

        body
    }
}

/// Common chat request validation helper.
pub fn validate_chat_request_common(
    provider: &'static str,
    request: &crate::core::types::chat::ChatRequest,
    max_output_tokens: u32,
) -> Result<(), ProviderError> {
    if request.messages.is_empty() {
        return Err(ProviderError::invalid_request(
            provider,
            "Messages cannot be empty",
        ));
    }

    if let Some(max_tokens) = request.max_tokens
        && max_tokens > max_output_tokens
    {
        return Err(ProviderError::invalid_request(
            provider,
            format!(
                "max_tokens {} exceeds model limit of {}",
                max_tokens, max_output_tokens
            ),
        ));
    }

    Ok(())
}

#[cfg(test)]
#[path = "http/network_policy_tests.rs"]
mod network_policy_tests;

#[cfg(test)]
#[path = "http/source_boundary_tests.rs"]
mod source_boundary_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[rustfmt::skip]
    #[test]
    fn endpoint_policy_preserving_opt_ins_are_gemini_only() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"); let (mut stack, mut callers) = (vec![root.clone()], Vec::new());
        while let Some(directory) = stack.pop() { for entry in std::fs::read_dir(directory).expect("provider sources must be readable") {
                let path = entry.expect("provider entry must be readable").path(); if path.is_dir() { stack.push(path); continue; }
                let name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();
                if path.extension().and_then(|ext| ext.to_str()) != Some("rs") || name == "tests.rs"
                    || name.ends_with("_tests.rs") || path == root.join("core/providers/base/connection_pool.rs") || path == root.join("core/providers/base/http.rs") { continue; }
                let source = std::fs::read_to_string(&path).expect("source must be readable");
                for method in ["execute_request_preserving_endpoint_policy", "execute_streaming_request_preserving_endpoint_policy"] {
                    if source.contains(method) { callers.push((path.strip_prefix(&root).unwrap_or(&path).to_path_buf(), method)); }
                }
            }
        }
        let expected = std::path::PathBuf::from("core/providers/openai_like/provider.rs"); assert_eq!(callers, vec![(expected.clone(), "execute_request_preserving_endpoint_policy"), (expected, "execute_streaming_request_preserving_endpoint_policy")]);
    }

    #[test]
    fn base_http_client_rejects_public_loopback_base() {
        let error = BaseHttpClient::new_for_provider(
            "test",
            BaseConfig {
                api_base: Some("http://127.0.0.1:11434/v1".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(matches!(error, ProviderError::Configuration { .. }));
        assert!(error.to_string().contains("SSRF protection"));
    }

    #[test]
    fn base_http_client_accepts_private_base_and_pins_authority() {
        let client = BaseHttpClient::new_for_provider(
            "test",
            BaseConfig {
                api_base: Some("http://127.0.0.1:11434/v1".to_string()),
                endpoint_access: ProviderEndpointAccess::PrivateNetwork,
                ..Default::default()
            },
        )
        .unwrap_or_else(|error| panic!("private test client should build: {error}"));

        let error = client
            .get("http://127.0.0.1:11435/v1/models")
            .err()
            .unwrap_or_else(|| panic!("cross-authority request must fail"));
        assert!(matches!(error, ProviderError::Network { .. }));
        assert!(error.to_string().contains("does not match"));
    }

    #[test]
    fn typed_request_preserves_direct_endpoint_policy_error() {
        let client = BaseHttpClient::new_for_provider(
            "test",
            BaseConfig {
                api_base: Some("https://api.example.com/v1".to_string()),
                ..Default::default()
            },
        )
        .expect("public test client should build");
        let error = match client
            .request_preserving_endpoint_policy(Method::GET, "ftp://api.example.com/v1")
        {
            Err(error) => error,
            Ok(_) => panic!("unsupported schemes must be rejected"),
        };

        assert!(matches!(error, ProviderError::Configuration { .. }));
    }

    #[test]
    fn base_http_client_requires_base_for_private_access() {
        let error = BaseHttpClient::new_for_provider(
            "test",
            BaseConfig {
                endpoint_access: ProviderEndpointAccess::PrivateNetwork,
                ..Default::default()
            },
        )
        .unwrap_err();

        assert!(matches!(error, ProviderError::Configuration { .. }));
        assert!(error.to_string().contains("requires an API base"));
    }

    #[test]
    fn http_mapper_extracts_json_message_for_invalid_request() {
        let err = HttpErrorMapper::map_status_code(
            "openai_like",
            400,
            r#"{"error":{"message":"bad prompt","type":"invalid_request_error"}}"#,
        );

        match err {
            ProviderError::InvalidRequest { provider, message } => {
                assert_eq!(provider, "openai_like");
                assert_eq!(message, "bad prompt");
            }
            other => panic!("expected invalid request, got {other:?}"),
        }
    }

    #[test]
    fn http_mapper_preserves_retry_after_for_rate_limits() {
        let err = HttpErrorMapper::map_status_code(
            "openai_like",
            429,
            r#"{"error":{"message":"slow down","retry_after":42}}"#,
        );

        match err {
            ProviderError::RateLimit {
                provider,
                retry_after,
                ..
            } => {
                assert_eq!(provider, "openai_like");
                assert_eq!(retry_after, Some(42));
            }
            other => panic!("expected rate limit, got {other:?}"),
        }
    }

    #[test]
    fn http_mapper_maps_special_statuses_to_specific_variants() {
        assert!(matches!(
            HttpErrorMapper::map_status_code("p", 408, "timeout"),
            ProviderError::Timeout { .. }
        ));
        assert!(matches!(
            HttpErrorMapper::map_status_code("p", 413, "too many tokens"),
            ProviderError::ContextLengthExceeded { .. }
        ));
        assert!(matches!(
            HttpErrorMapper::map_status_code("p", 503, "overloaded"),
            ProviderError::ProviderUnavailable { .. }
        ));
    }

    #[test]
    fn json_error_mapper_uses_type_and_code() {
        let rate = HttpErrorMapper::parse_json_error(
            "openai_like",
            &json!({
                "error": {
                    "type": "rate_limit_error",
                    "message": "slow down",
                    "retry_after": 17
                }
            }),
        );
        assert!(matches!(
            rate,
            ProviderError::RateLimit {
                retry_after: Some(17),
                ..
            }
        ));

        let not_found = HttpErrorMapper::parse_json_error(
            "openai_like",
            &json!({
                "error": {
                    "type": "invalid_request_error",
                    "code": "model_not_found",
                    "message": "missing model"
                }
            }),
        );
        assert!(matches!(not_found, ProviderError::ModelNotFound { .. }));
    }
}
