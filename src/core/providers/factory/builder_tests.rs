//! Tests for provider-specific config builders

use super::builder::*;
#[cfg(feature = "providers-extended")]
use super::cohere_builder::build_cohere_config_from_factory;
#[cfg(feature = "providers-extended")]
use super::gemini_builder::build_gemini_config_from_factory;
use super::{Provider, ProviderType, create_provider};
use crate::core::net::ProviderEndpointAccess;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());
const AWS_DEFAULT_REGION: &str = "AWS_DEFAULT_REGION";
const BEDROCK_ENV_KEYS_WITH_DEFAULT_REGION: [&str; 5] = [
    "AWS_ACCESS_KEY_ID",
    "AWS_SECRET_ACCESS_KEY",
    "AWS_SESSION_TOKEN",
    "AWS_REGION",
    AWS_DEFAULT_REGION,
];

struct EnvSnapshot {
    values: Vec<(&'static str, Option<String>)>,
}

impl EnvSnapshot {
    fn clear(keys: &[&'static str]) -> Self {
        let values = keys
            .iter()
            .map(|key| {
                let value = std::env::var(key).ok();
                unsafe { std::env::remove_var(key) };
                (*key, value)
            })
            .collect();

        Self { values }
    }
}

impl Drop for EnvSnapshot {
    fn drop(&mut self) {
        for (key, value) in &self.values {
            if let Some(value) = value {
                unsafe { std::env::set_var(key, value) };
            } else {
                unsafe { std::env::remove_var(key) };
            }
        }
    }
}

#[test]
fn test_build_openai_config_from_factory_maps_optional_fields() {
    let config = serde_json::json!({
        "api_key": "sk-test123",
        "base_url": "https://example-openai.test/v1",
        "endpoint_access": "private_network",
        "timeout": 42,
        "max_retries": 7,
        "organization": "org-test",
        "project": "proj-test",
        "headers": {
            "x-team-id": "team-1"
        },
        "custom_headers": {
            "x-request-source": "gateway"
        },
        "model_mappings": {
            "gpt-4": "gpt-4o",
            "ignored": 123
        }
    });

    let openai_config = build_openai_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("openai config should parse: {err}"));
    assert_eq!(openai_config.base.api_key.as_deref(), Some("sk-test123"));
    assert_eq!(
        openai_config.base.api_base.as_deref(),
        Some("https://example-openai.test/v1")
    );
    assert_eq!(openai_config.base.timeout, 42);
    assert_eq!(openai_config.base.max_retries, 7);
    assert_eq!(
        openai_config.base.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );
    assert_eq!(openai_config.organization.as_deref(), Some("org-test"));
    assert_eq!(openai_config.project.as_deref(), Some("proj-test"));
    assert_eq!(
        openai_config
            .base
            .headers
            .get("x-team-id")
            .map(String::as_str),
        Some("team-1")
    );
    assert_eq!(
        openai_config
            .base
            .headers
            .get("x-request-source")
            .map(String::as_str),
        Some("gateway")
    );
    assert_eq!(
        openai_config
            .model_mappings
            .get("gpt-4")
            .map(String::as_str),
        Some("gpt-4o")
    );
    assert!(!openai_config.model_mappings.contains_key("ignored"));
}

#[test]
fn test_build_anthropic_config_from_factory_maps_optional_fields() {
    let config = serde_json::json!({
        "api_key": "sk-ant-test",
        "api_base": "https://example-anthropic.test",
        "endpoint_access": "private_network",
        "api_version": "2024-01-01",
        "timeout": 99,
        "max_retries": 6,
        "retry_delay_base": 250,
        "headers": {
            "x-anthropic-a": "a"
        },
        "custom_headers": {
            "x-anthropic-b": "b"
        },
        "enable_multimodal": false,
        "enable_cache_control": false,
        "enable_computer_use": true,
        "enable_experimental": true,
        "allow_unknown_models": true,
        "models": ["mimo-v2.5", "mimo-v2.5-pro"],
        "multimodal_models": ["mimo-v2.5"]
    });

    let anthropic_config = build_anthropic_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("anthropic config should parse: {err}"));
    assert_eq!(anthropic_config.api_key.as_deref(), Some("sk-ant-test"));
    assert_eq!(anthropic_config.base_url, "https://example-anthropic.test");
    assert_eq!(anthropic_config.api_version, "2024-01-01");
    assert_eq!(anthropic_config.request_timeout, 99);
    assert_eq!(anthropic_config.connect_timeout, 10);
    assert_eq!(anthropic_config.max_retries, 6);
    assert_eq!(anthropic_config.retry_delay_base, 250);
    assert!(anthropic_config.proxy_url.is_none());
    assert_eq!(
        anthropic_config.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );
    assert_eq!(
        anthropic_config
            .custom_headers
            .get("x-anthropic-a")
            .map(String::as_str),
        Some("a")
    );
    assert_eq!(
        anthropic_config
            .custom_headers
            .get("x-anthropic-b")
            .map(String::as_str),
        Some("b")
    );
    assert!(!anthropic_config.enable_multimodal);
    assert!(!anthropic_config.enable_cache_control);
    assert!(anthropic_config.enable_computer_use);
    assert!(anthropic_config.enable_experimental);
    assert!(anthropic_config.allow_unknown_models);
    assert_eq!(
        anthropic_config.configured_models,
        vec!["mimo-v2.5".to_string(), "mimo-v2.5-pro".to_string()]
    );
    assert_eq!(
        anthropic_config.configured_multimodal_models,
        vec!["mimo-v2.5".to_string()]
    );
}

#[test]
fn test_anthropic_factory_rejects_unsafe_client_options() {
    for config in [
        serde_json::json!({
            "api_key": "sk-ant-test",
            "api_base": "https://8.8.8.8",
            "proxy": "http://localhost:8080"
        }),
        serde_json::json!({
            "api_key": "sk-ant-test",
            "api_base": "https://8.8.8.8",
            "connect_timeout": 12
        }),
    ] {
        assert!(build_anthropic_config_from_factory(&config).is_err());
    }
}

#[cfg(feature = "providers-extended")]
#[test]
fn test_gemini_factory_maps_access_and_rejects_unsafe_client_options() {
    let config = serde_json::json!({
        "api_key": "test-api-key-1234567890123456",
        "base_url": "http://127.0.0.1:18080",
        "endpoint_access": "private_network"
    });
    let gemini = build_gemini_config_from_factory(&config)
        .unwrap_or_else(|error| panic!("private Gemini config should build: {error}"));
    assert_eq!(
        gemini.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );

    for invalid in [
        serde_json::json!({
            "api_key": "test-api-key-1234567890123456",
            "base_url": "http://127.0.0.1:18080",
            "endpoint_access": "private_network",
            "proxy_url": "http://localhost:8080"
        }),
        serde_json::json!({
            "api_key": "test-api-key-1234567890123456",
            "base_url": "http://127.0.0.1:18080",
            "endpoint_access": "private_network",
            "connect_timeout": 12
        }),
    ] {
        assert!(build_gemini_config_from_factory(&invalid).is_err());
    }
}

#[test]
fn test_build_anthropic_config_from_factory_validates_compatible_models() {
    let config = serde_json::json!({
        "api_key": "xiaomi-compatible-key",
        "api_base": "https://token-plan-sgp.xiaomimimo.com/anthropic",
        "allow_unknown_models": true
    });

    let err = match build_anthropic_config_from_factory(&config) {
        Ok(_) => panic!("compatible Anthropic config should require models"),
        Err(err) => err,
    };

    assert!(format!("{err}").contains("explicit models allow-list"));
}

#[test]
fn test_build_mistral_config_from_factory_maps_optional_fields() {
    let config = serde_json::json!({
        "api_key": "mistral-key",
        "api_base": "https://example-mistral.test/v1",
        "endpoint_access": "private_network",
        "timeout": 88,
        "max_retries": 4
    });

    let mistral_config = build_mistral_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("mistral config should parse: {err}"));
    assert_eq!(mistral_config.api_key, "mistral-key");
    assert_eq!(mistral_config.api_base, "https://example-mistral.test/v1");
    assert_eq!(mistral_config.timeout_seconds, 88);
    assert_eq!(mistral_config.max_retries, 4);
    assert_eq!(
        mistral_config.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );
}

#[cfg(feature = "providers-extended")]
#[test]
fn test_build_cohere_config_from_factory_maps_endpoint_access() {
    let config = serde_json::json!({
        "api_key": "cohere-key",
        "base_url": "https://example-cohere.test",
        "endpoint_access": "private_network",
        "timeout": 73,
        "max_retries": 4
    });

    let cohere_config = build_cohere_config_from_factory(&config)
        .unwrap_or_else(|error| panic!("Cohere config should parse: {error}"));
    assert_eq!(cohere_config.api_base, "https://example-cohere.test");
    assert_eq!(cohere_config.timeout_seconds, 73);
    assert_eq!(cohere_config.max_retries, 4);
    assert_eq!(
        cohere_config.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );
}

#[test]
fn test_build_cloudflare_config_from_factory_maps_alias_and_optional_fields() {
    let config = serde_json::json!({
        "organization": "acct-xyz",
        "api_key": "token-xyz",
        "base_url": "https://cf.example.test",
        "timeout": 77,
        "max_retries": 5,
        "debug": true
    });

    let cf_config = build_cloudflare_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("cloudflare config should parse: {err}"));
    assert_eq!(cf_config.account_id.as_deref(), Some("acct-xyz"));
    assert_eq!(cf_config.api_token.as_deref(), Some("token-xyz"));
    assert_eq!(
        cf_config.api_base.as_deref(),
        Some("https://cf.example.test")
    );
    assert_eq!(cf_config.timeout, 77);
    assert_eq!(cf_config.max_retries, 5);
    assert!(cf_config.debug);
}

#[test]
fn test_build_openai_like_config_from_factory_maps_optional_fields() {
    let config = serde_json::json!({
        "base_url": "https://openai-like.example.test/v1",
        "api_key": "sk-openai-like",
        "endpoint_access": "private_network",
        "provider_name": "custom-like",
        "timeout": 55,
        "max_retries": 4,
        "model_prefix": "prefix/",
        "default_model": "gpt-4o-mini",
        "pass_through_params": false,
        "skip_api_key": true,
        "organization": "org-like",
        "api_version": "2024-12-01",
        "headers": {
            "x-base-header": "base"
        },
        "custom_headers": {
            "x-custom-header": "custom"
        }
    });

    let oai_like = build_openai_like_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("openai_like config should parse: {err}"));

    assert_eq!(
        oai_like.base.api_base.as_deref(),
        Some("https://openai-like.example.test/v1")
    );
    assert_eq!(oai_like.base.api_key.as_deref(), Some("sk-openai-like"));
    assert_eq!(oai_like.provider_name, "custom-like");
    assert_eq!(oai_like.base.timeout, 55);
    assert_eq!(oai_like.base.max_retries, 4);
    assert_eq!(
        oai_like.base.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );
    assert_eq!(oai_like.model_prefix.as_deref(), Some("prefix/"));
    assert_eq!(oai_like.default_model.as_deref(), Some("gpt-4o-mini"));
    assert!(!oai_like.pass_through_params);
    assert!(oai_like.skip_api_key);
    assert_eq!(oai_like.base.organization.as_deref(), Some("org-like"));
    assert_eq!(oai_like.base.api_version.as_deref(), Some("2024-12-01"));
    assert_eq!(
        oai_like
            .base
            .headers
            .get("x-base-header")
            .map(String::as_str),
        Some("base")
    );
    assert_eq!(
        oai_like
            .custom_headers
            .get("x-custom-header")
            .map(String::as_str),
        Some("custom")
    );
}

#[test]
fn test_build_openai_like_config_from_factory_requires_api_base() {
    let config = serde_json::json!({
        "api_key": "sk-openai-like"
    });

    let err = build_openai_like_config_from_factory(&config)
        .err()
        .unwrap_or_else(|| panic!("missing base_url should return an error"));
    assert!(err.to_string().contains("base_url"));
}

#[test]
fn test_factory_endpoint_access_defaults_and_rejects_invalid_values() {
    let openai = build_openai_config_from_factory(&serde_json::json!({
        "api_key": "sk-test"
    }))
    .unwrap_or_else(|error| panic!("default OpenAI access should parse: {error}"));
    let openai_like = build_openai_like_config_from_factory(&serde_json::json!({
        "base_url": "https://api.example.test/v1",
        "skip_api_key": true
    }))
    .unwrap_or_else(|error| panic!("default OpenAI-like access should parse: {error}"));
    assert_eq!(
        openai.base.endpoint_access,
        ProviderEndpointAccess::PublicOnly
    );
    assert_eq!(
        openai_like.base.endpoint_access,
        ProviderEndpointAccess::PublicOnly
    );

    for invalid in [
        serde_json::json!(""),
        serde_json::json!("private"),
        serde_json::json!(true),
    ] {
        let config = serde_json::json!({"endpoint_access": invalid});
        let error = config_endpoint_access(&config, "test")
            .err()
            .unwrap_or_else(|| panic!("invalid endpoint_access must fail"));
        assert!(error.to_string().contains("invalid endpoint_access"));
    }
}

#[tokio::test]
async fn test_gateway_and_direct_registry_endpoint_access_contract() {
    let mut config = crate::config::models::provider::ProviderConfig {
        name: "test-openai-like".to_string(),
        provider_type: "openai_compatible".to_string(),
        base_url: Some("https://api.example.com/v1".to_string()),
        ..Default::default()
    };
    config
        .settings
        .insert("skip_api_key".to_string(), serde_json::json!(true));

    let provider = create_provider(config.clone())
        .await
        .unwrap_or_else(|error| panic!("public-only Gateway config should work: {error}"));
    let Provider::OpenAILike(provider) = provider else {
        panic!("expected OpenAILike provider");
    };
    assert_eq!(
        provider.config().base.endpoint_access,
        ProviderEndpointAccess::PublicOnly
    );

    config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config.base_url = Some("http://127.0.0.1/v1".to_string());
    assert!(create_provider(config).await.is_ok());

    let mut settings_override = crate::config::models::provider::ProviderConfig {
        name: "openai".to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-test".to_string(),
        ..Default::default()
    };
    settings_override.settings.insert(
        "endpoint_access".to_string(),
        serde_json::json!("private_network"),
    );
    let error = create_provider(settings_override)
        .await
        .err()
        .unwrap_or_else(|| panic!("settings must not override endpoint access"));
    assert!(error.to_string().contains("top-level"));

    let provider = Provider::from_config_async(
        ProviderType::Custom("together".to_string()),
        serde_json::json!({"api_key": "x", "base_url": "http://127.0.0.1/v1",
            "endpoint_access": "private_network"}),
    )
    .await
    .unwrap_or_else(|error| panic!("catalog-backed custom type should activate: {error}"));
    let Provider::OpenAILike(provider) = provider else {
        panic!("expected catalog-backed OpenAILike provider");
    };
    assert_eq!(
        provider.config().base.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );
}

#[cfg(not(feature = "providers-extra"))]
#[test]
fn test_build_azure_openai_like_fallback_configs_map_fields() {
    let azure = serde_json::json!({
        "api_key": "azure-key",
        "azure_endpoint": "https://example-resource.openai.azure.com/openai/deployments/prod",
        "api_version": "2024-03-01",
        "timeout": 66,
        "max_retries": 4,
        "headers": {
            "x-azure-base": "base"
        },
        "custom_headers": {
            "x-azure-custom": "custom"
        }
    });
    let azure_config = build_azure_openai_like_config_from_factory(&azure)
        .unwrap_or_else(|err| panic!("azure fallback config should parse: {err}"));

    assert_eq!(azure_config.provider_name, "azure");
    assert_eq!(azure_config.base.api_key.as_deref(), Some("azure-key"));
    assert_eq!(
        azure_config.base.api_base.as_deref(),
        Some("https://example-resource.openai.azure.com/openai/deployments/prod")
    );
    assert_eq!(azure_config.base.api_version.as_deref(), Some("2024-03-01"));
    assert_eq!(azure_config.base.timeout, 66);
    assert_eq!(azure_config.base.max_retries, 4);
    assert_eq!(
        azure_config
            .base
            .headers
            .get("x-azure-base")
            .map(String::as_str),
        Some("base")
    );
    assert_eq!(
        azure_config
            .custom_headers
            .get("x-azure-custom")
            .map(String::as_str),
        Some("custom")
    );

    let azure_ai = serde_json::json!({
        "api_key": "azure-ai-key",
        "azure_ai_endpoint": "https://example-resource.services.ai.azure.com/models",
        "api_version": "2024-05-01-preview"
    });
    let azure_ai_config = build_azure_ai_openai_like_config_from_factory(&azure_ai)
        .unwrap_or_else(|err| panic!("azure_ai fallback config should parse: {err}"));

    assert_eq!(azure_ai_config.provider_name, "azure_ai");
    assert_eq!(
        azure_ai_config.base.api_key.as_deref(),
        Some("azure-ai-key")
    );
    assert_eq!(
        azure_ai_config.base.api_base.as_deref(),
        Some("https://example-resource.services.ai.azure.com/models")
    );
    assert_eq!(
        azure_ai_config.base.api_version.as_deref(),
        Some("2024-05-01-preview")
    );
}

#[cfg(not(feature = "providers-extra"))]
#[test]
fn test_azure_openai_like_fallback_rejects_bare_resource_endpoints() {
    let azure = serde_json::json!({
        "api_key": "azure-key",
        "azure_endpoint": "https://example-resource.openai.azure.com",
        "deployment_name": "prod"
    });
    let err = build_azure_openai_like_config_from_factory(&azure)
        .err()
        .unwrap_or_else(|| panic!("bare Azure endpoint should be rejected"));
    assert!(err.to_string().contains("providers-extra"));

    let azure_ai = serde_json::json!({
        "api_key": "azure-ai-key",
        "azure_ai_endpoint": "https://example-resource.services.ai.azure.com"
    });
    let err = build_azure_ai_openai_like_config_from_factory(&azure_ai)
        .err()
        .unwrap_or_else(|| panic!("bare Azure AI endpoint should be rejected"));
    assert!(err.to_string().contains("providers-extra"));
}

#[cfg(feature = "providers-extra")]
#[test]
fn test_build_azure_config_from_factory_maps_native_fields() {
    let config = serde_json::json!({
        "api_key": "azure-key",
        "azure_endpoint": "https://example-resource.openai.azure.com",
        "endpoint_access": "private_network",
        "deployment_name": "gpt-4o-prod",
        "api_version": "2024-03-01",
        "timeout": 31,
        "max_retries": 6,
        "headers": {
            "x-azure-base": "base"
        },
        "custom_headers": {
            "x-azure-custom": "custom"
        }
    });

    let azure_config = build_azure_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("azure config should parse: {err}"));

    assert_eq!(azure_config.api_key.as_deref(), Some("azure-key"));
    assert_eq!(
        azure_config.azure_endpoint.as_deref(),
        Some("https://example-resource.openai.azure.com")
    );
    assert_eq!(azure_config.deployment_name.as_deref(), Some("gpt-4o-prod"));
    assert_eq!(azure_config.api_version, "2024-03-01");
    assert_eq!(azure_config.timeout, 31);
    assert_eq!(azure_config.max_retries, 6);
    assert_eq!(
        azure_config.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );
    assert_eq!(
        azure_config
            .custom_headers
            .get("x-azure-base")
            .map(String::as_str),
        Some("base")
    );
    assert_eq!(
        azure_config
            .custom_headers
            .get("x-azure-custom")
            .map(String::as_str),
        Some("custom")
    );
}

#[cfg(feature = "providers-extra")]
#[test]
fn test_build_azure_ai_config_from_factory_maps_native_fields() {
    let config = serde_json::json!({
        "api_key": "azure-ai-key",
        "azure_ai_endpoint": "https://example-resource.services.ai.azure.com",
        "api_version": "2024-05-01-preview",
        "timeout": 44,
        "max_retries": 2,
        "headers": {
            "x-azure-ai-base": "base"
        },
        "custom_headers": {
            "x-azure-ai-custom": "custom"
        }
    });

    let azure_ai_config = build_azure_ai_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("azure_ai config should parse: {err}"));

    assert_eq!(
        azure_ai_config.base.api_key.as_deref(),
        Some("azure-ai-key")
    );
    assert_eq!(
        azure_ai_config.base.api_base.as_deref(),
        Some("https://example-resource.services.ai.azure.com")
    );
    assert_eq!(
        azure_ai_config.base.api_version.as_deref(),
        Some("2024-05-01-preview")
    );
    assert_eq!(azure_ai_config.base.timeout, 44);
    assert_eq!(azure_ai_config.base.max_retries, 2);
    assert_eq!(
        azure_ai_config
            .base
            .headers
            .get("x-azure-ai-base")
            .map(String::as_str),
        Some("base")
    );
    assert_eq!(
        azure_ai_config
            .base
            .headers
            .get("x-azure-ai-custom")
            .map(String::as_str),
        Some("custom")
    );
}

#[cfg(feature = "providers-extra")]
#[test]
fn test_vertex_factory_maps_endpoint_access() {
    let config = serde_json::json!({
        "project_id": "project",
        "location": "us-central1",
        "base_url": "http://127.0.0.1:18080",
        "endpoint_access": "private_network",
        "access_token": "token"
    });
    let vertex = build_vertex_ai_config_from_factory(&config)
        .unwrap_or_else(|error| panic!("private Vertex config should build: {error}"));
    assert_eq!(
        vertex.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );
}

#[test]
fn test_env_str_any_skips_empty_values() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _snapshot = EnvSnapshot::clear(&["AWS_REGION", AWS_DEFAULT_REGION]);

    unsafe {
        std::env::set_var("AWS_REGION", "");
        std::env::set_var(AWS_DEFAULT_REGION, "us-west-2");
    }

    assert_eq!(
        env_str_any(&["AWS_REGION", AWS_DEFAULT_REGION]).as_deref(),
        Some("us-west-2")
    );
}

#[test]
fn test_build_bedrock_config_defaults_region() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _snapshot = EnvSnapshot::clear(&BEDROCK_ENV_KEYS_WITH_DEFAULT_REGION);

    let config = serde_json::json!({
        "aws_access_key_id": "AKIATEST123456789012",
        "aws_secret_access_key": "test-secret-key",
        "endpoint_access": "private_network"
    });

    let bedrock_config = build_bedrock_config_from_factory(&config)
        .unwrap_or_else(|err| panic!("bedrock config should parse: {err}"));

    assert_eq!(bedrock_config.aws_region, "us-east-1");
    assert_eq!(bedrock_config.aws_access_key_id, "AKIATEST123456789012");
    assert_eq!(bedrock_config.aws_secret_access_key, "test-secret-key");
    assert_eq!(
        bedrock_config.endpoint_access,
        ProviderEndpointAccess::PrivateNetwork
    );
}

#[test]
fn test_build_bedrock_config_rejects_missing_credentials() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _snapshot = EnvSnapshot::clear(&BEDROCK_ENV_KEYS_WITH_DEFAULT_REGION);

    let err = build_bedrock_config_from_factory(&serde_json::json!({}))
        .err()
        .unwrap_or_else(|| panic!("missing credentials should return an error"));
    assert!(err.to_string().contains("aws_access_key_id"));

    let err = build_bedrock_config_from_factory(&serde_json::json!({
        "aws_access_key_id": "AKIATEST123456789012"
    }))
    .err()
    .unwrap_or_else(|| panic!("missing secret should return an error"));
    assert!(err.to_string().contains("aws_secret_access_key"));
}

#[test]
fn test_build_bedrock_config_skips_empty_env_values() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _snapshot = EnvSnapshot::clear(&BEDROCK_ENV_KEYS_WITH_DEFAULT_REGION);

    unsafe {
        std::env::set_var("AWS_ACCESS_KEY_ID", "AKIATEST123456789012");
        std::env::set_var("AWS_SECRET_ACCESS_KEY", "test-secret-key");
        std::env::set_var("AWS_SESSION_TOKEN", "");
        std::env::set_var("AWS_REGION", "");
        std::env::set_var(AWS_DEFAULT_REGION, "us-west-2");
    }

    let bedrock_config = build_bedrock_config_from_factory(&serde_json::json!({}))
        .unwrap_or_else(|err| panic!("bedrock config should parse from env: {err}"));

    assert_eq!(bedrock_config.aws_access_key_id, "AKIATEST123456789012");
    assert_eq!(bedrock_config.aws_secret_access_key, "test-secret-key");
    assert_eq!(bedrock_config.aws_region, "us-west-2");
    assert!(bedrock_config.aws_session_token.is_none());
}

#[test]
fn test_build_bedrock_config_skips_empty_config_session_token() {
    let _guard = ENV_LOCK.lock().unwrap();
    let _snapshot = EnvSnapshot::clear(&BEDROCK_ENV_KEYS_WITH_DEFAULT_REGION);

    unsafe {
        std::env::set_var("AWS_SESSION_TOKEN", "env-session-token");
    }

    let bedrock_config = build_bedrock_config_from_factory(&serde_json::json!({
        "aws_access_key_id": "AKIATEST123456789012",
        "aws_secret_access_key": "test-secret-key",
        "aws_session_token": "",
        "aws_region": "us-west-2"
    }))
    .unwrap_or_else(|err| panic!("bedrock config should parse: {err}"));

    assert_eq!(
        bedrock_config.aws_session_token.as_deref(),
        Some("env-session-token")
    );
}
