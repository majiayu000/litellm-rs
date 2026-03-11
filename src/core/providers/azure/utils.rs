//! Azure OpenAI Utilities
//!
//! Utility functions for Azure OpenAI Service

use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;

use super::config::{AzureConfig, AzureModelInfo};
use super::error::{azure_config_error, azure_header_error};
use crate::core::providers::unified_provider::ProviderError;

/// Azure endpoint types
#[derive(Debug, Clone, PartialEq)]
pub enum AzureEndpointType {
    ChatCompletions,
    Completions,
    Embeddings,
    Images,
    ImageEdits,
    ImageVariations,
    AudioSpeech,
    AudioTranscriptions,
    AudioTranslations,
    Files,
    FineTuning,
    Models,
}

/// Azure OpenAI utilities
pub struct AzureUtils;

impl AzureUtils {
    /// Build Azure OpenAI URL
    pub fn build_azure_url(
        azure_endpoint: &str,
        deployment_name: &str,
        api_version: &str,
        endpoint_type: AzureEndpointType,
    ) -> String {
        let base = azure_endpoint.trim_end_matches('/');
        let endpoint_path = match endpoint_type {
            AzureEndpointType::ChatCompletions => "chat/completions",
            AzureEndpointType::Completions => "completions",
            AzureEndpointType::Embeddings => "embeddings",
            AzureEndpointType::Images => "images/generations",
            AzureEndpointType::ImageEdits => "images/edits",
            AzureEndpointType::ImageVariations => "images/variations",
            AzureEndpointType::AudioSpeech => "audio/speech",
            AzureEndpointType::AudioTranscriptions => "audio/transcriptions",
            AzureEndpointType::AudioTranslations => "audio/translations",
            AzureEndpointType::Files => "files",
            AzureEndpointType::FineTuning => "fine_tuning/jobs",
            AzureEndpointType::Models => "models",
        };

        format!(
            "{}/openai/deployments/{}/{}?api-version={}",
            base, deployment_name, endpoint_path, api_version
        )
    }

    /// Process Azure headers to OpenAI format
    pub fn process_azure_headers(headers: &HeaderMap) -> HashMap<String, String> {
        let mut openai_headers = HashMap::new();

        // Rate limit headers
        if let Some(limit) = headers.get("x-ratelimit-limit-requests")
            && let Ok(value) = limit.to_str()
        {
            openai_headers.insert("x-ratelimit-limit-requests".to_string(), value.to_string());
        }

        if let Some(remaining) = headers.get("x-ratelimit-remaining-requests")
            && let Ok(value) = remaining.to_str()
        {
            openai_headers.insert(
                "x-ratelimit-remaining-requests".to_string(),
                value.to_string(),
            );
        }

        if let Some(reset) = headers.get("x-ratelimit-reset-requests")
            && let Ok(value) = reset.to_str()
        {
            openai_headers.insert("x-ratelimit-reset-requests".to_string(), value.to_string());
        }

        // Token rate limit headers
        if let Some(limit) = headers.get("x-ratelimit-limit-tokens")
            && let Ok(value) = limit.to_str()
        {
            openai_headers.insert("x-ratelimit-limit-tokens".to_string(), value.to_string());
        }

        if let Some(remaining) = headers.get("x-ratelimit-remaining-tokens")
            && let Ok(value) = remaining.to_str()
        {
            openai_headers.insert(
                "x-ratelimit-remaining-tokens".to_string(),
                value.to_string(),
            );
        }

        if let Some(reset) = headers.get("x-ratelimit-reset-tokens")
            && let Ok(value) = reset.to_str()
        {
            openai_headers.insert("x-ratelimit-reset-tokens".to_string(), value.to_string());
        }

        // Azure specific headers
        if let Some(request_id) = headers.get("x-request-id")
            && let Ok(value) = request_id.to_str()
        {
            openai_headers.insert("x-request-id".to_string(), value.to_string());
        }

        openai_headers
    }

    /// Create Azure request headers
    pub fn create_azure_headers(
        config: &AzureConfig,
        api_key: &str,
    ) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();

        // API key header (Azure uses api-key header, not Authorization)
        headers.insert(
            "api-key",
            HeaderValue::from_str(api_key)
                .map_err(|e| azure_header_error(format!("Invalid API key: {}", e)))?,
        );

        // Content type
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );

        // User agent
        headers.insert(
            HeaderName::from_static("user-agent"),
            HeaderValue::from_static("litellm-rust/1.0.0"),
        );

        // Add custom headers
        for (key, value) in &config.custom_headers {
            let header_name = HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| azure_header_error(format!("Invalid header name {}: {}", key, e)))?;
            let header_value = HeaderValue::from_str(value).map_err(|e| {
                azure_header_error(format!("Invalid header value for {}: {}", key, e))
            })?;
            headers.insert(header_name, header_value);
        }

        Ok(headers)
    }

    /// Validate Azure configuration
    pub fn validate_config(config: &AzureConfig) -> Result<(), ProviderError> {
        if config.get_effective_azure_endpoint().is_none() {
            return Err(azure_config_error("Azure endpoint is required"));
        }

        if config.api_version.is_empty() {
            return Err(azure_config_error("API version is required"));
        }

        Ok(())
    }

    /// Extract deployment name from model
    pub fn extract_deployment_from_model(model: &str) -> Option<String> {
        // Handle model names like "azure/gpt-4" or direct deployment names
        if let Some(stripped) = model.strip_prefix("azure/") {
            Some(stripped.to_string())
        } else if model.contains('/') {
            // Skip provider prefix
            model.split('/').next_back().map(|s| s.to_string())
        } else {
            // Use model name directly as deployment
            Some(model.to_string())
        }
    }

    /// Get model info from Azure deployment
    pub fn get_model_info_from_deployment(deployment_name: &str) -> AzureModelInfo {
        AzureModelInfo {
            deployment_name: deployment_name.to_string(),
            model_name: Self::infer_model_from_deployment(deployment_name),
            max_tokens: Self::get_max_tokens_for_model(deployment_name),
            supports_functions: Self::supports_functions(deployment_name),
            supports_streaming: true,
            api_version: "2024-02-01".to_string(),
        }
    }

    /// Infer base model from deployment name
    fn infer_model_from_deployment(deployment: &str) -> String {
        let lower = deployment.to_lowercase();

        if lower.contains("gpt-4") {
            if lower.contains("vision") || lower.contains("v") {
                "gpt-4-vision-preview".to_string()
            } else if lower.contains("turbo") || lower.contains("1106") {
                "gpt-4-1106-preview".to_string()
            } else {
                "gpt-4".to_string()
            }
        } else if lower.contains("gpt-35-turbo") || lower.contains("gpt-3.5-turbo") {
            if lower.contains("1106") {
                "gpt-3.5-turbo-1106".to_string()
            } else if lower.contains("instruct") {
                "gpt-3.5-turbo-instruct".to_string()
            } else {
                "gpt-3.5-turbo".to_string()
            }
        } else if lower.contains("text-embedding") {
            if lower.contains("ada-002") {
                "text-embedding-ada-002".to_string()
            } else {
                "text-embedding-3-small".to_string()
            }
        } else if lower.contains("dall-e") {
            if lower.contains("3") {
                "dall-e-3".to_string()
            } else {
                "dall-e-2".to_string()
            }
        } else {
            deployment.to_string()
        }
    }

    /// Get maximum tokens for model
    fn get_max_tokens_for_model(deployment: &str) -> Option<u32> {
        let lower = deployment.to_lowercase();

        if lower.contains("gpt-4") {
            if lower.contains("32k") {
                Some(32768)
            } else if lower.contains("turbo") || lower.contains("1106") {
                Some(128000)
            } else {
                Some(8192)
            }
        } else if lower.contains("gpt-35-turbo") || lower.contains("gpt-3.5-turbo") {
            if lower.contains("16k") {
                Some(16384)
            } else if lower.contains("1106") {
                Some(16385)
            } else {
                Some(4096)
            }
        } else {
            None
        }
    }

    /// Check if model supports function calling
    fn supports_functions(deployment: &str) -> bool {
        let lower = deployment.to_lowercase();

        lower.contains("gpt-4") || lower.contains("gpt-35-turbo") || lower.contains("gpt-3.5-turbo")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_azure_url_chat_completions() {
        let url = AzureUtils::build_azure_url(
            "https://test.openai.azure.com",
            "gpt-4-deployment",
            "2024-02-01",
            AzureEndpointType::ChatCompletions,
        );
        assert_eq!(
            url,
            "https://test.openai.azure.com/openai/deployments/gpt-4-deployment/chat/completions?api-version=2024-02-01"
        );
    }

    #[test]
    fn test_build_azure_url_completions() {
        let url = AzureUtils::build_azure_url(
            "https://test.openai.azure.com",
            "gpt-4-deployment",
            "2024-02-01",
            AzureEndpointType::Completions,
        );
        assert!(url.contains("/completions?"));
        assert!(!url.contains("/chat/"));
    }

    #[test]
    fn test_build_azure_url_embeddings() {
        let url = AzureUtils::build_azure_url(
            "https://test.openai.azure.com",
            "ada-embedding",
            "2024-02-01",
            AzureEndpointType::Embeddings,
        );
        assert!(url.contains("/embeddings?"));
    }

    #[test]
    fn test_build_azure_url_images() {
        let url = AzureUtils::build_azure_url(
            "https://test.openai.azure.com",
            "dalle-3",
            "2024-02-01",
            AzureEndpointType::Images,
        );
        assert!(url.contains("/images/generations?"));
    }

    #[test]
    fn test_build_azure_url_audio() {
        let url = AzureUtils::build_azure_url(
            "https://test.openai.azure.com",
            "whisper",
            "2024-02-01",
            AzureEndpointType::AudioTranscriptions,
        );
        assert!(url.contains("/audio/transcriptions?"));
    }

    #[test]
    fn test_build_azure_url_trailing_slash() {
        let url = AzureUtils::build_azure_url(
            "https://test.openai.azure.com/",
            "gpt-4",
            "2024-02-01",
            AzureEndpointType::ChatCompletions,
        );
        // Should not have double slashes
        assert!(!url.contains("//openai"));
    }

    #[test]
    fn test_create_azure_headers() {
        let config = AzureConfig::new();
        let headers = AzureUtils::create_azure_headers(&config, "test-api-key").unwrap();

        assert_eq!(headers.get("api-key").unwrap(), "test-api-key");
        assert_eq!(headers.get("content-type").unwrap(), "application/json");
        assert_eq!(headers.get("user-agent").unwrap(), "litellm-rust/1.0.0");
    }

    #[test]
    fn test_create_azure_headers_with_custom_headers() {
        let mut config = AzureConfig::new();
        config
            .custom_headers
            .insert("x-custom-header".to_string(), "custom-value".to_string());

        let headers = AzureUtils::create_azure_headers(&config, "test-api-key").unwrap();

        assert_eq!(headers.get("x-custom-header").unwrap(), "custom-value");
    }

    #[test]
    fn test_validate_config_missing_endpoint() {
        let config = AzureConfig::new();
        let result = AzureUtils::validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_empty_api_version() {
        let mut config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
        config.api_version = String::new();

        let result = AzureUtils::validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_success() {
        let config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());

        let result = AzureUtils::validate_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_extract_deployment_from_model_azure_prefix() {
        let deployment = AzureUtils::extract_deployment_from_model("azure/gpt-4");
        assert_eq!(deployment, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_extract_deployment_from_model_other_prefix() {
        let deployment = AzureUtils::extract_deployment_from_model("openai/gpt-4");
        assert_eq!(deployment, Some("gpt-4".to_string()));
    }

    #[test]
    fn test_extract_deployment_from_model_no_prefix() {
        let deployment = AzureUtils::extract_deployment_from_model("gpt-4-deployment");
        assert_eq!(deployment, Some("gpt-4-deployment".to_string()));
    }

    #[test]
    fn test_get_model_info_from_deployment_gpt4() {
        let info = AzureUtils::get_model_info_from_deployment("gpt-4");
        assert_eq!(info.deployment_name, "gpt-4");
        assert_eq!(info.model_name, "gpt-4");
        assert!(info.supports_functions);
        assert!(info.supports_streaming);
        assert_eq!(info.max_tokens, Some(8192));
    }

    #[test]
    fn test_get_model_info_from_deployment_gpt4_turbo() {
        let info = AzureUtils::get_model_info_from_deployment("gpt-4-turbo-1106");
        assert_eq!(info.model_name, "gpt-4-1106-preview");
        assert_eq!(info.max_tokens, Some(128000));
    }

    #[test]
    fn test_get_model_info_from_deployment_gpt4_32k() {
        let info = AzureUtils::get_model_info_from_deployment("gpt-4-32k");
        assert_eq!(info.max_tokens, Some(32768));
    }

    #[test]
    fn test_get_model_info_from_deployment_gpt35() {
        let info = AzureUtils::get_model_info_from_deployment("gpt-35-turbo");
        assert_eq!(info.model_name, "gpt-3.5-turbo");
        assert!(info.supports_functions);
        assert_eq!(info.max_tokens, Some(4096));
    }

    #[test]
    fn test_get_model_info_from_deployment_gpt35_16k() {
        let info = AzureUtils::get_model_info_from_deployment("gpt-35-turbo-16k");
        assert_eq!(info.max_tokens, Some(16384));
    }

    #[test]
    fn test_get_model_info_from_deployment_gpt35_1106() {
        let info = AzureUtils::get_model_info_from_deployment("gpt-35-turbo-1106");
        assert_eq!(info.model_name, "gpt-3.5-turbo-1106");
        assert_eq!(info.max_tokens, Some(16385));
    }

    #[test]
    fn test_get_model_info_from_deployment_embedding() {
        let info = AzureUtils::get_model_info_from_deployment("text-embedding-ada-002");
        assert_eq!(info.model_name, "text-embedding-ada-002");
        assert!(info.max_tokens.is_none());
    }

    #[test]
    fn test_get_model_info_from_deployment_dalle() {
        let info = AzureUtils::get_model_info_from_deployment("dall-e-3");
        assert_eq!(info.model_name, "dall-e-3");

        let info2 = AzureUtils::get_model_info_from_deployment("dall-e-2");
        assert_eq!(info2.model_name, "dall-e-2");
    }

    #[test]
    fn test_get_model_info_from_deployment_unknown() {
        let info = AzureUtils::get_model_info_from_deployment("custom-deployment");
        assert_eq!(info.deployment_name, "custom-deployment");
        assert_eq!(info.model_name, "custom-deployment");
        assert!(info.max_tokens.is_none());
    }

    #[test]
    fn test_process_azure_headers_rate_limits() {
        let mut headers = HeaderMap::new();
        headers.insert("x-ratelimit-limit-requests", "100".parse().unwrap());
        headers.insert("x-ratelimit-remaining-requests", "90".parse().unwrap());
        headers.insert("x-ratelimit-reset-requests", "60".parse().unwrap());
        headers.insert("x-ratelimit-limit-tokens", "10000".parse().unwrap());
        headers.insert("x-ratelimit-remaining-tokens", "9000".parse().unwrap());
        headers.insert("x-ratelimit-reset-tokens", "30".parse().unwrap());
        headers.insert("x-request-id", "abc-123".parse().unwrap());

        let processed = AzureUtils::process_azure_headers(&headers);

        assert_eq!(
            processed.get("x-ratelimit-limit-requests"),
            Some(&"100".to_string())
        );
        assert_eq!(
            processed.get("x-ratelimit-remaining-requests"),
            Some(&"90".to_string())
        );
        assert_eq!(
            processed.get("x-ratelimit-limit-tokens"),
            Some(&"10000".to_string())
        );
        assert_eq!(processed.get("x-request-id"), Some(&"abc-123".to_string()));
    }

    #[test]
    fn test_process_azure_headers_empty() {
        let headers = HeaderMap::new();
        let processed = AzureUtils::process_azure_headers(&headers);
        assert!(processed.is_empty());
    }

    #[test]
    fn test_endpoint_type_equality() {
        assert_eq!(
            AzureEndpointType::ChatCompletions,
            AzureEndpointType::ChatCompletions
        );
        assert_ne!(
            AzureEndpointType::ChatCompletions,
            AzureEndpointType::Completions
        );
    }

    #[test]
    fn test_all_endpoint_types() {
        let endpoints = vec![
            (AzureEndpointType::ChatCompletions, "chat/completions"),
            (AzureEndpointType::Completions, "completions"),
            (AzureEndpointType::Embeddings, "embeddings"),
            (AzureEndpointType::Images, "images/generations"),
            (AzureEndpointType::ImageEdits, "images/edits"),
            (AzureEndpointType::ImageVariations, "images/variations"),
            (AzureEndpointType::AudioSpeech, "audio/speech"),
            (
                AzureEndpointType::AudioTranscriptions,
                "audio/transcriptions",
            ),
            (AzureEndpointType::AudioTranslations, "audio/translations"),
            (AzureEndpointType::Files, "files"),
            (AzureEndpointType::FineTuning, "fine_tuning/jobs"),
            (AzureEndpointType::Models, "models"),
        ];

        for (endpoint_type, expected_path) in endpoints {
            let url = AzureUtils::build_azure_url(
                "https://test.openai.azure.com",
                "deployment",
                "2024-02-01",
                endpoint_type,
            );
            assert!(
                url.contains(expected_path),
                "Expected {} to contain {}",
                url,
                expected_path
            );
        }
    }
}
