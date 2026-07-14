use super::{builder, catalog_definition_for_supported_selector, provider_diagnostic_name};
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::providers::{ProviderType, registry as provider_registry};

pub(super) fn provider_type_supports(provider_type: &ProviderType) -> bool {
    use ProviderType::*;
    match provider_type {
        OpenAI | OpenAICompatible | Anthropic | Mistral | Cohere | Azure | AzureAI | Bedrock
        | VertexAI | Gemini => true,
        Cloudflare | FalAI | Replicate | GitHubCopilot => false,
        _ => provider_registry::catalog_definition_for_provider_type(provider_type).is_some(),
    }
}

pub(crate) fn selector_supports_endpoint_access(selector: &str) -> bool {
    catalog_definition_for_supported_selector(selector).is_some()
        || selector
            .parse::<ProviderType>()
            .is_ok_and(|provider_type| provider_type_supports(&provider_type))
}

pub(crate) fn selector_allows_implicit_private(selector: &str) -> bool {
    selector
        .parse::<ProviderType>()
        .is_ok_and(|provider_type| matches!(provider_type, ProviderType::Bedrock))
        || catalog_definition_for_supported_selector(selector)
            .and_then(|definition| url::Url::parse(definition.base_url).ok())
            .is_some_and(|url| url.host_str() == Some("localhost"))
}

pub(crate) fn invalid_endpoint(value: Option<&serde_json::Value>) -> bool {
    value.is_some_and(|value| !value.is_null() && !value.is_string())
}

pub(super) fn validate_direct_endpoint_policy(
    provider_type: &ProviderType,
    config: &serde_json::Value,
) -> Result<(), ProviderError> {
    let provider = provider_diagnostic_name(provider_type);
    let fail = |message| ProviderError::configuration(provider, message);
    if ["base_url", "api_base"]
        .into_iter()
        .any(|key| invalid_endpoint(config.get(key)))
    {
        return Err(fail("endpoint must be a string"));
    }
    let has_endpoint = ["base_url", "api_base"].into_iter().any(|key| {
        config
            .get(key)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    });
    let access = builder::config_endpoint_access(config, provider)?;
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
    {
        return Err(fail(
            "configurable endpoint access is unavailable because this provider runtime is not policy-wired",
        ));
    }
    Ok(())
}
