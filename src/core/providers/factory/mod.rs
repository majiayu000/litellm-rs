//! Provider factory: creation from configuration
//!
//! This module coordinates provider creation. It is split into focused submodules:
//! - `resolver`: selector support detection
//! - `builder`: per-provider config builders and extraction helpers
//! - `registry`: `Provider::from_config_async` dispatch table

mod builder;
mod registry;
mod resolver;

pub use resolver::is_provider_selector_supported;

use super::provider_type::ProviderType;
use super::unified_provider::ProviderError;
use super::{Provider, openai_like, registry as provider_registry};
use tracing::warn;

/// Create a provider from configuration
///
/// This is the main factory function for creating providers
pub async fn create_provider(
    config: crate::config::models::provider::ProviderConfig,
) -> Result<Provider, ProviderError> {
    use serde_json::Value;

    let crate::config::models::provider::ProviderConfig {
        name,
        provider_type,
        api_key,
        base_url,
        api_version,
        organization,
        project,
        timeout,
        max_retries,
        settings,
        ..
    } = config;

    let provider_selector = if provider_type.trim().is_empty() {
        name.as_str()
    } else {
        provider_type.as_str()
    };
    // --- Tier 1: check the data-driven catalog first ---
    let provider_name_lower = provider_selector.to_lowercase();
    if let Some(def) = provider_registry::get_definition(&provider_name_lower) {
        let effective_key = if api_key.is_empty() {
            def.resolve_api_key(None)
        } else {
            Some(api_key.clone())
        };
        let mut oai_config =
            def.to_openai_like_config(effective_key.as_deref(), base_url.as_deref());
        oai_config.base.timeout = timeout;
        oai_config.base.max_retries = max_retries;

        if let Some(version) = api_version.filter(|v| !v.trim().is_empty()) {
            oai_config.base.api_version = Some(version);
        }
        if let Some(org) = organization.filter(|v| !v.trim().is_empty()) {
            oai_config.base.organization = Some(org);
        }

        let ignored_settings =
            builder::apply_tier1_openai_like_overrides(&mut oai_config, &settings);
        if !ignored_settings.is_empty() {
            warn!(
                provider = def.name,
                ignored_settings = ?ignored_settings,
                "Tier-1 catalog provider has unsupported settings that were ignored"
            );
        }
        if let Some(project) = project.filter(|v| !v.trim().is_empty()) {
            warn!(
                provider = def.name,
                project = %project,
                "Provider project field is ignored for Tier-1 catalog providers"
            );
        }

        let provider = openai_like::OpenAILikeProvider::new(oai_config)
            .await
            .map_err(|e| ProviderError::initialization(def.name, e.to_string()))?;
        return Ok(Provider::OpenAILike(provider));
    }

    // --- Tier 2/3: existing factory logic ---
    // Catalog selectors are already handled above; use strict FromStr so unknown strings
    // produce a ConfigError::InvalidValue instead of silently becoming ProviderType::Custom.
    let provider_type_enum = provider_selector
        .parse::<ProviderType>()
        .map_err(|e| ProviderError::invalid_request("provider_type", e.to_string()))?;

    if !Provider::factory_supported_provider_types().contains(&provider_type_enum) {
        return Err(ProviderError::not_implemented(
            "unknown",
            format!("Factory for {:?} not yet implemented", provider_type_enum),
        ));
    }

    let mut factory_config = serde_json::Map::new();

    if !api_key.is_empty() {
        factory_config.insert("api_key".to_string(), Value::String(api_key.clone()));
    }
    if let Some(value) = base_url.filter(|v| !v.is_empty()) {
        factory_config.insert("base_url".to_string(), Value::String(value));
    }
    if let Some(value) = api_version.filter(|v| !v.is_empty()) {
        factory_config.insert("api_version".to_string(), Value::String(value));
    }
    if let Some(value) = organization.filter(|v| !v.is_empty()) {
        factory_config.insert("organization".to_string(), Value::String(value.clone()));
        factory_config
            .entry("account_id".to_string())
            .or_insert(Value::String(value));
    }
    if let Some(value) = project.filter(|v| !v.is_empty()) {
        factory_config.insert("project".to_string(), Value::String(value));
    }

    for (key, value) in settings {
        factory_config.entry(key).or_insert(value);
    }

    if matches!(provider_type_enum, ProviderType::Cloudflare)
        && !factory_config.contains_key("api_token")
        && !api_key.is_empty()
    {
        factory_config.insert("api_token".to_string(), Value::String(api_key));
    }

    Provider::from_config_async(provider_type_enum, Value::Object(factory_config)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::registry as provider_registry;

    #[tokio::test]
    async fn test_catalog_entries_are_creatable_via_factory() {
        for (name, def) in provider_registry::PROVIDER_CATALOG.iter() {
            let config = crate::config::models::provider::ProviderConfig {
                name: (*name).to_string(),
                provider_type: (*name).to_string(),
                api_key: if def.skip_api_key {
                    String::new()
                } else {
                    "test-key".to_string()
                },
                ..Default::default()
            };

            let provider = create_provider(config).await.unwrap_or_else(|e| {
                panic!("Catalog provider '{}' should be creatable: {}", name, e)
            });

            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Catalog provider '{}' must create OpenAILike variant",
                name
            );
        }
    }

    #[tokio::test]
    async fn test_create_provider_prefers_provider_type_over_name() {
        let config = crate::config::models::provider::ProviderConfig {
            name: "openai".to_string(),
            provider_type: "pydantic_ai".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let err = create_provider(config)
            .await
            .expect_err("Expected unsupported provider type to fail");
        assert!(
            matches!(err, ProviderError::NotImplemented { .. }),
            "Expected NotImplemented error, got {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_provider_falls_back_to_name_when_provider_type_empty() {
        let config = crate::config::models::provider::ProviderConfig {
            name: "pydantic_ai".to_string(),
            provider_type: "".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let err = create_provider(config)
            .await
            .expect_err("Expected unsupported provider name to fail");
        assert!(
            matches!(err, ProviderError::NotImplemented { .. }),
            "Expected NotImplemented error, got {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_provider_tier1_catalog_creates_openai_like() {
        let config = crate::config::models::provider::ProviderConfig {
            name: "perplexity".to_string(),
            provider_type: "".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let provider = create_provider(config)
            .await
            .expect("Tier 1 provider should succeed");
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }

    #[tokio::test]
    async fn test_create_provider_tier1_catalog_applies_openai_like_overrides() {
        let mut config = crate::config::models::provider::ProviderConfig {
            name: "perplexity".to_string(),
            provider_type: "".to_string(),
            api_key: "test-key".to_string(),
            timeout: 42,
            max_retries: 6,
            api_version: Some("2024-01-01".to_string()),
            organization: Some("org-top-level".to_string()),
            ..Default::default()
        };
        config
            .settings
            .insert("model_prefix".to_string(), serde_json::json!("pplx/"));
        config.settings.insert(
            "default_model".to_string(),
            serde_json::json!("llama-3.1-sonar-small"),
        );
        config
            .settings
            .insert("pass_through_params".to_string(), serde_json::json!(false));
        config.settings.insert(
            "headers".to_string(),
            serde_json::json!({"x-test-header": "ok"}),
        );
        config.settings.insert(
            "custom_headers".to_string(),
            serde_json::json!({"x-custom-header": "ok"}),
        );

        let provider = create_provider(config)
            .await
            .expect("Tier 1 provider should accept openai-like overrides");

        match provider {
            Provider::OpenAILike(provider) => {
                let cfg = provider.config();
                assert_eq!(cfg.provider_name, "perplexity");
                assert_eq!(cfg.base.timeout, 42);
                assert_eq!(cfg.base.max_retries, 6);
                assert_eq!(cfg.base.api_version.as_deref(), Some("2024-01-01"));
                assert_eq!(cfg.base.organization.as_deref(), Some("org-top-level"));
                assert_eq!(cfg.model_prefix.as_deref(), Some("pplx/"));
                assert_eq!(cfg.default_model.as_deref(), Some("llama-3.1-sonar-small"));
                assert!(!cfg.pass_through_params);
                assert_eq!(
                    cfg.base.headers.get("x-test-header").map(String::as_str),
                    Some("ok")
                );
                assert_eq!(
                    cfg.custom_headers
                        .get("x-custom-header")
                        .map(String::as_str),
                    Some("ok")
                );
            }
            _ => panic!("Expected OpenAILike provider"),
        }
    }

    #[test]
    fn test_b1_first_batch_selectors_are_supported() {
        for selector in ["aiml_api", "anyscale", "bytez", "comet_api"] {
            assert!(
                is_provider_selector_supported(selector),
                "Expected selector '{}' to be supported",
                selector
            );
        }
    }

    #[tokio::test]
    async fn test_b1_first_batch_create_provider_from_name() {
        for provider_name in ["aiml_api", "anyscale", "bytez", "comet_api"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: provider_name.to_string(),
                provider_type: "".to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config)
                .await
                .unwrap_or_else(|e| panic!("Expected '{}' to be creatable: {}", provider_name, e));
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected '{}' to create OpenAILike provider",
                provider_name
            );
        }
    }

    #[tokio::test]
    async fn test_b1_first_batch_create_provider_from_provider_type() {
        for provider_type in ["aiml_api", "anyscale", "bytez", "comet_api"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: "openai".to_string(),
                provider_type: provider_type.to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config).await.unwrap_or_else(|e| {
                panic!(
                    "Expected '{}' provider_type to be creatable: {}",
                    provider_type, e
                )
            });
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected provider_type '{}' to create OpenAILike provider",
                provider_type
            );
        }
    }

    #[test]
    fn test_b2_second_batch_selectors_are_supported() {
        for selector in ["compactifai", "aleph_alpha", "yi", "lambda_ai"] {
            assert!(
                is_provider_selector_supported(selector),
                "Expected selector '{}' to be supported",
                selector
            );
        }
    }

    #[tokio::test]
    async fn test_b2_second_batch_create_provider_from_name() {
        for provider_name in ["compactifai", "aleph_alpha", "yi", "lambda_ai"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: provider_name.to_string(),
                provider_type: "".to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config)
                .await
                .unwrap_or_else(|e| panic!("Expected '{}' to be creatable: {}", provider_name, e));
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected '{}' to create OpenAILike provider",
                provider_name
            );
        }
    }

    #[tokio::test]
    async fn test_b2_second_batch_create_provider_from_provider_type() {
        for provider_type in ["compactifai", "aleph_alpha", "yi", "lambda_ai"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: "openai".to_string(),
                provider_type: provider_type.to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config).await.unwrap_or_else(|e| {
                panic!(
                    "Expected '{}' provider_type to be creatable: {}",
                    provider_type, e
                )
            });
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected provider_type '{}' to create OpenAILike provider",
                provider_type
            );
        }
    }

    #[test]
    fn test_b3_third_batch_selectors_are_supported() {
        for selector in ["ovhcloud", "maritalk", "siliconflow", "lemonade"] {
            assert!(
                is_provider_selector_supported(selector),
                "Expected selector '{}' to be supported",
                selector
            );
        }
    }

    #[tokio::test]
    async fn test_b3_third_batch_create_provider_from_name() {
        for provider_name in ["ovhcloud", "maritalk", "siliconflow", "lemonade"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: provider_name.to_string(),
                provider_type: "".to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config)
                .await
                .unwrap_or_else(|e| panic!("Expected '{}' to be creatable: {}", provider_name, e));
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected '{}' to create OpenAILike provider",
                provider_name
            );
        }
    }

    #[tokio::test]
    async fn test_b3_third_batch_create_provider_from_provider_type() {
        for provider_type in ["ovhcloud", "maritalk", "siliconflow", "lemonade"] {
            let config = crate::config::models::provider::ProviderConfig {
                name: "openai".to_string(),
                provider_type: provider_type.to_string(),
                api_key: "test-key".to_string(),
                ..Default::default()
            };

            let provider = create_provider(config).await.unwrap_or_else(|e| {
                panic!(
                    "Expected '{}' provider_type to be creatable: {}",
                    provider_type, e
                )
            });
            assert!(
                matches!(provider, Provider::OpenAILike(_)),
                "Expected provider_type '{}' to create OpenAILike provider",
                provider_type
            );
        }
    }

    #[tokio::test]
    async fn test_create_provider_reports_unknown_custom_provider() {
        let config = crate::config::models::provider::ProviderConfig {
            name: "my-custom-provider".to_string(),
            provider_type: "".to_string(),
            api_key: "test-key".to_string(),
            ..Default::default()
        };

        let err = create_provider(config)
            .await
            .expect_err("Expected unknown custom provider to fail");
        // Unknown provider strings now produce InvalidRequest (via ConfigError::InvalidValue)
        // instead of NotImplemented, so callers get a clear parse-time error.
        assert!(
            matches!(err, ProviderError::InvalidRequest { .. }),
            "Expected InvalidRequest error, got {}",
            err
        );
        assert!(
            err.to_string().contains("my-custom-provider"),
            "Expected custom provider name in error, got {}",
            err
        );
    }

    #[tokio::test]
    async fn test_create_provider_openai_compatible_factory() {
        let mut config = crate::config::models::provider::ProviderConfig {
            name: "local-openai-like".to_string(),
            provider_type: "openai_compatible".to_string(),
            api_key: "".to_string(),
            base_url: Some("http://localhost:11434/v1".to_string()),
            ..Default::default()
        };
        config
            .settings
            .insert("skip_api_key".to_string(), serde_json::Value::Bool(true));

        let provider = create_provider(config)
            .await
            .expect("openai_compatible provider should be creatable");
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }
}
