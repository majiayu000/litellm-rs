//! DeepL Translation Provider Implementation
//!
//! Note: DeepL is a translation service, not a chat completion model.
//! This provider adapts DeepL's translation API to the LLMProvider interface
//! by treating translation requests as special chat completions.

use serde::{Deserialize, Serialize};

use crate::core::types::message::{MessageContent, MessageRole};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeepLTranslateRequest {
    text: Vec<String>,
    target_lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_lang: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    formality: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeepLTranslation {
    detected_source_language: String,
    text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeepLTranslateResponse {
    translations: Vec<DeepLTranslation>,
}

crate::define_http_provider_with_hooks!(
    provider: super::PROVIDER_NAME,
    struct_name: DeepLProvider,
    config: super::config::DeepLConfig,
    error_mapper: super::error_mapper::DeepLErrorMapper,
    model_info: super::model_info::get_supported_models,
    capabilities: &[
        crate::core::types::model::ProviderCapability::ChatCompletion,
    ],
    url_builder: |provider: &DeepLProvider| -> String {
        let base_url = provider
            .config
            .base
            .api_base
            .as_deref()
            .unwrap_or(super::DEFAULT_BASE_URL);
        format!("{}/translate", base_url)
    },
    request_builder: |provider: &DeepLProvider, url: &str| -> reqwest::RequestBuilder {
        provider.http_client.post(url)
    },
    supported_params: ["temperature"],
    build_headers: |provider: &DeepLProvider, headers: &mut std::collections::HashMap<String, String>| {
        if let Some(api_key) = &provider.config.base.api_key {
            headers.insert(
                "Authorization".to_string(),
                format!("DeepL-Auth-Key {}", api_key),
            );
        }

        headers.insert("Content-Type".to_string(), "application/json".to_string());
    },
    with_api_key: |api_key: String| -> Result<DeepLProvider, crate::core::providers::unified_provider::ProviderError> {
        let config = super::config::DeepLConfig::new("deepl")
            .with_api_key(api_key);
        DeepLProvider::new(config)
    },
    request_transform: |provider: &DeepLProvider,
                        request: crate::core::types::chat::ChatRequest|
     -> Result<serde_json::Value, crate::core::providers::unified_provider::ProviderError> {
        let (target_lang, source_lang, text) = provider.extract_translation_params(&request)?;

        let translate_request = DeepLTranslateRequest {
            text: vec![text],
            target_lang,
            source_lang,
            formality: None, // Can be mapped from temperature or other params
        };

        serde_json::to_value(translate_request).map_err(|e| {
            crate::core::providers::unified_provider::ProviderError::serialization(
                "deepl",
                e.to_string(),
            )
        })
    },
    response_transform: |_provider: &DeepLProvider,
                         raw_response: &[u8],
                         _model: &str,
                         _request_id: &str|
     -> Result<crate::core::types::responses::ChatResponse, crate::core::providers::unified_provider::ProviderError> {
        let response_text = String::from_utf8_lossy(raw_response);
        let deepl_response: DeepLTranslateResponse =
            serde_json::from_str(&response_text).map_err(|e| {
                crate::core::providers::unified_provider::ProviderError::serialization(
                    "deepl",
                    e.to_string(),
                )
            })?;

        // Convert DeepL response to ChatResponse format
        let translation = deepl_response
            .translations
            .first()
            .ok_or_else(|| {
                crate::core::providers::unified_provider::ProviderError::api_error(
                    "deepl",
                    500,
                    "No translation returned",
                )
            })?;

        let response = serde_json::json!({
            "id": format!("deepl-{}", uuid::Uuid::new_v4()),
            "object": "chat.completion",
            "created": chrono::Utc::now().timestamp(),
            "model": "deepl-translate",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": translation.text.clone(),
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 0,
                "completion_tokens": 0,
                "total_tokens": 0
            }
        });

        serde_json::from_value(response).map_err(|e| {
            crate::core::providers::unified_provider::ProviderError::serialization(
                "deepl",
                e.to_string(),
            )
        })
    },
    error_map: |_provider: &DeepLProvider,
                status: u16,
                error_text: String,
                _request: &crate::core::types::chat::ChatRequest|
     -> crate::core::providers::unified_provider::ProviderError {
        match status {
            401 | 403 => crate::core::providers::unified_provider::ProviderError::authentication(
                "deepl",
                error_text,
            ),
            429 => crate::core::providers::unified_provider::ProviderError::rate_limit(
                "deepl",
                None,
            ),
            456 => crate::core::providers::unified_provider::ProviderError::quota_exceeded(
                "deepl",
                "Quota exceeded",
            ),
            400 => crate::core::providers::unified_provider::ProviderError::invalid_request(
                "deepl",
                error_text,
            ),
            _ => crate::core::providers::unified_provider::ProviderError::api_error(
                "deepl",
                status,
                error_text,
            ),
        }
    },
    health_check: |provider: &DeepLProvider| {
        let base_url = provider
            .config
            .base
            .api_base
            .as_deref()
            .unwrap_or(super::DEFAULT_BASE_URL)
            .to_string();
        let headers = provider.build_headers();
        let http_client = provider.http_client.clone();

        async move {
            let url = format!("{}/usage", base_url);
            let mut req_builder = http_client.get(&url);
            for (key, value) in headers {
                req_builder = req_builder.header(key, value);
            }

            match req_builder.send().await {
                Ok(response) if response.status().is_success() => {
                    crate::core::types::health::HealthStatus::Healthy
                }
                Ok(_) => crate::core::types::health::HealthStatus::Unhealthy,
                Err(_) => crate::core::types::health::HealthStatus::Unhealthy,
            }
        }
    },
    streaming_error: "Streaming is not supported for translation",
);

impl DeepLProvider {
    /// Extract translation parameters from chat request
    /// Expected message format: "Translate to {target_lang}: {text}"
    /// Or with source language: "Translate from {source_lang} to {target_lang}: {text}"
    fn extract_translation_params(
        &self,
        request: &crate::core::types::chat::ChatRequest,
    ) -> Result<
        (String, Option<String>, String),
        crate::core::providers::unified_provider::ProviderError,
    > {
        // Get the last user message
        let user_message = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .ok_or_else(|| {
                crate::core::providers::unified_provider::ProviderError::invalid_request(
                    "deepl",
                    "No user message found",
                )
            })?;

        let content = user_message.content.as_ref().ok_or_else(|| {
            crate::core::providers::unified_provider::ProviderError::invalid_request(
                "deepl",
                "Message content is empty",
            )
        })?;

        // Extract text from MessageContent
        let text = match content {
            MessageContent::Text(text_content) => text_content.as_str(),
            MessageContent::Parts(parts) => {
                // Find text part
                use crate::core::types::content::ContentPart;
                parts
                    .iter()
                    .find_map(|p| match p {
                        ContentPart::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .ok_or_else(|| {
                        crate::core::providers::unified_provider::ProviderError::invalid_request(
                            "deepl",
                            "No text content found in message",
                        )
                    })?
            }
        };

        // Parse translation instruction
        let parts: Vec<&str> = text.split(':').collect();
        if parts.len() < 2 {
            return Err(
                crate::core::providers::unified_provider::ProviderError::invalid_request(
                    "deepl",
                    "Invalid translation format. Expected: 'Translate to {lang}: {text}'",
                ),
            );
        }

        let instruction = parts[0].trim();
        let text_to_translate = parts[1..].join(":").trim().to_string();

        let (source_lang, target_lang) =
            if instruction.contains("from") && instruction.contains("to") {
                // Parse "Translate from EN to DE"
                let lang_parts: Vec<&str> = instruction.split_whitespace().collect();
                let from_idx = lang_parts
                    .iter()
                    .position(|&s| s == "from")
                    .ok_or_else(|| {
                        crate::core::providers::unified_provider::ProviderError::invalid_request(
                            "deepl",
                            "Invalid translation format",
                        )
                    })?;
                let to_idx = lang_parts.iter().position(|&s| s == "to").ok_or_else(|| {
                    crate::core::providers::unified_provider::ProviderError::invalid_request(
                        "deepl",
                        "Invalid translation format",
                    )
                })?;

                let source = if from_idx + 1 < lang_parts.len() {
                    Some(lang_parts[from_idx + 1].to_uppercase())
                } else {
                    None
                };

                let target = if to_idx + 1 < lang_parts.len() {
                    lang_parts[to_idx + 1].to_uppercase()
                } else {
                    return Err(
                        crate::core::providers::unified_provider::ProviderError::invalid_request(
                            "deepl",
                            "Target language not specified",
                        ),
                    );
                };

                (source, target)
            } else if instruction.contains("to") {
                // Parse "Translate to DE"
                let lang_parts: Vec<&str> = instruction.split_whitespace().collect();
                let to_idx = lang_parts.iter().position(|&s| s == "to").ok_or_else(|| {
                    crate::core::providers::unified_provider::ProviderError::invalid_request(
                        "deepl",
                        "Invalid translation format",
                    )
                })?;

                let target = if to_idx + 1 < lang_parts.len() {
                    lang_parts[to_idx + 1].to_uppercase()
                } else {
                    return Err(
                        crate::core::providers::unified_provider::ProviderError::invalid_request(
                            "deepl",
                            "Target language not specified",
                        ),
                    );
                };

                (None, target)
            } else {
                return Err(
                    crate::core::providers::unified_provider::ProviderError::invalid_request(
                        "deepl",
                        "Invalid translation format. Expected: 'Translate to {lang}: {text}'",
                    ),
                );
            };

        Ok((target_lang, source_lang, text_to_translate))
    }
}

#[cfg(test)]
mod tests {
    use super::super::config::DeepLConfig;
    use super::*;
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
    use crate::core::types::model::ProviderCapability;

    #[tokio::test]
    async fn test_provider_creation() {
        let config = DeepLConfig::new("deepl").with_api_key("test-key");
        let provider = DeepLProvider::new(config);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_provider_capabilities() {
        let config = DeepLConfig::new("deepl").with_api_key("test-key");
        let provider = DeepLProvider::new(config).unwrap();

        let caps = provider.capabilities();
        assert!(caps.contains(&ProviderCapability::ChatCompletion));
        assert!(!caps.contains(&ProviderCapability::AudioTranslation));
    }

    #[test]
    fn test_provider_models() {
        let config = DeepLConfig::new("deepl").with_api_key("test-key");
        let provider = DeepLProvider::new(config).unwrap();

        let models = provider.models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "deepl-translate"));
    }

    #[test]
    fn test_provider_name() {
        let config = DeepLConfig::new("deepl").with_api_key("test-key");
        let provider = DeepLProvider::new(config).unwrap();
        assert_eq!(provider.name(), "deepl");
    }
}
