#[cfg(feature = "providers-extended")]
mod matrix {
    use super::super::*;
    use crate::core::providers::unified_provider::ProviderError;
    use std::sync::{Mutex, MutexGuard};
    static ENV_LOCK: Mutex<()> = Mutex::new(());
    #[rustfmt::skip]
    const ENVS: &[&str] = &[
        "MIMO_API_KEY", "XIAOMI_API_KEY", "CLOUDFLARE_API_TOKEN",
        "REPLICATE_API_TOKEN", "REPLICATE_API_KEY", "FAL_AI_API_KEY",
        "COHERE_API_KEY", "GEMINI_API_KEY", "GOOGLE_API_KEY",
        "PERPLEXITY_API_KEY", "GITHUB_TOKEN",
    ];
    const GEM_TOP: &str = "gem-top-test-api-key-12345678901234567890";
    const GEM_SETTINGS: &str = "gem-settings-test-api-key-12345678901234567890";
    const GEM_GOOGLE: &str = "gem-google-test-api-key-12345678901234567890";
    const GEM_SETTING: &str = "gem-setting-test-api-key-12345678901234567890";
    const GEM_ENV: &str = "gem-env-test-api-key-12345678901234567890";
    const GEM_GOOGLE_ENV: &str = "gem-google-env-test-api-key-12345678901234567890";
    const _: () =
        assert!(GEM_TOP.len() >= 20 && GEM_SETTINGS.len() >= 20 && GEM_GOOGLE.len() >= 20);
    const _: () =
        assert!(GEM_SETTING.len() >= 20 && GEM_ENV.len() >= 20 && GEM_GOOGLE_ENV.len() >= 20);

    struct EnvScope {
        previous: Vec<(&'static str, Option<String>)>,
        _lock: MutexGuard<'static, ()>,
    }

    impl EnvScope {
        fn new(values: &[(&str, &str)]) -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
            let previous = ENVS
                .iter()
                .map(|key| (*key, std::env::var(key).ok()))
                .collect();
            for key in ENVS {
                unsafe { std::env::remove_var(key) };
            }
            for &(key, value) in values {
                unsafe { std::env::set_var(key, value) };
            }
            Self {
                previous,
                _lock: lock,
            }
        }
    }

    impl Drop for EnvScope {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..).rev() {
                match value {
                    Some(value) => unsafe { std::env::set_var(key, value) },
                    None => unsafe { std::env::remove_var(key) },
                }
            }
        }
    }

    struct Case {
        name: &'static str,
        selector: &'static str,
        top: &'static str,
        settings: &'static [(&'static str, &'static str)],
        env: &'static [(&'static str, &'static str)],
        selected: Option<&'static str>,
        shadowed: &'static [&'static str],
    }

    fn provider(name: &str, api_key: &str) -> ProviderConfig {
        ProviderConfig {
            name: name.to_string(),
            api_key: api_key.to_string(),
            models: vec!["credential-model".to_string()],
            ..ProviderConfig::default()
        }
    }

    async fn run(case: &Case) {
        let before = ENVS
            .iter()
            .map(|key| (*key, std::env::var(key).ok()))
            .collect::<Vec<_>>();
        {
            let _env = EnvScope::new(case.env);
            let mut config = provider(case.name, case.top);
            config.provider_type = case.selector.to_string();
            if matches!(
                case.selector.parse::<ProviderType>(),
                Ok(ProviderType::Cloudflare)
            ) {
                config.organization = Some("fixture-account".to_string());
            }
            for &(key, value) in case.settings {
                config.settings.insert(key.to_string(), value.into());
            }
            let router = Router::from_gateway_config(&[config], None).await;
            if let Some(selected) = case.selected {
                let snapshot = router
                    .unwrap_or_else(|error| panic!("{}: {error}", case.name))
                    .load_routing_snapshot();
                assert!(
                    matches!(
                        snapshot.resolve_legacy_credential("credential-model", selected),
                        Ok(deployment) if deployment == format!("{}-credential-model", case.name)
                    ),
                    "{} selected wrong credential",
                    case.name
                );
                for shadowed in case.shadowed {
                    assert!(
                        matches!(
                            snapshot.resolve_legacy_credential("credential-model", shadowed),
                            Err(ProviderError::ModelNotFound { .. })
                        ),
                        "{} accepted shadowed credential",
                        case.name
                    );
                }
            } else {
                if let Ok(router) = router {
                    assert!(
                        matches!(
                            router.load_routing_snapshot().resolve_legacy_credential(
                                "credential-model",
                                "unresolved-fixture"
                            ),
                            Err(ProviderError::ModelNotFound { .. })
                        ),
                        "{} published unresolved provenance",
                        case.name
                    );
                }
            }
        }
        assert_eq!(
            before,
            ENVS.iter()
                .map(|key| (*key, std::env::var(key).ok()))
                .collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn complete_construction_credential_precedence_matrix() {
        #[rustfmt::skip]
    let cases = [
        Case { name: "native-top", selector: "openai", top: "sk-top-fixture", settings: &[("api_key","sk-settings-fixture")], env: &[], selected: Some("sk-top-fixture"), shadowed: &["sk-settings-fixture"] },
        Case { name: "native-settings", selector: "openai", top: " ", settings: &[("api_key","sk-settings-fixture")], env: &[], selected: Some("sk-settings-fixture"), shadowed: &[] },
        Case { name: "catalog-explicit", selector: "xiaomi_mimo", top: "explicit", settings: &[], env: &[("MIMO_API_KEY","primary"),("XIAOMI_API_KEY","alternate")], selected: Some("explicit"), shadowed: &["primary","alternate"] },
        Case { name: "catalog-primary", selector: "xiaomi_mimo", top: " ", settings: &[], env: &[("MIMO_API_KEY","primary"),("XIAOMI_API_KEY","alternate")], selected: Some("primary"), shadowed: &["alternate"] },
        Case { name: "catalog-alternate", selector: "xiaomi_mimo", top: "", settings: &[], env: &[("MIMO_API_KEY"," "),("XIAOMI_API_KEY","alternate")], selected: Some("alternate"), shadowed: &[] },
        Case { name: "catalog-blank", selector: "xiaomi_mimo", top: " ", settings: &[], env: &[("MIMO_API_KEY"," "),("XIAOMI_API_KEY","")], selected: None, shadowed: &[] },
        Case { name: "catalog-alias-pplx", selector: "pplx", top: "", settings: &[], env: &[("PERPLEXITY_API_KEY","primary")], selected: Some("primary"), shadowed: &[] },
        Case { name: "catalog-alias-github", selector: "github-models", top: "", settings: &[], env: &[("GITHUB_TOKEN","primary")], selected: Some("primary"), shadowed: &[] },
        Case { name: "cf-settings", selector: "cf", top: "top", settings: &[("api_token","settings")], env: &[("CLOUDFLARE_API_TOKEN","env")], selected: Some("settings"), shadowed: &["top","env"] },
        Case { name: "cf-top", selector: "cloudflare", top: "top", settings: &[("api_token"," ")], env: &[("CLOUDFLARE_API_TOKEN","env")], selected: Some("top"), shadowed: &["env"] },
        Case { name: "cf-env", selector: "workers-ai", top: " ", settings: &[("api_token","")], env: &[("CLOUDFLARE_API_TOKEN","env")], selected: Some("env"), shadowed: &[] },
        Case { name: "cf-blank", selector: "cloudflare", top: " ", settings: &[("api_token","")], env: &[("CLOUDFLARE_API_TOKEN"," ")], selected: None, shadowed: &[] },
        Case { name: "cf-api-key", selector: "cloudflare", top: " ", settings: &[("api_token",""),("api_key","settings")], env: &[("CLOUDFLARE_API_TOKEN","env")], selected: Some("settings"), shadowed: &["env"] },
        Case { name: "bedrock-api-key-ignored", selector: "bedrock", top: "stray", settings: &[("aws_access_key_id","AKIATEST123456789012"),("aws_secret_access_key","test-secret-key")], env: &[], selected: None, shadowed: &[] },
        Case { name: "rep-top", selector: "replicate", top: "top", settings: &[("api_key","settings"),("api_token","token")], env: &[("REPLICATE_API_TOKEN","env1"),("REPLICATE_API_KEY","env2")], selected: Some("top"), shadowed: &["settings","token","env1","env2"] },
        Case { name: "rep-settings", selector: "replicate-ai", top: " ", settings: &[("api_key","settings"),("api_token","token")], env: &[("REPLICATE_API_TOKEN","env1")], selected: Some("settings"), shadowed: &["token","env1"] },
        Case { name: "rep-token", selector: "replicate", top: "", settings: &[("api_key"," "),("api_token","token")], env: &[("REPLICATE_API_TOKEN","env1")], selected: Some("token"), shadowed: &["env1"] },
        Case { name: "rep-env1", selector: "replicate", top: "", settings: &[], env: &[("REPLICATE_API_TOKEN","env1"),("REPLICATE_API_KEY","env2")], selected: Some("env1"), shadowed: &["env2"] },
        Case { name: "rep-env2", selector: "replicate", top: "", settings: &[], env: &[("REPLICATE_API_TOKEN"," "),("REPLICATE_API_KEY","env2")], selected: Some("env2"), shadowed: &[] },
        Case { name: "rep-blank", selector: "replicate", top: " ", settings: &[("api_key"," "),("api_token","")], env: &[("REPLICATE_API_TOKEN"," "),("REPLICATE_API_KEY","")], selected: None, shadowed: &[] },
        Case { name: "fal-top", selector: "fal-ai", top: "top", settings: &[("api_key","settings")], env: &[("FAL_AI_API_KEY","env")], selected: Some("top"), shadowed: &["settings","env"] },
        Case { name: "fal-settings", selector: "fal", top: " ", settings: &[("api_key","settings")], env: &[("FAL_AI_API_KEY","env")], selected: Some("settings"), shadowed: &["env"] },
        Case { name: "fal-env", selector: "fal_ai", top: "", settings: &[("api_key"," ")], env: &[("FAL_AI_API_KEY","env")], selected: Some("env"), shadowed: &[] },
        Case { name: "fal-blank", selector: "fal_ai", top: " ", settings: &[("api_key","")], env: &[("FAL_AI_API_KEY"," ")], selected: None, shadowed: &[] },
        Case { name: "cohere-top", selector: "cohere", top: "top", settings: &[("api_key","settings")], env: &[("COHERE_API_KEY","env")], selected: Some("top"), shadowed: &["settings","env"] },
        Case { name: "cohere-settings", selector: "cohere-ai", top: " ", settings: &[("api_key","settings")], env: &[("COHERE_API_KEY","env")], selected: Some("settings"), shadowed: &["env"] },
        Case { name: "cohere-env", selector: "cohere", top: "", settings: &[], env: &[("COHERE_API_KEY","env")], selected: Some("env"), shadowed: &[] },
        Case { name: "cohere-blank", selector: "cohere", top: " ", settings: &[("api_key","")], env: &[("COHERE_API_KEY"," ")], selected: None, shadowed: &[] },
        Case { name: "gem-top", selector: "gemini", top: GEM_TOP, settings: &[("api_key",GEM_SETTINGS)], env: &[("GEMINI_API_KEY",GEM_ENV)], selected: Some(GEM_TOP), shadowed: &[GEM_SETTINGS,GEM_ENV] },
        Case { name: "gem-settings", selector: "google-gemini", top: " ", settings: &[("api_key",GEM_SETTINGS),("google_api_key",GEM_GOOGLE)], env: &[], selected: Some(GEM_SETTINGS), shadowed: &[GEM_GOOGLE] },
        Case { name: "gem-google", selector: "google_ai", top: "", settings: &[("api_key"," "),("google_api_key",GEM_GOOGLE),("gemini_api_key",GEM_SETTING)], env: &[], selected: Some(GEM_GOOGLE), shadowed: &[GEM_SETTING] },
        Case { name: "gem-setting", selector: "google-ai", top: "", settings: &[("google_api_key"," "),("gemini_api_key",GEM_SETTING)], env: &[("GEMINI_API_KEY",GEM_ENV)], selected: Some(GEM_SETTING), shadowed: &[GEM_ENV] },
        Case { name: "gem-env1", selector: "gemini", top: "", settings: &[], env: &[("GEMINI_API_KEY",GEM_ENV),("GOOGLE_API_KEY",GEM_GOOGLE_ENV)], selected: Some(GEM_ENV), shadowed: &[GEM_GOOGLE_ENV] },
        Case { name: "gem-env2", selector: "gemini", top: "", settings: &[], env: &[("GEMINI_API_KEY"," "),("GOOGLE_API_KEY",GEM_GOOGLE_ENV)], selected: Some(GEM_GOOGLE_ENV), shadowed: &[] },
        Case { name: "gem-blank", selector: "gemini", top: " ", settings: &[("api_key"," "),("google_api_key",""),("gemini_api_key"," ")], env: &[("GEMINI_API_KEY"," "),("GOOGLE_API_KEY","")], selected: None, shadowed: &[] },
        Case { name: "unknown", selector: "not-a-provider", top: "unknown", settings: &[], env: &[], selected: None, shadowed: &[] },
    ];
        for case in cases {
            run(&case).await;
        }
    }
}

use super::Router;
use std::collections::HashMap;

fn function<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing {signature}"));
    let body = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .unwrap();
    let mut depth = 0;
    for (offset, byte) in source[body..].bytes().enumerate() {
        depth += usize::from(byte == b'{');
        depth -= usize::from(byte == b'}');
        if depth == 0 {
            return &source[start..=body + offset];
        }
    }
    panic!("unclosed {signature}");
}

#[test]
fn credential_resolver_and_construction_source_guards() {
    let unified = include_str!("unified.rs");
    for signature in [
        "fn resolve_legacy_credential(",
        "fn resolve_legacy_credential_with(",
    ] {
        let resolver = function(unified, signature);
        for forbidden in [
            "std::env",
            "ProviderConfig",
            "create_provider",
            "factory",
            "add_deployment",
        ] {
            assert!(
                !resolver.contains(forbidden),
                "{signature} contains {forbidden}"
            );
        }
    }
    let gateway = include_str!("gateway_config.rs");
    let identity = include_str!("gateway_identity.rs");
    let canonical = function(
        gateway,
        "pub(super) async fn from_gateway_config_with_identity(",
    );
    assert_eq!(canonical.matches("create_provider(").count(), 1);

    let construction_entry_points = [
        function(gateway, "pub async fn from_gateway_config("),
        function(gateway, "pub async fn from_gateway_config_with_aliases("),
        canonical,
        function(identity, "pub async fn from_gateway_config_with_pricing("),
        function(
            identity,
            "pub async fn from_gateway_config_with_aliases_and_pricing(",
        ),
    ];
    assert_eq!(
        construction_entry_points
            .iter()
            .map(|entry_point| entry_point.matches("create_provider(").count())
            .sum::<usize>(),
        1
    );
}

#[test]
fn ambiguous_native_probe_capabilities_require_custom_endpoint_at_config_boundaries() {
    for provider_type in [
        "openai",
        "bedrock",
        "openrouter",
        "vertex_ai",
        "gemini",
        "fal_ai",
        "mistral",
        "cloudflare",
        "azure",
        "azure_ai",
        "ollama",
        "cohere",
        "replicate",
    ] {
        let mut provider = crate::config::models::provider::ProviderConfig {
            name: format!("{provider_type}-primary"),
            provider_type: provider_type.to_string(),
            health_check: crate::config::models::provider::ProviderHealthCheckConfig {
                interval: 15,
                ..Default::default()
            },
            ..crate::config::models::provider::ProviderConfig::default()
        };

        let error = provider
            .validate_health_check_runtime()
            .expect_err("multi-capability providers require an explicit probe endpoint");
        assert!(
            error.contains("require a custom health_check.endpoint"),
            "{error}"
        );

        provider.health_check.endpoint = Some("https://8.8.8.8/health".to_string());
        assert!(provider.validate_health_check_runtime().is_ok());
    }
}

#[test]
fn fal_ai_name_selector_cannot_bypass_native_probe_validation() {
    let provider = crate::config::models::provider::ProviderConfig {
        name: "fal_ai".to_string(),
        provider_type: String::new(),
        health_check: crate::config::models::provider::ProviderHealthCheckConfig {
            interval: 15,
            ..Default::default()
        },
        ..crate::config::models::provider::ProviderConfig::default()
    };

    let error = provider
        .validate_health_check_runtime()
        .expect_err("the effective name selector must be validated");
    assert!(
        error.contains("require a custom health_check.endpoint"),
        "{error}"
    );
}

#[test]
fn unambiguously_chat_only_provider_can_opt_into_native_probe() {
    let provider = crate::config::models::provider::ProviderConfig {
        name: "anthropic-primary".to_string(),
        provider_type: "anthropic".to_string(),
        health_check: crate::config::models::provider::ProviderHealthCheckConfig {
            interval: 15,
            ..Default::default()
        },
        ..crate::config::models::provider::ProviderConfig::default()
    };

    assert!(provider.validate_health_check_runtime().is_ok());
}

fn identity_provider_config(
    model: &str,
    mapping: Option<crate::core::providers::model_identity::ModelIdentityMapping>,
) -> crate::config::models::provider::ProviderConfig {
    let mut config = crate::config::models::provider::ProviderConfig {
        name: "identity-openai".to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-test".to_string(),
        models: vec![model.to_string()],
        ..Default::default()
    };
    if let Some(mapping) = mapping {
        config.settings.insert(
            crate::core::providers::model_identity::MODEL_IDENTITY_MAPPINGS_KEY.to_string(),
            serde_json::json!({model: mapping}),
        );
    }
    config
}

fn runtime_price(provider: &str) -> crate::core::pricing_service::LiteLLMModelInfo {
    crate::core::pricing_service::LiteLLMModelInfo {
        max_tokens: Some(4096),
        max_input_tokens: Some(4096),
        max_output_tokens: Some(1024),
        input_cost_per_token: Some(0.01),
        output_cost_per_token: Some(0.02),
        input_cost_per_character: None,
        output_cost_per_character: None,
        cost_per_second: None,
        litellm_provider: provider.to_string(),
        mode: "chat".to_string(),
        supports_function_calling: None,
        supports_vision: None,
        supports_streaming: None,
        supports_parallel_function_calling: None,
        supports_system_message: None,
        extra: HashMap::new(),
    }
}

#[tokio::test]
async fn pricing_aware_default_openai_publishes_only_callable_catalog_models() {
    let pricing = std::sync::Arc::new(
        crate::core::pricing_service::PricingService::with_embedded_default()
            .expect("embedded pricing should load"),
    );
    let provider = crate::config::models::provider::ProviderConfig {
        name: "default-openai".to_string(),
        provider_type: "openai".to_string(),
        api_key: "sk-test".to_string(),
        models: Vec::new(),
        ..Default::default()
    };

    let router = Router::from_gateway_config_with_pricing(&[provider], None, pricing)
        .await
        .expect("default OpenAI catalog should publish only callable deployments");

    assert!(!router.get_deployments_for_model("gpt-4").is_empty());
    for non_callable in [
        "high/1536-x-1024/gpt-image-1",
        "openai/container",
        "daybreak-blue-latest",
        "openai/sora-2",
        "GPT-4",
    ] {
        assert!(
            router.get_deployments_for_model(non_callable).is_empty(),
            "non-callable or non-exact catalog identity {non_callable:?} became routable"
        );
    }
}

#[tokio::test]
async fn explicit_fine_tune_mapping_preserves_wire_id_and_uses_exact_openai_metadata() {
    let wire_model = "ft:tenant:custom-chat";
    let pricing = std::sync::Arc::new(crate::core::pricing_service::PricingService::new(None));
    pricing.add_custom_model("runtime-only-price".to_string(), runtime_price("openai"));
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        Some("gpt-4".to_string()),
        Some("runtime-only-price".to_string()),
    );

    let router = Router::from_gateway_config_with_pricing(
        &[identity_provider_config(wire_model, Some(mapping))],
        None,
        pricing.clone(),
    )
    .await
    .expect("injected runtime target should validate");
    let deployment = router
        .get_deployment("identity-openai-ft:tenant:custom-chat")
        .expect("deployment should be published");
    let bound = deployment
        .provider
        .runtime_pricing()
        .expect("managed provider should retain injected authority");
    assert!(std::sync::Arc::ptr_eq(&pricing, &bound));
    let identity = deployment
        .provider
        .deployment_model_identity()
        .expect("configured fine-tune must retain typed identity");
    assert_eq!(identity.wire_model(), wire_model);
    assert_eq!(identity.capability_catalog_model(), Some("gpt-4"));
    assert_eq!(identity.pricing_model(), Some("runtime-only-price"));
    let model = crate::core::providers::openai::models::get_openai_registry()
        .get_model_spec(
            identity
                .capability_catalog_model()
                .expect("mapping must provide exact metadata identity"),
        )
        .expect("mapped OpenAI metadata must exist");
    assert_eq!(model.model_info.max_context_length, 8192);
    assert_eq!(model.model_info.max_output_length, Some(8192));
    assert!(deployment.provider.supports_capability_for_model(
        wire_model,
        &crate::core::types::model::ProviderCapability::ChatCompletion,
    ));
    assert!(!deployment.provider.supports_capability_for_model(
        wire_model,
        &crate::core::types::model::ProviderCapability::Embeddings,
    ));
}

#[tokio::test]
async fn unmapped_fine_tune_stays_unprivileged_and_reports_mapping_hint() {
    let wire_model = "ft:tenant:custom-chat";
    let pricing = std::sync::Arc::new(crate::core::pricing_service::PricingService::new(None));
    let router = Router::from_gateway_config_with_pricing(
        &[identity_provider_config(wire_model, None)],
        None,
        pricing,
    )
    .await
    .expect("an exact configured deployment must survive without semantic mappings");

    let deployment = router
        .get_deployment("identity-openai-ft:tenant:custom-chat")
        .expect("configured deployment should be published");
    let identity = deployment
        .provider
        .deployment_model_identity()
        .expect("configured deployment should retain a typed identity");
    assert_eq!(identity.wire_model(), wire_model);
    assert_eq!(identity.capability_catalog_model(), None);
    assert_eq!(identity.pricing_model(), None);

    let error = match router.select_deployment_lease_for_capability(
        wire_model,
        &crate::core::types::model::ProviderCapability::ChatCompletion,
    ) {
        Ok(_) => panic!("an unmapped deployment must not gain catalog capabilities"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("model_identity_mappings.ft:tenant:custom-chat.capability_catalog_model"),
        "{error}"
    );
}

#[tokio::test]
async fn pricing_only_mapping_never_grants_provider_capability() {
    let pricing = std::sync::Arc::new(crate::core::pricing_service::PricingService::new(None));
    pricing.add_custom_model("runtime-only-price".to_string(), runtime_price("openai"));
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        None,
        Some("runtime-only-price".to_string()),
    );
    let router = Router::from_gateway_config_with_pricing(
        &[identity_provider_config("wire-deployment", Some(mapping))],
        None,
        pricing,
    )
    .await
    .expect("pricing-only deployment may be retained for non-capability consumers");
    let deployment = router
        .get_deployment("identity-openai-wire-deployment")
        .expect("deployment should be published");
    assert!(!deployment.provider.supports_capability_for_model(
        "wire-deployment",
        &crate::core::types::model::ProviderCapability::ChatCompletion,
    ));
}

#[tokio::test]
async fn explicit_unpriced_mapping_never_falls_back_to_raw_catalog_price() {
    let pricing = std::sync::Arc::new(
        crate::core::pricing_service::PricingService::with_embedded_default()
            .expect("embedded pricing should load"),
    );
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        Some("gpt-4".to_string()),
        None,
    );
    let router = Router::from_gateway_config_with_pricing(
        &[identity_provider_config("gpt-4", Some(mapping))],
        None,
        pricing.clone(),
    )
    .await
    .expect("explicit unpriced mapping should remain routable by capability");
    let deployment = router
        .get_deployment("identity-openai-gpt-4")
        .expect("deployment should be published");
    let identity = deployment
        .provider
        .deployment_model_identity()
        .expect("validated deployment identity should bind");
    assert_eq!(identity.pricing_model(), None);
    assert_eq!(identity.capability_catalog_model(), Some("gpt-4"));
}

#[tokio::test]
async fn compatibility_constructor_rejects_runtime_mapping_without_authority() {
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        Some("gpt-4".to_string()),
        Some("runtime-only-price".to_string()),
    );
    let error = Router::from_gateway_config(
        &[identity_provider_config("wire-deployment", Some(mapping))],
        None,
    )
    .await
    .expect_err("mapping without injected runtime pricing must fail");
    let text = error.to_string();
    assert!(text.contains("model_identity_mappings") && text.contains("pricing"));
}

#[tokio::test]
async fn pricing_aware_constructor_fails_absent_target_before_snapshot_publication() {
    let pricing = std::sync::Arc::new(crate::core::pricing_service::PricingService::new(None));
    let mapping = crate::core::providers::model_identity::ModelIdentityMapping::new(
        Some("gpt-4".to_string()),
        Some("missing-runtime-price".to_string()),
    );
    let error = Router::from_gateway_config_with_pricing(
        &[identity_provider_config("wire-deployment", Some(mapping))],
        None,
        pricing,
    )
    .await
    .expect_err("missing runtime target must fail startup");
    let text = error.to_string();
    assert!(
        text.contains("identity-openai")
            && text.contains("wire-deployment")
            && text.contains("pricing_model")
            && text.contains("missing-runtime-price"),
        "{text}"
    );
}
