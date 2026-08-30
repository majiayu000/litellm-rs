use super::create_provider;
use crate::config::Validate;
use crate::core::providers::Provider;
use crate::core::providers::ProviderError;

#[tokio::test]
async fn sagemaker_standard_provider_config_validates_and_builds() {
    let config = crate::config::models::provider::ProviderConfig {
        name: "sagemaker-chat".to_string(),
        provider_type: "sagemaker".to_string(),
        models: vec!["tenant-chat".to_string()],
        settings: serde_json::from_value(serde_json::json!({
            "aws_access_key_id": "AKIATEST",
            "aws_secret_access_key": "secret-test",
            "aws_session_token": "session-test",
            "region": "us-east-1",
            "endpoint_name": "tenant-chat",
            "payload_transformer": "open_ai_chat"
        }))
        .expect("settings object"),
        ..Default::default()
    };

    let mut with_unused_api_key = config.clone();
    with_unused_api_key.api_key = "unused-top-level-key".to_string();

    config.validate().expect("standard config should validate");
    let provider = create_provider(config)
        .await
        .expect("standard config should build SageMaker");
    assert!(matches!(provider, Provider::Enterprise(_)));
    create_provider(with_unused_api_key)
        .await
        .expect("irrelevant top-level API key should be discarded");
}

#[tokio::test]
async fn snowflake_organization_alias_builds_through_standard_config() {
    let config = crate::config::models::provider::ProviderConfig {
        name: "snowflake".to_string(),
        provider_type: "snowflake".to_string(),
        api_key: "oauth-token".to_string(),
        organization: Some("org-account".to_string()),
        settings: serde_json::from_value(serde_json::json!({
            "token_type": "OAUTH"
        }))
        .expect("settings object"),
        ..Default::default()
    };

    config.validate().expect("standard config should validate");
    let provider = create_provider(config)
        .await
        .expect("organization alias should build Snowflake");
    assert!(matches!(provider, Provider::Enterprise(_)));
}

#[tokio::test]
async fn oci_standard_provider_config_validates_and_builds() {
    let config = crate::config::models::provider::ProviderConfig {
        name: "oci".to_string(),
        provider_type: "oci".to_string(),
        api_key: "oci-compatible-token".to_string(),
        settings: serde_json::from_value(serde_json::json!({
            "region": "us-chicago-1",
            "api_mode": "open_ai_compatible"
        }))
        .expect("settings object"),
        ..Default::default()
    };

    config.validate().expect("standard config should validate");
    let provider = create_provider(config)
        .await
        .expect("standard config should build OCI compatible runtime");
    assert!(matches!(provider, Provider::Enterprise(_)));
}

#[tokio::test]
async fn watsonx_requires_explicit_access_token_in_settings() {
    let explicit = crate::config::models::provider::ProviderConfig {
        name: "watsonx".to_string(),
        provider_type: "watsonx".to_string(),
        api_version: Some("2025-01-01".to_string()),
        project: Some("project-id".to_string()),
        settings: serde_json::from_value(serde_json::json!({
            "access_token": "iam-access-token",
            "region": "us-south"
        }))
        .expect("settings object"),
        ..Default::default()
    };
    explicit
        .validate()
        .expect("explicit access token config should validate");
    assert!(matches!(
        create_provider(explicit).await,
        Ok(Provider::Enterprise(_))
    ));

    let ambiguous = crate::config::models::provider::ProviderConfig {
        name: "watsonx".to_string(),
        provider_type: "watsonx".to_string(),
        api_key: "ibm-api-key-is-not-an-access-token".to_string(),
        project: Some("project-id".to_string()),
        settings: serde_json::from_value(serde_json::json!({
            "region": "us-south"
        }))
        .expect("settings object"),
        ..Default::default()
    };
    let error = create_provider(ambiguous)
        .await
        .expect_err("api_key must not be reinterpreted as an IAM access token");
    assert!(error.to_string().contains("access_token"));
}

#[tokio::test]
async fn enterprise_api_base_alias_builds_and_top_level_base_url_wins() {
    let cases = [
        ("databricks", "databricks-token", serde_json::json!({})),
        (
            "oci",
            "oci-token",
            serde_json::json!({
                "region": "us-chicago-1",
                "api_mode": "open_ai_compatible"
            }),
        ),
        (
            "watsonx",
            "",
            serde_json::json!({
                "access_token": "iam-access-token",
                "project_id": "project-id",
                "region": "us-south"
            }),
        ),
        (
            "sagemaker",
            "",
            serde_json::json!({
                "aws_access_key_id": "AKIATEST",
                "aws_secret_access_key": "secret-test",
                "region": "us-east-1",
                "endpoint_name": "tenant-chat",
                "payload_transformer": "open_ai_chat"
            }),
        ),
    ];

    for (provider_type, api_key, settings) in cases {
        let mut alias_only = crate::config::models::provider::ProviderConfig {
            name: format!("{provider_type}-alias"),
            provider_type: provider_type.to_string(),
            api_key: api_key.to_string(),
            endpoint_access: crate::core::net::ProviderEndpointAccess::PrivateNetwork,
            settings: serde_json::from_value(settings.clone()).expect("settings object"),
            ..Default::default()
        };
        alias_only.settings.insert(
            "api_base".to_string(),
            serde_json::json!("https://enterprise.example.com"),
        );
        create_provider(alias_only)
            .await
            .unwrap_or_else(|error| panic!("{provider_type} api_base alias should build: {error}"));

        let mut top_level = crate::config::models::provider::ProviderConfig {
            name: format!("{provider_type}-precedence"),
            provider_type: provider_type.to_string(),
            api_key: api_key.to_string(),
            base_url: Some("https://enterprise.example.com".to_string()),
            settings: serde_json::from_value(settings).expect("settings object"),
            ..Default::default()
        };
        top_level.settings.insert(
            "api_base".to_string(),
            serde_json::json!("https://user:password@ignored.example.com"),
        );
        create_provider(top_level).await.unwrap_or_else(|error| {
            panic!("{provider_type} top-level base_url should win: {error}")
        });
    }
}

#[tokio::test]
async fn reports_unknown_custom_provider() {
    let config = crate::config::models::provider::ProviderConfig {
        name: "my-custom-provider".to_string(),
        provider_type: "".to_string(),
        api_key: "test-key".to_string(),
        ..Default::default()
    };

    let error = create_provider(config)
        .await
        .expect_err("Expected unknown custom provider to fail");
    // Unknown provider strings produce InvalidRequest at selector parsing time.
    assert!(
        matches!(error, ProviderError::InvalidRequest { .. }),
        "Expected InvalidRequest error, got {error}"
    );
    assert!(
        error.to_string().contains("my-custom-provider"),
        "Expected custom provider name in error, got {error}"
    );
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn creates_native_ollama_from_gateway_config() {
    let config = crate::config::models::provider::ProviderConfig {
        name: "local-ollama".to_string(),
        provider_type: "ollama".to_string(),
        base_url: Some("http://127.0.0.1:11434".to_string()),
        endpoint_access: crate::core::net::ProviderEndpointAccess::PrivateNetwork,
        models: vec!["llama3:8b".to_string()],
        ..Default::default()
    };

    let provider = create_provider(config)
        .await
        .unwrap_or_else(|error| panic!("gateway config should create native Ollama: {error}"));
    assert!(matches!(provider, Provider::Ollama(_)));
}
