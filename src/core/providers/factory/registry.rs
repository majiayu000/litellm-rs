//! Provider construction registry
//!
//! Implements `Provider::from_config_async`, which maps each `ProviderType`
//! to its concrete provider instantiation logic.

use crate::core::providers::provider_type::ProviderType;
use crate::core::providers::registry as provider_registry;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::providers::{Provider, anthropic, cloudflare, mistral, openai, openai_like};

use super::builder::{
    build_amazon_nova_config_from_factory, build_anthropic_config_from_factory,
    build_azure_ai_config_from_factory, build_cloudflare_config_from_factory,
    build_fal_ai_config_from_factory, build_meta_llama_config_from_factory,
    build_mistral_config_from_factory, build_openai_config_from_factory,
    build_openai_like_config_from_factory, build_v0_config_from_factory, config_str, config_u32,
    config_u64,
};

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
            ProviderType::OpenAICompatible => {
                let oai_like = build_openai_like_config_from_factory(&config)?;
                let provider = openai_like::OpenAILikeProvider::new(oai_like)
                    .await
                    .map_err(|e| {
                        ProviderError::initialization("openai_compatible", e.to_string())
                    })?;
                Ok(Provider::OpenAILike(provider))
            }
            // Catalog-covered provider types: delegate to the Tier 1 registry
            ref pt if provider_registry::get_definition(&pt.to_string()).is_some() => {
                let name = pt.to_string();
                // Safety: guard guarantees the definition exists
                let def = match provider_registry::get_definition(&name) {
                    Some(d) => d,
                    None => {
                        return Err(ProviderError::not_implemented(
                            "unknown",
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
                if let Some(timeout) = config_u64(&config, "timeout") {
                    oai_config.base.timeout = timeout;
                }
                if let Some(max_retries) = config_u32(&config, "max_retries") {
                    oai_config.base.max_retries = max_retries;
                }
                let provider = openai_like::OpenAILikeProvider::new(oai_config)
                    .await
                    .map_err(|e| ProviderError::initialization(def.name, e.to_string()))?;
                Ok(Provider::OpenAILike(provider))
            }
            ProviderType::MetaLlama => {
                let oai_config = build_meta_llama_config_from_factory(&config)?;
                let provider = openai_like::OpenAILikeProvider::new(oai_config)
                    .await
                    .map_err(|e| ProviderError::initialization("meta_llama", e.to_string()))?;
                Ok(Provider::OpenAILike(provider))
            }
            ProviderType::V0 => {
                let oai_config = build_v0_config_from_factory(&config)?;
                let provider = openai_like::OpenAILikeProvider::new(oai_config)
                    .await
                    .map_err(|e| ProviderError::initialization("v0", e.to_string()))?;
                Ok(Provider::OpenAILike(provider))
            }
            ProviderType::AzureAI => {
                let oai_config = build_azure_ai_config_from_factory(&config)?;
                let provider = openai_like::OpenAILikeProvider::new(oai_config)
                    .await
                    .map_err(|e| ProviderError::initialization("azure_ai", e.to_string()))?;
                Ok(Provider::OpenAILike(provider))
            }
            ProviderType::AmazonNova => {
                let oai_config = build_amazon_nova_config_from_factory(&config)?;
                let provider = openai_like::OpenAILikeProvider::new(oai_config)
                    .await
                    .map_err(|e| ProviderError::initialization("amazon_nova", e.to_string()))?;
                Ok(Provider::OpenAILike(provider))
            }
            ProviderType::FalAI => {
                let oai_config = build_fal_ai_config_from_factory(&config)?;
                let provider = openai_like::OpenAILikeProvider::new(oai_config)
                    .await
                    .map_err(|e| ProviderError::initialization("fal_ai", e.to_string()))?;
                Ok(Provider::OpenAILike(provider))
            }
            _ => Err(ProviderError::not_implemented(
                "unknown",
                format!("Factory for {:?} not yet implemented", provider_type),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supported_factory_provider_types() -> Vec<ProviderType> {
        Provider::factory_supported_provider_types().to_vec()
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
        }
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
}
