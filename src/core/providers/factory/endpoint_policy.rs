use super::{builder, catalog_definition_for_supported_selector, provider_diagnostic_name};
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::openai::config::validate_private_official_openai_endpoint;
use crate::core::providers::{ProviderError, ProviderType, registry as provider_registry};

pub(super) fn provider_type_supports(provider_type: &ProviderType) -> bool {
    use ProviderType::*;
    match provider_type {
        OpenAI | OpenAICompatible | Anthropic | Mistral | Cohere | Voyage | Azure | AzureAI
        | Bedrock | VertexAI | Gemini | Ollama | Databricks | Snowflake | Oci | Watsonx
        | SageMaker | Stability | BlackForestLabs | Deepgram | ElevenLabs => true,
        Cloudflare | FalAI | Replicate | GitHubCopilot => false,
        _ => provider_registry::catalog_definition_for_provider_type(provider_type).is_some(),
    }
}

pub(super) fn merge_factory_endpoint_and_settings(
    factory: &mut serde_json::Map<String, serde_json::Value>,
    provider_type: &ProviderType,
    base_url: Option<&str>,
    configured_endpoint: Option<&str>,
    settings: &std::collections::HashMap<String, serde_json::Value>,
) {
    let native_image_provider = matches!(
        provider_type,
        ProviderType::Stability | ProviderType::BlackForestLabs
    );
    let endpoint = if native_image_provider {
        configured_endpoint
    } else {
        base_url
    };
    if let Some(endpoint) = endpoint {
        let key = if native_image_provider {
            "api_base"
        } else {
            "base_url"
        };
        factory.insert(key.to_string(), endpoint.into());
    }
    for (key, value) in settings {
        if native_image_provider && matches!(key.as_str(), "base_url" | "api_base") {
            continue;
        }
        factory.entry(key.clone()).or_insert_with(|| value.clone());
    }
}

pub(crate) fn selector_supports_endpoint_access(selector: &str) -> bool {
    catalog_definition_for_supported_selector(selector).is_some()
        || selector
            .parse::<ProviderType>()
            .is_ok_and(|provider_type| provider_type_supports(&provider_type))
}

pub(crate) fn selector_allows_implicit_private(selector: &str) -> bool {
    selector.parse::<ProviderType>().is_ok_and(|provider_type| {
        matches!(provider_type, ProviderType::Bedrock | ProviderType::Ollama)
    }) || catalog_definition_for_supported_selector(selector)
        .and_then(|definition| url::Url::parse(definition.base_url).ok())
        .is_some_and(|url| url.host_str() == Some("localhost"))
}

const STANDARD_ENDPOINT_KEYS: &[&str] = &["base_url", "api_base"];
const AZURE_ENDPOINT_KEYS: &[&str] = &["base_url", "api_base", "endpoint", "azure_endpoint"];
const AZURE_AI_ENDPOINT_KEYS: &[&str] = &["base_url", "api_base", "endpoint", "azure_ai_endpoint"];
const VERTEX_ENDPOINT_KEYS: &[&str] = &["base_url", "api_base", "endpoint"];
const DATABRICKS_ENDPOINT_KEYS: &[&str] = &["base_url", "api_base", "workspace_url"];

pub(crate) fn endpoint_keys_for_selector(selector: &str) -> &'static [&'static str] {
    match selector.parse::<ProviderType>() {
        Ok(ProviderType::Azure) => AZURE_ENDPOINT_KEYS,
        Ok(ProviderType::AzureAI) => AZURE_AI_ENDPOINT_KEYS,
        Ok(ProviderType::VertexAI) => VERTEX_ENDPOINT_KEYS,
        Ok(ProviderType::Databricks) => DATABRICKS_ENDPOINT_KEYS,
        _ => STANDARD_ENDPOINT_KEYS,
    }
}

pub(crate) fn invalid_endpoint(value: Option<&serde_json::Value>) -> bool {
    value.is_some_and(|value| {
        !value.is_null() && value.as_str().is_none_or(|url| url.trim().is_empty())
    })
}

pub(crate) fn configured_endpoint_for_keys<'a>(
    base_endpoint: Option<&'a str>,
    config: &'a std::collections::HashMap<String, serde_json::Value>,
    endpoint_keys: &[&str],
) -> Option<&'a str> {
    base_endpoint.or_else(|| {
        endpoint_keys.iter().copied().find_map(|key| {
            config
                .get(key)
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.trim().is_empty())
        })
    })
}

fn is_native_default_endpoint(
    provider_type: &ProviderType,
    config: &serde_json::Value,
    access: ProviderEndpointAccess,
) -> bool {
    let endpoint = config.get("api_base").and_then(serde_json::Value::as_str);
    let default = match provider_type {
        ProviderType::FalAI => "https://fal.run",
        ProviderType::Replicate => "https://api.replicate.com/v1",
        _ => return false,
    };
    access == ProviderEndpointAccess::PublicOnly
        && config
            .get("base_url")
            .is_none_or(serde_json::Value::is_null)
        && endpoint == Some(default)
}

pub(super) fn validate_direct_endpoint_policy(
    provider_type: &ProviderType,
    config: &serde_json::Value,
) -> Result<(), ProviderError> {
    let provider = provider_diagnostic_name(provider_type);
    let fail = |message| ProviderError::configuration(provider, message);
    let endpoint_keys = endpoint_keys_for_selector(&provider_type.to_string());
    if endpoint_keys
        .iter()
        .copied()
        .any(|key| invalid_endpoint(config.get(key)))
    {
        return Err(fail("endpoint must be a string"));
    }
    let configured_endpoint = endpoint_keys.iter().copied().find_map(|key| {
        config
            .get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.trim().is_empty())
    });
    let has_endpoint = configured_endpoint.is_some();
    let access = builder::config_endpoint_access(config, provider)?;
    validate_private_official_openai_endpoint(access, configured_endpoint).map_err(fail)?;
    let selector = provider_type.to_string();
    if access == ProviderEndpointAccess::PrivateNetwork
        && !has_endpoint
        && !selector_allows_implicit_private(&selector)
    {
        return Err(fail("private_network endpoint access requires a base URL"));
    }
    if !provider_type_supports(provider_type)
        && (config
            .get("endpoint_access")
            .is_some_and(|value| !value.is_null())
            || has_endpoint)
        && !is_native_default_endpoint(provider_type, config, access)
    {
        return Err(fail(
            "configurable endpoint access is unavailable because this provider runtime is not policy-wired",
        ));
    }
    Ok(())
}
