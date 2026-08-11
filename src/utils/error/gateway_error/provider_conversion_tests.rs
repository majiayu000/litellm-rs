use super::*;

#[test]
fn provider_authentication_is_preserved_in_gateway_error() {
    let provider_error = ProviderError::authentication("openai", "invalid key");
    let gateway_error = GatewayError::from(provider_error);

    assert!(matches!(
        gateway_error,
        GatewayError::Provider(ProviderError::Authentication {
            provider: "openai",
            ..
        })
    ));
}

#[test]
fn provider_rate_limit_is_preserved_in_gateway_error() {
    let provider_error = ProviderError::rate_limit("anthropic", Some(30));
    let gateway_error = GatewayError::from(provider_error);

    assert!(matches!(
        gateway_error,
        GatewayError::Provider(ProviderError::RateLimit {
            provider: "anthropic",
            retry_after: Some(30),
            ..
        })
    ));
}
