//! Provider construction registry
//!
//! Implements `Provider::from_config_async`, which maps each `ProviderType`
//! to its concrete provider instantiation logic.

use crate::core::providers::provider_type::ProviderType;
use crate::core::providers::registry as provider_registry;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::providers::{
    Provider, anthropic, bedrock, cloudflare, mistral, openai, openai_like,
};
#[cfg(feature = "providers-extra")]
use crate::core::providers::{azure, azure_ai, vertex_ai};
#[cfg(feature = "providers-extended")]
use crate::core::providers::{cohere, fal_ai, gemini, github_copilot};

#[cfg(feature = "providers-extended")]
use super::builder::build_github_copilot_config_from_factory;
use super::builder::{
    apply_tier1_openai_like_overrides, build_anthropic_config_from_factory,
    build_bedrock_config_from_factory, build_cloudflare_config_from_factory,
    build_mistral_config_from_factory, build_openai_config_from_factory,
    build_openai_like_config_from_factory, build_replicate_config_from_factory, config_str,
};
#[cfg(feature = "providers-extra")]
use super::builder::{
    build_azure_ai_config_from_factory, build_azure_config_from_factory,
    build_vertex_ai_config_from_factory,
};
#[cfg(not(feature = "providers-extra"))]
use super::builder::{
    build_azure_ai_openai_like_config_from_factory, build_azure_openai_like_config_from_factory,
};
#[cfg(feature = "providers-extended")]
use super::cohere_builder::build_cohere_config_from_factory;
#[cfg(feature = "providers-extended")]
use super::fal_ai_builder::build_fal_ai_config_from_factory;
#[cfg(feature = "providers-extended")]
use super::gemini_builder::build_gemini_config_from_factory;

impl Provider {
    /// Create provider from configuration asynchronously
    ///
    /// This is the preferred method for creating providers from configuration.
    /// It supports all provider types and handles async initialization properly.
    pub async fn from_config_async(
        provider_type: ProviderType,
        config: serde_json::Value,
    ) -> Result<Self, ProviderError> {
        match provider_type {
            ProviderType::OpenAI => {
                let openai_config = build_openai_config_from_factory(&config)?;
                let provider = openai::OpenAIProvider::new(openai_config)
                    .await
                    .map_err(|e| ProviderError::initialization("openai", e.to_string()))?;
                Ok(Provider::OpenAI(provider))
            }
            ProviderType::Anthropic => {
                let anthropic_config = build_anthropic_config_from_factory(&config)?;
                let provider = anthropic::AnthropicProvider::new(anthropic_config)?;
                Ok(Provider::Anthropic(provider))
            }
            ProviderType::Mistral => {
                let mistral_config = build_mistral_config_from_factory(&config)?;
                let provider = mistral::MistralProvider::new(mistral_config)
                    .await
                    .map_err(|e| ProviderError::initialization("mistral", e.to_string()))?;
                Ok(Provider::Mistral(provider))
            }
            ProviderType::Cloudflare => {
                let cf_config = build_cloudflare_config_from_factory(&config)?;
                let provider = cloudflare::CloudflareProvider::new(cf_config)
                    .await
                    .map_err(|e| ProviderError::initialization("cloudflare", e.to_string()))?;
                Ok(Provider::Cloudflare(provider))
            }
            ProviderType::Cohere => {
                #[cfg(feature = "providers-extended")]
                {
                    let cohere_config = build_cohere_config_from_factory(&config)?;
                    let provider = cohere::CohereProvider::new(cohere_config)
                        .await
                        .map_err(|e| ProviderError::initialization("cohere", e.to_string()))?;
                    Ok(Provider::Cohere(provider))
                }
                #[cfg(not(feature = "providers-extended"))]
                {
                    Err(ProviderError::not_implemented(
                        "cohere",
                        "Cohere native dispatch requires the providers-extended feature",
                    ))
                }
            }
            ProviderType::OpenAICompatible => {
                let oai_like = build_openai_like_config_from_factory(&config)?;
                let provider = openai_like::OpenAILikeProvider::new(oai_like)
                    .await
                    .map_err(|e| {
                        ProviderError::initialization("openai_compatible", e.to_string())
                    })?;
                Ok(Provider::OpenAILike(provider))
            }
            ProviderType::AzureAI => {
                #[cfg(feature = "providers-extra")]
                {
                    let azure_ai_config = build_azure_ai_config_from_factory(&config)?;
                    let provider = azure_ai::AzureAIProvider::new(azure_ai_config)
                        .map_err(|e| ProviderError::initialization("azure_ai", e.to_string()))?;
                    Ok(Provider::AzureAI(provider))
                }
                #[cfg(not(feature = "providers-extra"))]
                {
                    let oai_config = build_azure_ai_openai_like_config_from_factory(&config)?;
                    let provider = openai_like::OpenAILikeProvider::new(oai_config)
                        .await
                        .map_err(|e| ProviderError::initialization("azure_ai", e.to_string()))?;
                    Ok(Provider::OpenAILike(provider))
                }
            }
            ProviderType::FalAI => {
                #[cfg(feature = "providers-extended")]
                {
                    let fal_config = build_fal_ai_config_from_factory(&config)?;
                    let provider = fal_ai::FalAIProvider::new(fal_config)
                        .map_err(|e| ProviderError::initialization("fal_ai", e.to_string()))?;
                    Ok(Provider::FalAI(provider))
                }
                #[cfg(not(feature = "providers-extended"))]
                {
                    Err(ProviderError::not_implemented(
                        "fal_ai",
                        "Fal AI native image-generation dispatch requires the providers-extended feature",
                    ))
                }
            }
            ProviderType::Azure => {
                #[cfg(feature = "providers-extra")]
                {
                    let azure_config = build_azure_config_from_factory(&config)?;
                    let provider = azure::AzureOpenAIProvider::new(azure_config)
                        .map_err(|e| ProviderError::initialization("azure", e.to_string()))?;
                    Ok(Provider::Azure(provider))
                }
                #[cfg(not(feature = "providers-extra"))]
                {
                    let oai_config = build_azure_openai_like_config_from_factory(&config)?;
                    let provider = openai_like::OpenAILikeProvider::new(oai_config)
                        .await
                        .map_err(|e| ProviderError::initialization("azure", e.to_string()))?;
                    Ok(Provider::OpenAILike(provider))
                }
            }
            ProviderType::Bedrock => {
                let bedrock_config = build_bedrock_config_from_factory(&config)?;
                let provider = bedrock::BedrockProvider::new(bedrock_config)
                    .await
                    .map_err(|e| ProviderError::initialization("bedrock", e.to_string()))?;
                Ok(Provider::Bedrock(provider))
            }
            ProviderType::VertexAI => {
                #[cfg(feature = "providers-extra")]
                {
                    let vertex_config = build_vertex_ai_config_from_factory(&config)?;
                    let provider = vertex_ai::VertexAIProvider::new(vertex_config)
                        .await
                        .map_err(|e| ProviderError::initialization("vertex_ai", e.to_string()))?;
                    Ok(Provider::VertexAI(provider))
                }
                #[cfg(not(feature = "providers-extra"))]
                {
                    Err(ProviderError::not_implemented(
                        "vertex_ai",
                        "Vertex AI native dispatch requires the providers-extra feature",
                    ))
                }
            }
            ProviderType::Gemini => {
                #[cfg(feature = "providers-extended")]
                {
                    let gemini_config = build_gemini_config_from_factory(&config)?;
                    let provider = gemini::GeminiProvider::new(gemini_config)
                        .map_err(|e| ProviderError::initialization("gemini", e.to_string()))?;
                    Ok(Provider::Gemini(std::sync::Arc::new(provider)))
                }
                #[cfg(not(feature = "providers-extended"))]
                {
                    Err(ProviderError::not_implemented(
                        "gemini",
                        "Gemini native dispatch requires the providers-extended feature",
                    ))
                }
            }
            ProviderType::Replicate => {
                let oai_config = build_replicate_config_from_factory(&config)?;
                let provider = openai_like::OpenAILikeProvider::new(oai_config)
                    .await
                    .map_err(|e| ProviderError::initialization("replicate", e.to_string()))?;
                Ok(Provider::OpenAILike(provider))
            }
            ProviderType::GitHubCopilot => {
                #[cfg(feature = "providers-extended")]
                {
                    let copilot_config = build_github_copilot_config_from_factory(&config)?;
                    let provider = github_copilot::GitHubCopilotProvider::new(copilot_config)
                        .await
                        .map_err(|e| {
                            ProviderError::initialization("github_copilot", e.to_string())
                        })?;
                    Ok(Provider::GitHubCopilot(provider))
                }
                #[cfg(not(feature = "providers-extended"))]
                {
                    Err(ProviderError::not_implemented(
                        "github_copilot",
                        "GitHub Copilot native dispatch requires the providers-extended feature",
                    ))
                }
            }
            // Catalog-covered provider types: delegate to the Tier 1 registry after
            // all explicit branches, so catalog metadata cannot shadow provider-specific builders.
            ref pt if provider_registry::get_definition(&pt.to_string()).is_some() => {
                let name = pt.to_string();
                // Safety: guard guarantees the definition exists
                let def = match provider_registry::get_definition(&name) {
                    Some(d) => d,
                    None => {
                        return Err(ProviderError::not_implemented(
                            super::provider_diagnostic_name(pt),
                            format!("Catalog definition for '{}' disappeared unexpectedly", name),
                        ));
                    }
                };
                let api_key = config_str(&config, "api_key")
                    .map(|s| s.to_string())
                    .or_else(|| def.resolve_api_key(None));
                let base_url_override =
                    config_str(&config, "base_url").or_else(|| config_str(&config, "api_base"));
                let mut oai_config =
                    def.to_openai_like_config(api_key.as_deref(), base_url_override);
                if let Some(settings) = config.as_object() {
                    let settings = settings
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect();
                    let _ignored_settings =
                        apply_tier1_openai_like_overrides(&mut oai_config, &settings);
                }
                let provider = openai_like::OpenAILikeProvider::new(oai_config)
                    .await
                    .map_err(|e| ProviderError::initialization(def.name, e.to_string()))?;
                Ok(Provider::OpenAILike(provider))
            }
            _ => Err(ProviderError::not_implemented(
                super::provider_diagnostic_name(&provider_type),
                format!("Factory for {:?} not yet implemented", provider_type),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use provider_registry::{ProviderDispatchKind, provider_type_registry};

    fn supported_factory_provider_types() -> Vec<ProviderType> {
        Provider::factory_supported_provider_types().to_vec()
    }

    fn minimal_dispatch_config() -> serde_json::Value {
        serde_json::json!({
            "api_key": "sk-test-key",
            "api_base": "https://example.test/v1",
            "base_url": "https://example.test/v1",
            "endpoint": "https://example.test/v1",
            "provider_name": "test-openai-compatible",
            "organization": "test-account",
            "account_id": "test-account",
            "aws_access_key_id": "AKIA_TEST",
            "aws_secret_access_key": "secret-test",
            "aws_region": "us-east-1",
            "skip_api_key": false
        })
    }

    fn minimal_dispatch_config_for(provider_type: &ProviderType) -> serde_json::Value {
        match provider_type {
            ProviderType::VertexAI => serde_json::json!({
                "project_id": "test-project",
                "location": "us-central1",
                "api_version": "v1",
                "access_token": "ya29.test-token",
                "timeout": 30,
                "max_retries": 2,
                "enable_experimental": true
            }),
            ProviderType::Gemini => serde_json::json!({
                "api_key": "test-gemini-api-key-1234567890",
                "api_version": "v1beta",
                "timeout": 30,
                "connect_timeout": 5,
                "max_retries": 2,
                "enable_caching": false,
                "debug": true
            }),
            ProviderType::GitHubCopilot => serde_json::json!({
                "token_dir": "/tmp/litellm-rs-github-copilot-test",
                "access_token_file": "access-token",
                "api_key_file": "api-key.json",
                "api_base": "https://example.test",
                "timeout": 30,
                "max_retries": 2,
                "disable_system_to_assistant": true,
                "debug": true
            }),
            ProviderType::Cohere => serde_json::json!({
                "api_key": "test-cohere-key",
                "api_base": "https://api.cohere.ai",
                "api_version": "v2",
                "timeout": 30,
                "max_retries": 2,
                "default_embedding_input_type": "search_query"
            }),
            ProviderType::FalAI => serde_json::json!({
                "api_key": "test-fal-ai-key",
                "api_base": "https://fal.run",
                "timeout": 30,
                "max_retries": 2,
                "output_format": "png",
                "sync_mode": true
            }),
            _ => minimal_dispatch_config(),
        }
    }

    #[tokio::test]
    async fn test_dispatch_kind_matches_runtime_variant() {
        for entry in provider_type_registry() {
            if !entry.is_dispatchable() {
                continue;
            }

            let provider = Provider::from_config_async(
                entry.provider_type.clone(),
                minimal_dispatch_config_for(&entry.provider_type),
            )
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "{:?} should be creatable for dispatch-kind guard: {}",
                    entry.provider_type, err
                )
            });

            match entry.dispatch_kind {
                ProviderDispatchKind::Native => match (&entry.provider_type, &provider) {
                    (ProviderType::OpenAI, Provider::OpenAI(_))
                    | (ProviderType::Anthropic, Provider::Anthropic(_))
                    | (ProviderType::Bedrock, Provider::Bedrock(_))
                    | (ProviderType::Mistral, Provider::Mistral(_))
                    | (ProviderType::Cloudflare, Provider::Cloudflare(_)) => {}
                    #[cfg(feature = "providers-extra")]
                    (ProviderType::Azure, Provider::Azure(_))
                    | (ProviderType::AzureAI, Provider::AzureAI(_))
                    | (ProviderType::VertexAI, Provider::VertexAI(_)) => {}
                    #[cfg(feature = "providers-extended")]
                    (ProviderType::Cohere, Provider::Cohere(_)) => {}
                    #[cfg(feature = "providers-extended")]
                    (ProviderType::FalAI, Provider::FalAI(_)) => {}
                    #[cfg(feature = "providers-extended")]
                    (ProviderType::Gemini, Provider::Gemini(_)) => {}
                    #[cfg(feature = "providers-extended")]
                    (ProviderType::GitHubCopilot, Provider::GitHubCopilot(_)) => {}
                    _ => panic!(
                        "{:?} is classified Native but created runtime provider {:?}",
                        entry.provider_type,
                        provider.provider_type()
                    ),
                },
                ProviderDispatchKind::ExplicitOpenAiLike
                | ProviderDispatchKind::CatalogOpenAiLike => assert!(
                    matches!(provider, Provider::OpenAILike(_)),
                    "{:?} is classified OpenAI-like but created {:?}",
                    entry.provider_type,
                    provider.provider_type()
                ),
                ProviderDispatchKind::UnsupportedEnum => unreachable!(
                    "{:?} is not dispatchable and should have been skipped",
                    entry.provider_type
                ),
            }
        }
    }

    #[tokio::test]
    async fn test_from_config_async_supported_variants_do_not_fallthrough_to_not_implemented() {
        for provider_type in supported_factory_provider_types() {
            let result =
                Provider::from_config_async(provider_type.clone(), serde_json::json!({})).await;
            // Success is fine (e.g. local catalog providers with skip_api_key);
            // a real config error is also fine. Only NotImplemented is wrong.
            if let Err(err) = result {
                assert!(
                    !matches!(err, ProviderError::NotImplemented { .. }),
                    "{:?} unexpectedly fell through to NotImplemented: {}",
                    provider_type,
                    err
                );
            }
        }
    }

    #[tokio::test]
    async fn test_from_config_async_unsupported_variants_return_not_implemented() {
        let supported = supported_factory_provider_types();

        for provider_type in crate::core::providers::provider_type::all_non_custom_provider_types()
        {
            if supported.contains(&provider_type) {
                continue;
            }

            let err = Provider::from_config_async(provider_type.clone(), serde_json::json!({}))
                .await
                .expect_err("Expected unsupported provider to fail");
            assert!(
                matches!(err, ProviderError::NotImplemented { .. }),
                "Expected NotImplemented for {:?}, got {}",
                provider_type,
                err
            );
            assert_eq!(
                err.provider(),
                provider_type.to_string(),
                "NotImplemented provider name should identify the requested provider"
            );
        }
    }

    #[cfg(feature = "providers-extended")]
    #[tokio::test]
    async fn test_from_config_async_cohere_creates_native_provider() {
        let provider = Provider::from_config_async(
            ProviderType::Cohere,
            serde_json::json!({
                "api_key": "test-cohere-key",
                "api_base": "https://api.cohere.ai",
                "api_version": "v2",
                "timeout": 30,
                "max_retries": 2,
                "default_embedding_input_type": "search_query"
            }),
        )
        .await
        .unwrap_or_else(|err| panic!("cohere should create native provider: {err}"));

        assert!(matches!(provider, Provider::Cohere(_)));
        assert_eq!(provider.name(), "cohere");
        assert_eq!(provider.provider_type(), ProviderType::Cohere);
    }

    #[cfg(feature = "providers-extended")]
    #[tokio::test]
    async fn test_from_config_async_cohere_rejects_invalid_api_version() {
        let err = Provider::from_config_async(
            ProviderType::Cohere,
            serde_json::json!({
                "api_key": "test-cohere-key",
                "api_version": "v3"
            }),
        )
        .await
        .expect_err("cohere should reject invalid API versions");

        assert!(
            matches!(err, ProviderError::Configuration { .. }),
            "expected Configuration, got {err}"
        );
        assert!(
            err.to_string().contains("api_version must be v1 or v2"),
            "error should identify valid Cohere API versions: {err}"
        );
    }

    #[cfg(feature = "providers-extended")]
    #[tokio::test]
    async fn test_from_config_async_cohere_rejects_invalid_embedding_input_type() {
        let err = Provider::from_config_async(
            ProviderType::Cohere,
            serde_json::json!({
                "api_key": "test-cohere-key",
                "default_embedding_input_type": "chat"
            }),
        )
        .await
        .expect_err("cohere should reject invalid default embedding input types");

        assert!(
            matches!(err, ProviderError::Configuration { .. }),
            "expected Configuration, got {err}"
        );
        assert!(
            err.to_string()
                .contains("default_embedding_input_type must be one of"),
            "error should identify valid Cohere embedding input types: {err}"
        );
    }

    #[tokio::test]
    async fn test_from_config_async_cloudflare_accepts_alias_fields() {
        let config = serde_json::json!({
            "organization": "acct-alias",
            "api_key": "token-alias"
        });

        let provider = Provider::from_config_async(ProviderType::Cloudflare, config)
            .await
            .unwrap_or_else(|err| {
                panic!("cloudflare should be creatable from alias fields: {err}")
            });
        assert!(matches!(provider, Provider::Cloudflare(_)));
    }

    #[cfg(feature = "providers-extended")]
    #[tokio::test]
    async fn test_from_config_async_fal_ai_creates_native_image_provider() {
        let provider = Provider::from_config_async(
            ProviderType::FalAI,
            minimal_dispatch_config_for(&ProviderType::FalAI),
        )
        .await
        .unwrap_or_else(|err| panic!("fal_ai should create native provider: {err}"));

        assert!(matches!(provider, Provider::FalAI(_)));
        assert_eq!(provider.name(), "fal_ai");
        assert_eq!(provider.provider_type(), ProviderType::FalAI);
        assert!(
            provider
                .capabilities()
                .contains(&crate::core::types::model::ProviderCapability::ImageGeneration)
        );
    }

    #[tokio::test]
    async fn test_from_config_async_openai_compatible_accepts_api_base_alias() {
        let config = serde_json::json!({
            "api_base": "http://localhost:11434/v1",
            "skip_api_key": true,
            "provider_name": "local-openai-like"
        });

        let provider = Provider::from_config_async(ProviderType::OpenAICompatible, config)
            .await
            .unwrap_or_else(|err| panic!("openai_compatible should be creatable: {err}"));
        assert!(matches!(provider, Provider::OpenAILike(_)));
    }

    #[cfg(feature = "providers-extended")]
    #[tokio::test]
    async fn test_from_config_async_gemini_creates_native_google_ai_provider() {
        let provider = Provider::from_config_async(
            ProviderType::Gemini,
            minimal_dispatch_config_for(&ProviderType::Gemini),
        )
        .await
        .unwrap_or_else(|err| panic!("gemini should create native provider: {err}"));

        assert!(matches!(provider, Provider::Gemini(_)));
        assert_eq!(provider.name(), "gemini");
        assert_eq!(provider.provider_type(), ProviderType::Gemini);
    }

    #[cfg(feature = "providers-extended")]
    #[tokio::test]
    async fn test_from_config_async_gemini_rejects_vertex_ai_fields() {
        let err = Provider::from_config_async(
            ProviderType::Gemini,
            serde_json::json!({
                "api_key": "test-gemini-api-key-1234567890",
                "project_id": "test-project",
                "location": "us-central1"
            }),
        )
        .await
        .expect_err("gemini selector should reject Vertex AI project/location credentials");

        assert!(
            matches!(err, ProviderError::InvalidRequest { .. }),
            "expected InvalidRequest, got {err}"
        );
        assert!(
            err.to_string().contains("use vertex_ai"),
            "error should point callers to vertex_ai selector: {err}"
        );
    }

    #[cfg(feature = "providers-extended")]
    #[tokio::test]
    async fn test_from_config_async_github_copilot_creates_native_without_static_key() {
        let provider = Provider::from_config_async(
            ProviderType::GitHubCopilot,
            minimal_dispatch_config_for(&ProviderType::GitHubCopilot),
        )
        .await
        .unwrap_or_else(|err| panic!("github_copilot should create native provider: {err}"));

        assert!(matches!(provider, Provider::GitHubCopilot(_)));
        assert_eq!(provider.name(), "github_copilot");
        assert_eq!(provider.provider_type(), ProviderType::GitHubCopilot);
    }

    #[cfg(feature = "providers-extended")]
    #[tokio::test]
    async fn test_from_config_async_github_copilot_rejects_static_api_key() {
        let err = Provider::from_config_async(
            ProviderType::GitHubCopilot,
            serde_json::json!({"api_key": "sk-test"}),
        )
        .await
        .expect_err("github_copilot native auth should reject static api_key");

        assert!(
            matches!(err, ProviderError::InvalidRequest { .. }),
            "expected InvalidRequest, got {err}"
        );
        assert!(
            err.to_string().contains("static api_key"),
            "error should identify static api_key rejection: {err}"
        );
    }

    #[cfg(feature = "providers-extra")]
    #[tokio::test]
    async fn test_from_config_async_vertex_ai_creates_native_with_access_token() {
        let provider = Provider::from_config_async(
            ProviderType::VertexAI,
            minimal_dispatch_config_for(&ProviderType::VertexAI),
        )
        .await
        .unwrap_or_else(|err| panic!("vertex_ai should create native provider: {err}"));

        assert!(matches!(provider, Provider::VertexAI(_)));
        assert_eq!(provider.name(), "vertex_ai");
        assert_eq!(provider.provider_type(), ProviderType::VertexAI);
    }

    #[cfg(feature = "providers-extra")]
    #[tokio::test]
    async fn test_from_config_async_vertex_ai_rejects_static_api_key() {
        let err = Provider::from_config_async(
            ProviderType::VertexAI,
            serde_json::json!({
                "api_key": "sk-test",
                "project_id": "test-project"
            }),
        )
        .await
        .expect_err("vertex_ai native auth should reject static api_key");

        assert!(
            matches!(err, ProviderError::InvalidRequest { .. }),
            "expected InvalidRequest, got {err}"
        );
        assert!(
            err.to_string().contains("static api_key"),
            "error should identify static api_key rejection: {err}"
        );
    }

    #[tokio::test]
    async fn issue_606_catalogified_candidates_use_catalog_runtime_path() {
        for provider_type in [
            ProviderType::MetaLlama,
            ProviderType::V0,
            ProviderType::AmazonNova,
            ProviderType::GitHub,
        ] {
            let provider = Provider::from_config_async(
                provider_type.clone(),
                serde_json::json!({
                    "api_key": "sk-test-key",
                    "headers": {"x-test-header": "test-value"},
                    "custom_headers": {"x-custom-header": "custom-value"},
                    "timeout": 42,
                    "max_retries": 4
                }),
            )
            .await
            .unwrap_or_else(|err| panic!("{provider_type:?} should be creatable: {err}"));

            assert!(matches!(provider, Provider::OpenAILike(_)));
            assert_eq!(provider.name(), provider_type.to_string());
        }
    }
}
