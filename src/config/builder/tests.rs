//! Tests for configuration builders

use super::presets;
use super::types::{GatewayConfigBuilder, ProviderConfigBuilder};
#[cfg(test)]
use crate::config::models::auth::{AuthConfig, RbacConfig};

#[test]
fn test_config_builder() {
    let config = GatewayConfigBuilder::new()
        .with_server(presets::dev_server().build())
        .with_auth(AuthConfig {
            enable_jwt: true,
            enable_api_key: true,
            jwt_secret: "StrongJwtSecretWithMixedCaseAndNumbers1234!".to_string(),
            jwt_expiration: 3600,
            api_key_header: "X-API-Key".to_string(),
            api_key_hmac_secret: None,
            allow_anonymous: false,
            rbac: RbacConfig::default(),
        })
        .add_provider(
            presets::openai_provider("openai", "test-key")
                .unwrap()
                .build()
                .unwrap(),
        )
        .enable_feature("metrics")
        .build();

    assert!(config.is_ok());
    let config = config.unwrap();
    assert_eq!(config.gateway.server.port, 8080);
    assert_eq!(config.gateway.providers.len(), 1);
}

#[test]
fn test_provider_builder() {
    let provider = ProviderConfigBuilder::new()
        .name("test-provider")
        .unwrap()
        .provider_type("openai")
        .unwrap()
        .api_key("test-key")
        .add_model("gpt-4")
        .weight(2.0)
        .unwrap()
        .build();

    assert!(provider.is_ok());
    let provider = provider.unwrap();
    assert_eq!(provider.name, "test-provider");
    assert_eq!(provider.weight, 2.0);
}
