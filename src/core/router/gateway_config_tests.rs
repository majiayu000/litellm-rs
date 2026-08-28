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
    let construction = function(gateway, "pub async fn from_gateway_config_with_aliases(");
    assert_eq!(construction.matches("create_provider(").count(), 1);
}

mod model_identity {
    use super::super::*;
    use crate::core::providers::openai::OpenAIProvider;
    use crate::core::router::RuntimeBinding;
    use crate::core::types::model::ProviderCapability;
    use std::sync::Arc;

    fn openai_config(models: &[&str], mappings: serde_json::Value) -> ProviderConfig {
        let mut config = ProviderConfig {
            name: "identity-openai".to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-identity-test".to_string(),
            models: models.iter().map(|model| (*model).to_string()).collect(),
            ..ProviderConfig::default()
        };
        if !mappings.is_null() {
            config
                .settings
                .insert(MODEL_IDENTITY_MAPPINGS_KEY.to_string(), mappings);
        }
        config
    }

    #[tokio::test]
    async fn gateway_mapping_preserves_wire_and_routes_by_catalog_identity() {
        let router = Router::from_gateway_config(
            &[openai_config(
                &["customer-chat", "gpt-4", "1024-x-1024/dall-e-2"],
                serde_json::json!({
                    "customer-chat": {
                        "capability_catalog_model": "gpt-4",
                        "pricing_model": "gpt-4"
                    },
                    "1024-x-1024/dall-e-2": {
                        "capability_catalog_model": "gpt-4",
                        "pricing_model": "1024-x-1024/dall-e-2"
                    }
                }),
            )],
            None,
        )
        .await
        .expect("valid mapping should load");
        let custom = router
            .get_deployment("identity-openai-customer-chat")
            .expect("custom deployment should exist");
        assert_eq!(
            custom
                .provider_for_request()
                .resolve_model_identity("customer-chat")
                .wire_model(),
            Some("customer-chat")
        );
        assert!(custom.supports_capability(&ProviderCapability::ChatCompletion));
        assert_eq!(
            custom
                .provider_for_request()
                .resolve_model_identity("customer-chat")
                .pricing_model(),
            Some("gpt-4")
        );
        let exact = router
            .get_deployment("identity-openai-gpt-4")
            .expect("exact catalog deployment should exist");
        assert!(exact.supports_capability(&ProviderCapability::ChatCompletion));
        let pricing_collision = router
            .get_deployment("identity-openai-1024-x-1024/dall-e-2")
            .expect("explicit mapping must override the pricing-only raw key");
        assert!(pricing_collision.supports_capability(&ProviderCapability::ChatCompletion));
    }

    #[tokio::test]
    async fn malformed_or_unsafe_gateway_mappings_fail_loading() {
        for (name, mappings) in [
            (
                "unknown-key",
                serde_json::json!({"missing": {"capability_catalog_model": "gpt-4"}}),
            ),
            (
                "unknown-catalog",
                serde_json::json!({"custom": {"capability_catalog_model": "fake-gpt-5"}}),
            ),
            (
                "wrong-provider",
                serde_json::json!({"custom": {"capability_catalog_model": "anthropic/gpt-4"}}),
            ),
            (
                "pricing-only-capability",
                serde_json::json!({"custom": {"capability_catalog_model": "1024-x-1024/dall-e-2"}}),
            ),
            (
                "unknown-field",
                serde_json::json!({"custom": {"catalog_model": "gpt-4"}}),
            ),
            ("empty", serde_json::json!({"custom": {}})),
        ] {
            let error = Router::from_gateway_config(&[openai_config(&["custom"], mappings)], None)
                .await
                .expect_err(name);
            assert!(
                matches!(error, RouterError::InvalidConfiguration(_)),
                "{name}: {error}"
            );
        }

        let mappings = serde_json::json!({
            "gpt-4": {"capability_catalog_model": "fake-gpt-5", "pricing_model": "gpt-4"}
        });
        let error = Router::from_gateway_config(&[openai_config(&["gpt-4"], mappings)], None)
            .await
            .expect_err("explicit invalid mapping must override a raw catalog collision");
        let text = error.to_string();
        assert!(text.contains("capability_catalog_model"), "{text}");
        assert!(text.contains("fake-gpt-5"), "{text}");
        assert!(text.contains("identity-openai"), "{text}");
        for model in [
            "anthropic/gpt-4",
            "openai/openai/gpt-4",
            "unknown/native/slash",
        ] {
            let mappings = serde_json::json!({
                (model): {"capability_catalog_model": "gpt-4"}
            });
            let error = Router::from_gateway_config(&[openai_config(&[model], mappings)], None)
                .await
                .expect_err("qualified custom deployment must fail closed");
            assert!(
                matches!(error, RouterError::InvalidConfiguration(_)),
                "{model}: {error}"
            );
        }
    }

    #[cfg(feature = "providers-extra")]
    #[tokio::test]
    async fn azure_and_azure_ai_mappings_use_separate_exact_authorities() {
        fn config(
            provider_type: &str,
            model: &str,
            capability: &str,
            pricing: &str,
        ) -> ProviderConfig {
            let mut config = ProviderConfig {
                name: format!("identity-{provider_type}"),
                provider_type: provider_type.to_string(),
                api_key: "identity-test-key".to_string(),
                base_url: Some("https://identity-test.services.ai.azure.com".to_string()),
                api_version: Some("2024-05-01-preview".to_string()),
                models: vec![model.to_string()],
                ..ProviderConfig::default()
            };
            config.settings.insert(
                MODEL_IDENTITY_MAPPINGS_KEY.to_string(),
                serde_json::json!({
                    (model): {
                        "capability_catalog_model": capability,
                        "pricing_model": pricing
                    }
                }),
            );
            config
        }

        for pricing in [
            "azure/eu/gpt-4o-2024-08-06",
            "azure/global-standard/gpt-4o-2024-08-06",
        ] {
            Router::from_gateway_config(
                &[config("azure", "regional-chat", "gpt-4", pricing)],
                None,
            )
            .await
            .unwrap_or_else(|error| panic!("{pricing}: {error}"));
        }

        let router = Router::from_gateway_config(
            &[config(
                "azure_ai",
                "phi-production",
                "Phi-4",
                "azure_ai/Phi-4",
            )],
            None,
        )
        .await
        .expect("real Phi-4 mapping should load");
        let phi = router
            .get_deployment("identity-azure_ai-phi-production")
            .expect("Phi deployment");
        assert!(phi.supports_capability(&ProviderCapability::ChatCompletion));
        assert_eq!(
            phi.provider_for_request()
                .resolve_model_identity("phi-production")
                .pricing_model(),
            Some("azure_ai/Phi-4")
        );

        let error = Router::from_gateway_config(
            &[config(
                "azure",
                "wrong-provider-price",
                "gpt-4",
                "azure_ai/Phi-4",
            )],
            None,
        )
        .await
        .expect_err("Azure must not accept an Azure AI pricing identity");
        let text = error.to_string();
        assert!(text.contains("pricing_model"), "{text}");
        assert!(text.contains("azure_ai/Phi-4"), "{text}");
        assert!(text.contains("identity-azure"), "{text}");
    }

    #[tokio::test]
    async fn unmapped_custom_deployment_has_actionable_capability_error() {
        let router = Router::from_gateway_config(
            &[openai_config(
                &["custom", "1024-x-1024/dall-e-2"],
                serde_json::Value::Null,
            )],
            None,
        )
        .await
        .unwrap();
        let error = match router
            .select_deployment_lease_for_capability("custom", &ProviderCapability::ChatCompletion)
        {
            Err(error) => error,
            Ok(_) => panic!("unmapped custom deployment must not route"),
        };
        assert!(
            error
                .to_string()
                .contains("model_identity_mappings.<deployment>.capability_catalog_model")
        );
        let pricing_only = match router.select_deployment_lease_for_capability(
            "1024-x-1024/dall-e-2",
            &ProviderCapability::ChatCompletion,
        ) {
            Err(error) => error,
            Ok(_) => panic!("pricing-only identity must not route"),
        };
        assert!(matches!(
            pricing_only,
            RouterError::UnsupportedCapability { .. }
        ));
    }

    #[tokio::test]
    async fn bound_provider_clones_and_snapshot_replacement_are_isolated() {
        let provider =
            Provider::OpenAI(OpenAIProvider::with_api_key("sk-isolation").await.unwrap());
        let chat = Deployment::new(
            "chat".into(),
            provider.clone(),
            "customer".into(),
            "public".into(),
        )
        .with_model_identity(Some("gpt-4".into()), None);
        let embedding = Deployment::new(
            "embedding".into(),
            provider.clone(),
            "customer".into(),
            "public".into(),
        )
        .with_model_identity(Some("text-embedding-3-small".into()), None);

        let (chat_ok, embedding_ok) = tokio::join!(
            async { chat.supports_capability(&ProviderCapability::ChatCompletion) },
            async { embedding.supports_capability(&ProviderCapability::Embeddings) }
        );
        assert!(chat_ok && embedding_ok);
        assert!(
            !provider
                .supports_capability_for_model("customer", &ProviderCapability::ChatCompletion)
        );

        let router = Arc::new(Router::default());
        let binding = RuntimeBinding::new(router.clone());
        router.set_model_list(vec![chat]);
        let old = binding.bind();
        router.set_model_list(vec![embedding]);
        assert!(
            old.select_deployment_lease_for_capability(
                "public",
                &ProviderCapability::ChatCompletion
            )
            .is_ok()
        );
        assert!(
            binding
                .bind()
                .select_deployment_lease_for_capability("public", &ProviderCapability::Embeddings)
                .is_ok()
        );
    }
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
