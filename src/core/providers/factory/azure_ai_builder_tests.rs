use super::builder::build_azure_ai_config_from_factory;
use crate::core::net::ProviderEndpointAccess;

fn azure_ai_config() -> serde_json::Value {
    serde_json::json!({
        "api_key": "azure-ai-key",
        "azure_ai_endpoint": "https://example-resource.services.ai.azure.com"
    })
}

#[test]
fn azure_ai_factory_defaults_endpoint_access_to_public_only() {
    let config = match build_azure_ai_config_from_factory(&azure_ai_config()) {
        Ok(config) => config,
        Err(error) => panic!("AzureAI config should build: {error}"),
    };

    assert_eq!(
        config.base.endpoint_access,
        ProviderEndpointAccess::PublicOnly
    );
}

#[test]
fn azure_ai_factory_maps_private_endpoint_access() {
    let mut input = azure_ai_config();
    input["endpoint_access"] = serde_json::json!("private_network");
    let config = match build_azure_ai_config_from_factory(&input) {
        Ok(config) => config,
        Err(error) => panic!("AzureAI private config should build: {error}"),
    };

    assert_eq!(
        config.base.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );
}

#[test]
fn azure_ai_factory_rejects_invalid_endpoint_access() {
    let mut input = azure_ai_config();
    input["endpoint_access"] = serde_json::json!("internal_only");

    assert!(build_azure_ai_config_from_factory(&input).is_err());
}
