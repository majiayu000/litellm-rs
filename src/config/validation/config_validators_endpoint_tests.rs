//! Provider endpoint access and allow-list validation tests.

use super::*;
use crate::core::net::ProviderEndpointAccess;

fn public_provider() -> ProviderConfig {
    ProviderConfig {
        name: "staged".to_string(),
        provider_type: "openai_compatible".to_string(),
        api_key: "sk-test".to_string(),
        base_url: Some("https://8.8.8.8/v1".to_string()),
        ..Default::default()
    }
}

#[test]
fn endpoint_access_is_top_level_and_private_is_provider_gated() {
    let mut config = public_provider();
    assert!(Validate::validate(&config).is_ok());

    config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    assert!(Validate::validate(&config).is_ok());

    config.base_url = Some("http://127.0.0.1:18080/v1".to_string());
    assert!(Validate::validate(&config).is_ok());

    config.base_url = None;
    for (provider_type, allowed) in [("bedrock", true), ("vllm", true), ("openrouter", false)] {
        config.provider_type = provider_type.to_string();
        assert_eq!(Validate::validate(&config).is_ok(), allowed);
    }
    config.provider_type = "cloudflare".to_string();
    assert!(
        Validate::validate(&config)
            .unwrap_err()
            .contains("not policy-wired")
    );

    config.provider_type = "openai_compatible".to_string();
    config.base_url = Some("http://127.0.0.1:18080/v1".to_string());
    config.endpoint_access = ProviderEndpointAccess::PublicOnly;
    assert!(Validate::validate(&config).is_err());
    config.base_url = Some("wss://8.8.8.8/v1".to_string());
    assert!(Validate::validate(&config).is_err());
    config.base_url = Some(" ".to_string());
    assert!(Validate::validate(&config).is_err());
    config.base_url = Some("https://8.8.8.8/v1".to_string());
    config.settings.insert(
        "endpoint_access".to_string(),
        serde_json::json!("private_network"),
    );
    assert!(
        Validate::validate(&config)
            .unwrap_err()
            .contains("top-level")
    );
}

#[cfg(feature = "providers-extended")]
#[test]
fn keyless_ollama_startup_validation_preserves_endpoint_policy() {
    let mut config = public_provider();
    config.name = "local-ollama".to_string();
    config.provider_type = "ollama".to_string();
    config.api_key.clear();
    config.base_url = None;
    assert!(Validate::validate(&config).is_ok());

    config.base_url = Some("http://127.0.0.1:11434".to_string());
    let error = Validate::validate(&config)
        .expect_err("explicit public-only loopback must remain fail-closed");
    assert!(error.contains("private or reserved"), "{error}");

    config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    assert!(Validate::validate(&config).is_ok());
}

#[test]
fn health_check_runtime_honors_endpoint_access() {
    let mut config = public_provider();
    config.base_url = Some("http://127.0.0.1:18080/v1".to_string());
    config.health_check.endpoint = Some("health".to_string());
    config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;

    assert!(config.validate_health_check_runtime().is_ok());

    config.endpoint_access = ProviderEndpointAccess::PublicOnly;
    assert!(config.validate_health_check_runtime().is_err());

    config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config.health_check.endpoint = Some("ws://127.0.0.1:18080/health".to_string());
    let error = config
        .validate_health_check_runtime()
        .expect_err("health checks must remain limited to HTTP transports");
    assert!(error.contains("http:// or https://"));
}

#[test]
fn azure_endpoint_aliases_are_provider_specific_endpoint_sources() {
    for (provider_type, key, endpoint) in [
        (
            "azure",
            "endpoint",
            "http://127.0.0.1:18080/openai/deployments/test",
        ),
        (
            "azure",
            "azure_endpoint",
            "http://127.0.0.1:18080/openai/deployments/test",
        ),
        ("azure_ai", "endpoint", "http://127.0.0.1:18080/models"),
        (
            "azure_ai",
            "azure_ai_endpoint",
            "http://127.0.0.1:18080/models",
        ),
    ] {
        let mut config = public_provider();
        config.provider_type = provider_type.to_string();
        config.base_url = None;
        config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
        config.settings.insert(key.to_string(), endpoint.into());
        assert!(Validate::validate(&config).is_ok(), "{provider_type}.{key}");
    }

    let mut unrelated = public_provider();
    unrelated.base_url = None;
    unrelated.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    unrelated
        .settings
        .insert("endpoint".to_string(), "http://127.0.0.1:18080/v1".into());
    let error = Validate::validate(&unrelated).expect_err("OpenAI aliases must stay closed");
    assert!(error.contains("requires a base URL"), "{error}");

    let mut invalid = public_provider();
    invalid.provider_type = "azure".to_string();
    invalid.base_url = None;
    invalid
        .settings
        .insert("azure_endpoint".to_string(), serde_json::json!(42));
    let error = Validate::validate(&invalid).expect_err("invalid alias must fail");
    assert!(error.contains("must be a string"), "azure: {error}");
}

#[test]
fn provider_selector_normalization_preserves_endpoint_alias_policy() {
    let mut config = public_provider();
    config.provider_type = " Azure ".to_string();
    config.base_url = None;
    config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config.settings.insert(
        "azure_endpoint".to_string(),
        "http://127.0.0.1:18080/openai/deployments/test".into(),
    );

    assert!(Validate::validate(&config).is_ok());
}

#[cfg(feature = "providers-extra")]
#[test]
fn vertex_endpoint_alias_is_a_provider_specific_endpoint_source() {
    let mut config = public_provider();
    config.provider_type = "vertex_ai".to_string();
    config.base_url = None;
    config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config
        .settings
        .insert("endpoint".to_string(), "http://127.0.0.1:18080/v1".into());
    assert!(Validate::validate(&config).is_ok());

    for value in [serde_json::json!(42), serde_json::json!(" ")] {
        config.settings.insert("endpoint".to_string(), value);
        let error = Validate::validate(&config).expect_err("invalid alias must fail");
        assert!(error.contains("must be a string"), "{error}");
    }
}

#[test]
fn private_health_check_is_bound_to_the_provider_authority() {
    let mut config = public_provider();
    config.base_url = Some("http://127.0.0.1:18080/v1".to_string());
    config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    for endpoint in ["health", "http://127.0.0.1:18080/health"] {
        config.health_check.endpoint = Some(endpoint.to_string());
        assert!(config.validate_health_check_runtime().is_ok(), "{endpoint}");
    }
    for endpoint in [
        "http://127.0.0.2:18080/health",
        "http://127.0.0.1:18081/health",
        "https://127.0.0.1:18080/health",
    ] {
        config.health_check.endpoint = Some(endpoint.to_string());
        let error = config
            .validate_health_check_runtime()
            .expect_err("private health authority must not expand");
        assert!(error.contains("authority"), "{endpoint}: {error}");
    }

    config.base_url = None;
    config.provider_type = "azure".to_string();
    config.settings.insert(
        "azure_endpoint".to_string(),
        "http://127.0.0.1:18080/openai/deployments/test".into(),
    );
    config.health_check.endpoint = Some("health".to_string());
    assert!(config.validate_health_check_runtime().is_ok());
}

#[test]
fn official_openai_endpoint_cannot_receive_private_access() {
    for provider_type in ["openai", "openai_compatible"] {
        for endpoint_key in ["base_url", "api_base"] {
            let mut config = public_provider();
            config.provider_type = provider_type.to_string();
            config.base_url = None;
            config.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
            if endpoint_key == "base_url" {
                config.base_url = Some("https://api.openai.com/v1".to_string());
            } else {
                config
                    .settings
                    .insert(endpoint_key.to_string(), "https://api.openai.com/v1".into());
            }
            let error = Validate::validate(&config).expect_err("official OpenAI must stay public");
            assert!(
                error.contains("official OpenAI"),
                "{provider_type}.{endpoint_key}: {error}"
            );
        }
    }
}

// ---- #1503: endpoint_allow_cidrs ----

fn fake_ip_provider(provider_type: &str, allow: &[&str]) -> ProviderConfig {
    ProviderConfig {
        provider_type: provider_type.to_string(),
        base_url: Some("https://198.18.0.7/v1".to_string()),
        endpoint_allow_cidrs: crate::core::net::EndpointAllowCidrs::parse(allow).unwrap(),
        ..public_provider()
    }
}

#[test]
fn allow_cidrs_parse_at_config_load_and_invalid_cidr_is_an_error() {
    let parsed: ProviderConfig = serde_yml::from_str(
        "name: proxied\nprovider_type: openai_compatible\napi_key: sk-test\n\
         base_url: https://api.example.com/v1\nendpoint_allow_cidrs: [198.18.0.0/15]\n",
    )
    .expect("valid allow list should load");
    assert_eq!(
        parsed.endpoint_allow_cidrs.as_slice()[0].to_string(),
        "198.18.0.0/15"
    );
    assert!(Validate::validate(&parsed).is_ok());

    for invalid in [
        "198.18.0.1/15",
        "0.0.0.0/0",
        "198.18.0.0/40",
        "198.18.0.0/+15",
        "169.254.0.0/16",
        "fake-ip",
    ] {
        let yaml = format!(
            "name: proxied\nprovider_type: openai_compatible\napi_key: sk-test\n\
             endpoint_allow_cidrs: [\"{invalid}\"]\n"
        );
        let error = serde_yml::from_str::<ProviderConfig>(&yaml)
            .expect_err("invalid CIDR must fail config load")
            .to_string();
        assert!(
            error.contains("CIDR") || error.contains("/0"),
            "{invalid}: {error}"
        );
    }
    // Absent stays absent when serialized, so existing configs round-trip unchanged.
    let json = serde_json::to_value(public_provider()).unwrap();
    assert!(json.get("endpoint_allow_cidrs").is_none());
}

#[test]
fn allow_cidrs_admit_a_fake_ip_endpoint_only_when_listed() {
    let error = Validate::validate(&fake_ip_provider("openai_compatible", &[]))
        .expect_err("default must still reject the fake-IP range");
    assert!(
        error.contains("198.18.0.7") && error.contains("endpoint_allow_cidrs"),
        "{error}"
    );
    // Provider types that cannot honour the allow list are pointed at openai_compatible instead.
    let error = Validate::validate(&fake_ip_provider("anthropic", &[]))
        .expect_err("default must still reject the fake-IP range");
    assert!(
        error.contains("198.18.0.7")
            && error.contains("does not support `endpoint_allow_cidrs`")
            && error.contains("provider_type: openai_compatible"),
        "{error}"
    );
    assert!(!error.contains("add a CIDR"), "{error}");
    for provider_type in ["openai_compatible", "openai", "deepseek"] {
        let mut config = fake_ip_provider(provider_type, &["198.18.0.0/15"]);
        if provider_type == "openai" {
            config.api_key = "sk-test".to_string();
        }
        assert!(Validate::validate(&config).is_ok(), "{provider_type}");
    }
    let metadata = ProviderConfig {
        base_url: Some("http://169.254.169.254/v1".to_string()),
        ..fake_ip_provider("openai_compatible", &["168.0.0.0/7"])
    };
    let error = Validate::validate(&metadata).expect_err("metadata must stay blocked");
    assert!(error.contains("always blocked"), "{error}");
    let alibaba = ProviderConfig {
        base_url: Some("http://100.100.100.200/v1".to_string()),
        ..fake_ip_provider("openai_compatible", &["198.18.0.0/15", "100.64.0.0/10"])
    };
    let error = Validate::validate(&alibaba).expect_err("Alibaba metadata must stay blocked");
    assert!(error.contains("always blocked"), "{error}");
}

#[test]
fn allow_cidrs_are_top_level_and_rejected_where_the_runtime_cannot_honour_them() {
    for provider_type in ["anthropic", "bedrock"] {
        let config = ProviderConfig {
            base_url: None,
            ..fake_ip_provider(provider_type, &["198.18.0.0/15"])
        };
        let error = Validate::validate(&config).expect_err("unsupported runtime must reject");
        assert!(
            error.contains("endpoint_allow_cidrs is not supported"),
            "{provider_type}: {error}"
        );
    }
    let mut nested = public_provider();
    nested.settings.insert(
        "endpoint_allow_cidrs".to_string(),
        serde_json::json!(["198.18.0.0/15"]),
    );
    let error = Validate::validate(&nested).expect_err("nested allow list must be rejected");
    assert!(error.contains("top-level"), "{error}");
}

#[test]
fn health_check_endpoint_uses_the_same_allow_list() {
    let mut config = fake_ip_provider("openai_compatible", &["198.18.0.0/15"]);
    config.health_check.endpoint = Some("https://198.18.0.7/health".to_string());
    assert!(config.validate_health_check_runtime().is_ok());
    config.endpoint_allow_cidrs = Default::default();
    let error = config
        .validate_health_check_runtime()
        .expect_err("health endpoint must be rejected without the allow list");
    assert!(error.contains("endpoint_allow_cidrs"), "{error}");
}
