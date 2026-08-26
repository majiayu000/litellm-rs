fn valid_http_test_config() -> Config {
    let mut config = Config::default();
    config.gateway.providers.push(
        crate::config::models::provider::ProviderConfig {
            name: "test-openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test".to_string(),
            ..Default::default()
        },
    );
    config
}

#[tokio::test]
async fn new_rejects_invalid_auth_before_runtime_initialization() {
    let mut config = valid_http_test_config();
    config.gateway.auth.enable_jwt = true;
    config.gateway.auth.jwt_secret = "short".to_string();

    let error = match HttpServer::new(&config).await {
        Err(error) => error,
        Ok(_) => panic!("invalid auth must fail before server initialization"),
    };
    assert!(error.to_string().contains("JWT secret"));
}
