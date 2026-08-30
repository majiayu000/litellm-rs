//! Provider factory: creation from configuration
//!
//! This module coordinates provider creation. It is split into focused submodules:
//! - `resolver`: selector support detection
//! - `builder`: per-provider config builders and extraction helpers
//! - `registry`: `Provider::from_config_async` dispatch table

#[cfg(all(test, feature = "providers-extra"))]
mod azure_ai_builder_tests;
mod builder;
#[cfg(test)]
mod builder_tests;
#[cfg(feature = "providers-extended")]
mod cohere_builder;
#[cfg(test)]
mod endpoint_access_tests;
mod endpoint_policy;
mod enterprise_builder;
#[cfg(test)]
mod factory_creation_tests;
#[cfg(feature = "providers-extended")]
mod fal_ai_builder;
#[cfg(feature = "providers-extended")]
mod gemini_builder;
mod registry;
#[cfg(feature = "providers-extended")]
mod replicate_builder;
mod resolver;

pub(crate) use super::openai::config::validate_private_official_openai_endpoint;
pub(crate) use endpoint_policy::{
    configured_endpoint_for_keys, endpoint_keys_for_selector, invalid_endpoint,
    selector_allows_implicit_private, selector_supports_endpoint_access,
};
pub use resolver::is_provider_selector_supported;

use super::provider_type::ProviderType;
use super::unified_provider::ProviderError;
use super::{Provider, openai_like, registry as provider_registry};
use crate::core::net::ProviderEndpointAccess;
use endpoint_policy::provider_type_supports;
use tracing::warn;

fn provider_diagnostic_name(provider_type: &ProviderType) -> &'static str {
    provider_registry::entry_for_type(provider_type)
        .map(|entry| entry.canonical_name)
        .unwrap_or("custom")
}

fn catalog_definition_for_supported_selector(
    selector: &str,
) -> Option<&'static provider_registry::ProviderDefinition> {
    let normalized = selector.trim().to_ascii_lowercase();
    let def = provider_registry::get_definition(&normalized)?;
    match provider_registry::entry_for_name(&normalized) {
        Some(entry)
            if entry.dispatch_kind
                == provider_registry::ProviderDispatchKind::CatalogOpenAiLike =>
        {
            Some(def)
        }
        Some(_) => None,
        None => Some(def),
    }
}

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
        endpoint_access,
        api_version,
        organization,
        project,
        timeout,
        max_retries,
        mut settings,
        models,
        ..
    } = config;

    let provider_selector = if provider_type.trim().is_empty() {
        name.as_str()
    } else {
        provider_type.as_str()
    };
    let endpoint_keys = endpoint_keys_for_selector(provider_selector);
    if base_url.as_ref().is_some_and(|url| url.trim().is_empty())
        || endpoint_keys
            .iter()
            .copied()
            .any(|key| invalid_endpoint(settings.get(key)))
    {
        return Err(ProviderError::configuration(
            "provider",
            "endpoint must be a string",
        ));
    }
    let base_endpoint = base_url.as_deref().filter(|url| !url.trim().is_empty());
    let configured_endpoint = configured_endpoint_for_keys(base_endpoint, &settings, endpoint_keys);
    let has_endpoint = configured_endpoint.is_some();
    if settings.contains_key("endpoint_access") {
        return Err(ProviderError::configuration(
            "provider",
            "endpoint_access must be configured as a top-level provider field",
        ));
    }
    if endpoint_access == ProviderEndpointAccess::PrivateNetwork
        && !has_endpoint
        && !selector_allows_implicit_private(provider_selector)
    {
        return Err(ProviderError::configuration(
            "provider",
            "private_network endpoint access requires a base URL",
        ));
    }
    validate_private_official_openai_endpoint(endpoint_access, configured_endpoint)
        .map_err(|message| ProviderError::configuration("provider", message))?;
    if let Some(def) = catalog_definition_for_supported_selector(provider_selector) {
        let effective_key = if api_key.is_empty() {
            def.resolve_api_key(None)
        } else {
            Some(api_key.clone())
        };
        let settings_base_url = ["base_url", "api_base"]
            .into_iter()
            .filter_map(|key| settings.remove(key))
            .find_map(|value| {
                value
                    .as_str()
                    .filter(|url| !url.trim().is_empty())
                    .map(str::to_owned)
            });
        let mut oai_config = def.to_openai_like_config(
            effective_key.as_deref(),
            base_endpoint.or(settings_base_url.as_deref()),
        );
        oai_config.base.endpoint_access = endpoint_access;
        // Auto-detect private network access for catalog providers whose default
        // base URL is localhost (e.g. vllm, lm_studio, ollama).  When the user
        // relies on the catalog default and has not explicitly set the endpoint
        // access policy, switch to PrivateNetwork so the SSRF guard permits the
        // outbound connection to localhost.
        if endpoint_access == ProviderEndpointAccess::PublicOnly
            && base_endpoint.is_none()
            && settings_base_url.is_none()
            && selector_allows_implicit_private(provider_selector)
        {
            oai_config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        }
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

        let provider =
            openai_like::OpenAILikeProvider::new_for_catalog(oai_config, def.capabilities)
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

    if !provider_type_supports(&provider_type_enum)
        && (endpoint_access == ProviderEndpointAccess::PrivateNetwork || has_endpoint)
    {
        return Err(ProviderError::configuration(
            provider_diagnostic_name(&provider_type_enum),
            "configurable endpoint access is unavailable because this provider runtime is not policy-wired",
        ));
    }
    if !Provider::factory_supported_provider_types().contains(&provider_type_enum) {
        return Err(ProviderError::not_implemented(
            provider_diagnostic_name(&provider_type_enum),
            format!("Factory for {:?} not yet implemented", provider_type_enum),
        ));
    }

    let mut factory_config = serde_json::Map::new();
    factory_config.insert(
        "endpoint_access".to_string(),
        Value::String(endpoint_access.to_string()),
    );

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
    factory_config.insert("timeout".to_string(), Value::Number(timeout.into()));
    factory_config.insert("max_retries".to_string(), Value::Number(max_retries.into()));
    if !models.is_empty() {
        factory_config.insert(
            "models".to_string(),
            Value::Array(models.into_iter().map(Value::String).collect()),
        );
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
    if matches!(
        provider_type_enum,
        ProviderType::OpenAI | ProviderType::OpenAICompatible
    ) {
        factory_config
            .entry("provider_name".to_string())
            .or_insert(Value::String(name));
    }

    Provider::from_gateway_config_async(provider_type_enum, Value::Object(factory_config)).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::net::ProviderEndpointAccess::{PrivateNetwork, PublicOnly};
    use crate::core::providers::registry as provider_registry;

    #[test]
    fn test_catalog_factory_shortcut_is_registry_dispatch_gated_for_enum_selectors() {
        for name in provider_registry::PROVIDER_CATALOG.keys() {
            let shortcut = catalog_definition_for_supported_selector(name);
            if let Some(entry) = provider_registry::entry_for_name(name) {
                assert_eq!(
                    shortcut.is_some(),
                    entry.dispatch_kind
                        == provider_registry::ProviderDispatchKind::CatalogOpenAiLike,
                    "{name} must not use the catalog factory shortcut unless registry dispatch says CatalogOpenAiLike"
                );
            }
        }
    }

    #[test]
    fn test_catalog_factory_shortcut_keeps_pure_catalog_only_selectors() {
        let selector = "together";
        assert!(
            provider_registry::entry_for_name(selector).is_none(),
            "{selector} should remain a pure catalog-only selector for this guard"
        );
        assert!(catalog_definition_for_supported_selector(selector).is_some());
    }

    #[tokio::test]
    async fn test_catalog_entries_are_creatable_via_factory() {
        for (name, def) in provider_registry::PROVIDER_CATALOG.iter() {
            let is_local = def.base_url.contains("localhost");
            let config = crate::config::models::provider::ProviderConfig {
                name: (*name).to_string(),
                provider_type: (*name).to_string(),
                api_key: "test-key".into(),
                endpoint_access: if is_local { PrivateNetwork } else { PublicOnly },
                ..Default::default()
            };
            let mut opposite = config.clone();
            opposite.endpoint_access = if is_local { PublicOnly } else { PrivateNetwork };
            if is_local {
                // Use an explicit base URL matching the catalog default so the
                // auto-detection of catalog-default localhost does not kick in;
                // the SSRF guard must still block PublicOnly + localhost.
                opposite.base_url = Some(def.base_url.to_string());
                assert!(create_provider(opposite).await.is_err());
            } else {
                assert!(crate::config::Validate::validate(&opposite).is_err());
            }
            let provider = create_provider(config)
                .await
                .unwrap_or_else(|error| panic!("Catalog provider '{name}': {error}"));
            assert!(matches!(&provider, Provider::OpenAILike(_)));
            assert_eq!(provider.capabilities(), def.capabilities);
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
        assert_eq!(err.provider(), "pydantic_ai");
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
        assert_eq!(err.provider(), "pydantic_ai");
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
        assert!(matches!(&provider, Provider::OpenAILike(_)));
        let definition = provider_registry::get_definition("perplexity")
            .expect("Perplexity definition should exist");
        assert_eq!(provider.capabilities(), definition.capabilities);
    }

    #[tokio::test]
    async fn test_create_provider_cohere_feature_split() {
        let mut config = crate::config::models::provider::ProviderConfig {
            name: "cohere".to_string(),
            provider_type: "cohere".to_string(),
            api_key: "test-cohere-key".to_string(),
            base_url: Some("https://api.cohere.ai".to_string()),
            api_version: Some("v2".to_string()),
            timeout: 30,
            max_retries: 2,
            ..Default::default()
        };
        config.settings.insert(
            "default_embedding_input_type".to_string(),
            serde_json::json!("search_query"),
        );

        let result = create_provider(config).await;

        #[cfg(feature = "providers-extended")]
        {
            let provider =
                result.unwrap_or_else(|err| panic!("cohere should create native provider: {err}"));
            assert!(matches!(provider, Provider::Cohere(_)));
            assert_eq!(provider.name(), "cohere");
        }

        #[cfg(not(feature = "providers-extended"))]
        {
            let err = result.expect_err("cohere should be unsupported without providers-extended");
            assert!(
                matches!(err, ProviderError::NotImplemented { .. }),
                "Expected NotImplemented error, got {err}"
            );
            assert_eq!(err.provider(), "cohere");
        }
    }

    #[tokio::test]
    async fn test_create_provider_fal_ai_feature_split() {
        let mut config = crate::config::models::provider::ProviderConfig {
            name: "fal_ai".to_string(),
            provider_type: "fal_ai".to_string(),
            api_key: "test-fal-ai-key".to_string(),
            timeout: 30,
            max_retries: 2,
            ..Default::default()
        };
        config
            .settings
            .insert("output_format".to_string(), serde_json::json!("png"));
        config
            .settings
            .insert("sync_mode".to_string(), serde_json::json!(true));

        let result = create_provider(config).await;

        #[cfg(feature = "providers-extended")]
        {
            let provider =
                result.unwrap_or_else(|err| panic!("fal_ai should create native provider: {err}"));
            assert!(matches!(provider, Provider::FalAI(_)));
            assert_eq!(provider.name(), "fal_ai");
            assert!(
                provider
                    .capabilities()
                    .contains(&crate::core::types::model::ProviderCapability::ImageGeneration)
            );
        }

        #[cfg(not(feature = "providers-extended"))]
        {
            let err = result.expect_err("fal_ai should be unsupported without providers-extended");
            assert!(
                matches!(err, ProviderError::NotImplemented { .. }),
                "Expected NotImplemented error, got {err}"
            );
            assert_eq!(err.provider(), "fal_ai");
        }
    }

    #[tokio::test]
    async fn test_create_provider_replicate_feature_split() {
        let mut config = crate::config::models::provider::ProviderConfig {
            name: "replicate".to_string(),
            provider_type: "replicate".to_string(),
            api_key: "test-replicate-token".to_string(),
            timeout: 30,
            max_retries: 2,
            ..Default::default()
        };
        config
            .settings
            .insert("polling_delay_seconds".to_string(), serde_json::json!(1));
        config
            .settings
            .insert("polling_retries".to_string(), serde_json::json!(3));
        config
            .settings
            .insert("use_streaming".to_string(), serde_json::json!(true));

        let result = create_provider(config).await;

        #[cfg(feature = "providers-extended")]
        {
            let provider = result
                .unwrap_or_else(|err| panic!("replicate should create native provider: {err}"));
            assert!(matches!(provider, Provider::Replicate(_)));
            assert_eq!(provider.name(), "replicate");
            assert!(
                provider
                    .capabilities()
                    .contains(&crate::core::types::model::ProviderCapability::ImageGeneration)
            );
        }

        #[cfg(not(feature = "providers-extended"))]
        {
            let err =
                result.expect_err("replicate should be unsupported without providers-extended");
            assert!(
                matches!(err, ProviderError::NotImplemented { .. }),
                "Expected NotImplemented error, got {err}"
            );
            assert_eq!(err.provider(), "replicate");
        }
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
                assert_eq!(cfg.base.endpoint_access, ProviderEndpointAccess::PublicOnly);
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

    #[test]
    fn issue_760_litellm_alias_selectors_are_supported() {
        for selector in ["zai", "together_ai", "fireworks_ai", "aiml"] {
            assert!(
                is_provider_selector_supported(selector),
                "Expected LiteLLM alias selector '{}' to be supported",
                selector
            );
        }
    }

    #[tokio::test]
    async fn issue_760_create_provider_from_litellm_alias_selectors() {
        let cases = [
            ("zai", "https://api.z.ai/api/paas/v4"),
            ("together_ai", "https://api.together.xyz/v1"),
            ("fireworks_ai", "https://api.fireworks.ai/inference/v1"),
            ("aiml", "https://api.aimlapi.com/v1"),
        ];

        for (selector, base_url) in cases {
            for provider_type in ["", selector] {
                let config = crate::config::models::provider::ProviderConfig {
                    name: selector.to_string(),
                    provider_type: provider_type.to_string(),
                    api_key: "test-key".to_string(),
                    ..Default::default()
                };

                let provider = create_provider(config).await.unwrap_or_else(|e| {
                    panic!("Expected '{selector}' provider to be creatable: {e}")
                });
                let definition = provider_registry::get_definition(selector)
                    .expect("alias should resolve to a catalog definition");
                assert_eq!(provider.capabilities(), definition.capabilities);
                let Provider::OpenAILike(provider) = provider else {
                    panic!("Expected '{selector}' to create OpenAILike provider");
                };
                assert_eq!(provider.config().provider_name, selector);
                assert_eq!(provider.config().base.api_base.as_deref(), Some(base_url));
            }
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
}
