use reqwest::Method;

use super::{
    AzureAIChatHandler, AzureAIEmbeddingHandler, AzureAIImageHandler, AzureAIProvider,
    AzureAIRerankHandler, client::AzureAIClient, config::AzureAIConfig,
};
use crate::core::net::ProviderEndpointAccess;

fn policy_config(endpoint: &str, access: ProviderEndpointAccess) -> AzureAIConfig {
    let mut config = AzureAIConfig::new("azure_ai");
    config.base.api_key = Some("test-key".to_string());
    config.base.api_base = Some(endpoint.to_string());
    config.base.endpoint_access = access;
    config
}

#[test]
fn public_loopback_is_rejected_by_every_native_consumer() {
    let config = policy_config("http://127.0.0.1:18080", ProviderEndpointAccess::PublicOnly);

    assert!(AzureAIChatHandler::new(config.clone()).is_err());
    assert!(AzureAIEmbeddingHandler::new(config.clone()).is_err());
    assert!(AzureAIImageHandler::new(config.clone()).is_err());
    assert!(AzureAIRerankHandler::new(config.clone()).is_err());
    assert!(AzureAIProvider::new(config).is_err());
}

#[test]
fn private_loopback_is_accepted_by_every_native_consumer() {
    let config = policy_config(
        "http://127.0.0.1:18080",
        ProviderEndpointAccess::PrivateNetwork,
    );

    assert!(AzureAIChatHandler::new(config.clone()).is_ok());
    assert!(AzureAIEmbeddingHandler::new(config.clone()).is_ok());
    assert!(AzureAIImageHandler::new(config.clone()).is_ok());
    assert!(AzureAIRerankHandler::new(config.clone()).is_ok());
    assert!(AzureAIProvider::new(config).is_ok());
}

#[test]
fn metadata_is_rejected_even_with_private_access() {
    let config = policy_config(
        "http://169.254.169.254",
        ProviderEndpointAccess::PrivateNetwork,
    );

    assert!(AzureAIClient::new(config).is_err());
}

#[test]
fn private_client_rejects_cross_authority_requests() {
    let config = policy_config(
        "http://127.0.0.1:18080",
        ProviderEndpointAccess::PrivateNetwork,
    );
    let client = match AzureAIClient::new(config) {
        Ok(client) => client,
        Err(error) => panic!("private policy client should initialize: {error}"),
    };

    assert!(
        client
            .request(
                Method::POST,
                "http://127.0.0.1:18080/models/chat/completions"
            )
            .is_ok()
    );
    assert!(
        client
            .streaming_request(
                Method::POST,
                "http://127.0.0.1:18080/models/chat/completions",
            )
            .is_ok()
    );
    assert!(
        client
            .request(Method::GET, "http://127.0.0.1:18081/models")
            .is_err()
    );
    assert!(
        client
            .streaming_request(
                Method::POST,
                "http://127.0.0.1:18081/models/chat/completions",
            )
            .is_err()
    );
}
