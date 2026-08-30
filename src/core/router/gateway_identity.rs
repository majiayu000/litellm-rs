//! Startup-only validation and binding for deployment model identities.

use super::config::RouterConfig;
use super::error::RouterError;
use super::unified::Router;
use crate::config::models::provider::ProviderConfig;
use crate::core::pricing_service::{PricingService, PricingSnapshot};
use crate::core::providers::Provider;
use crate::core::providers::model_identity::{
    MODEL_IDENTITY_MAPPINGS_KEY, ModelIdentityMapping, canonical_identity_provider,
    validate_deployment_identity,
};
use crate::core::providers::provider_type::ProviderType;
use crate::core::providers::registry::model_catalog_authority::{
    CatalogAuthority, CatalogDecision, CatalogResolution,
};
use crate::core::types::model_id::ModelIdRef;
use std::collections::HashMap;
use std::sync::Arc;

/// One immutable authority generation used while a routing snapshot is staged.
pub(super) struct GatewayIdentityAuthority {
    pricing: Arc<PricingService>,
    snapshot: PricingSnapshot,
    catalog: CatalogAuthority,
}

impl GatewayIdentityAuthority {
    pub(super) fn new(pricing: Arc<PricingService>) -> Result<Self, RouterError> {
        let snapshot = pricing.snapshot();
        let catalog = CatalogAuthority::from_embedded()
            .map_err(|error| RouterError::InvalidConfiguration(error.to_string()))?;
        Ok(Self {
            pricing,
            snapshot,
            catalog,
        })
    }

    pub(super) fn bind(
        &self,
        provider_name: &str,
        provider: &mut Provider,
        wire_model: &str,
        mapping: Option<&ModelIdentityMapping>,
    ) -> Result<(), RouterError> {
        let (identity_provider, identity_model) = match provider {
            Provider::OpenAILike(openai_like) if openai_like.config().provider_name == "xai" => {
                let configured = openai_like.config().get_effective_model(wire_model);
                let parsed = ModelIdRef::parse(&configured);
                if parsed.provider() == Some("xai")
                    && ModelIdRef::parse(parsed.model()).provider() == Some("xai")
                {
                    return Err(RouterError::InvalidConfiguration(format!(
                        "provider '{provider_name}' deployment '{wire_model}' has double provider qualification"
                    )));
                }
                let effective = crate::core::providers::openai_like::models::xai_native_wire_model(
                    "xai",
                    openai_like.config().model_prefix.is_some(),
                    configured,
                );
                if mapping.is_none() {
                    let resolution = self.catalog.resolve_model("xai", &effective);
                    let pricing_key = if ModelIdRef::parse(&effective).provider() == Some("xai") {
                        effective.clone()
                    } else {
                        format!("xai/{effective}")
                    };
                    let legacy_unreviewed = matches!(resolution, CatalogResolution::Unreviewed)
                        || (matches!(resolution, CatalogResolution::Unknown)
                            && self.catalog.decision_for_pricing_key("xai", &pricing_key)
                                == Some(CatalogDecision::Unreviewed));
                    if legacy_unreviewed {
                        return Ok(());
                    }
                }
                ("xai", effective)
            }
            Provider::OpenAILike(openai_like)
                if mapping.is_some() || ModelIdRef::parse(wire_model).provider() == Some("xai") =>
            {
                (
                    "openai_compatible",
                    openai_like.config().get_effective_model(wire_model),
                )
            }
            _ => {
                let Some(identity_provider) = identity_provider(provider) else {
                    return Ok(());
                };
                (identity_provider, wire_model.to_string())
            }
        };
        debug_assert!(canonical_identity_provider(identity_provider).is_some());
        let legacy_target = provider
            .legacy_openai_model_target(wire_model)
            .map(str::to_owned);
        let identity = validate_deployment_identity(
            provider_name,
            identity_provider,
            &identity_model,
            mapping,
            legacy_target.as_deref(),
            &self.catalog,
            &self.snapshot,
        )
        .map_err(|error| RouterError::InvalidConfiguration(error.to_string()))?;
        provider
            .bind_deployment_model_identity(identity, Arc::clone(&self.pricing))
            .map_err(RouterError::InvalidConfiguration)
    }
}

pub(super) fn default_models(
    provider: &Provider,
    authority: Option<&GatewayIdentityAuthority>,
) -> Vec<String> {
    let identity_provider = identity_provider(provider);
    provider
        .list_models()
        .iter()
        .filter(|model| match (authority, identity_provider) {
            (Some(authority), Some(identity_provider)) => matches!(
                authority
                    .catalog
                    .resolve_model(identity_provider, &model.id),
                CatalogResolution::Callable(_)
            ),
            _ => true,
        })
        .map(|model| model.id.clone())
        .collect()
}

fn identity_provider(provider: &Provider) -> Option<&'static str> {
    match provider.provider_type() {
        ProviderType::OpenAI => Some("openai"),
        #[cfg(feature = "providers-extra")]
        ProviderType::Azure => Some("azure"),
        #[cfg(feature = "providers-extra")]
        ProviderType::AzureAI => Some("azure_ai"),
        _ => None,
    }
}

impl Router {
    /// Create a router using one runtime pricing authority for identity validation.
    pub async fn from_gateway_config_with_pricing(
        providers: &[ProviderConfig],
        router_config: Option<RouterConfig>,
        pricing: Arc<PricingService>,
    ) -> Result<Self, RouterError> {
        Self::from_gateway_config_with_aliases_and_pricing(
            providers,
            router_config,
            &HashMap::new(),
            pricing,
        )
        .await
    }

    /// Create a router with aliases and a shared runtime pricing authority.
    pub async fn from_gateway_config_with_aliases_and_pricing(
        providers: &[ProviderConfig],
        router_config: Option<RouterConfig>,
        model_aliases: &HashMap<String, String>,
        pricing: Arc<PricingService>,
    ) -> Result<Self, RouterError> {
        let authority = GatewayIdentityAuthority::new(pricing)?;
        Self::from_gateway_config_with_identity(
            providers,
            router_config,
            model_aliases,
            Some(&authority),
        )
        .await
    }
}

pub(super) fn take_identity_mappings(
    config: &mut ProviderConfig,
    authority: Option<&GatewayIdentityAuthority>,
) -> Result<HashMap<String, ModelIdentityMapping>, RouterError> {
    let Some(value) = config.settings.remove(MODEL_IDENTITY_MAPPINGS_KEY) else {
        return Ok(HashMap::new());
    };
    if authority.is_none() {
        return Err(RouterError::InvalidConfiguration(format!(
            "provider '{}' settings.{MODEL_IDENTITY_MAPPINGS_KEY} requires the pricing-aware router constructor",
            config.name
        )));
    }
    let mappings: HashMap<String, ModelIdentityMapping> =
        serde_json::from_value(value).map_err(|error| {
            RouterError::InvalidConfiguration(format!(
                "provider '{}' settings.{MODEL_IDENTITY_MAPPINGS_KEY} is invalid: {error}",
                config.name
            ))
        })?;
    for deployment in mappings.keys() {
        if deployment.trim().is_empty() || !config.models.iter().any(|model| model == deployment) {
            return Err(RouterError::InvalidConfiguration(format!(
                "provider '{}' settings.{MODEL_IDENTITY_MAPPINGS_KEY} key '{}' does not name a configured model",
                config.name, deployment
            )));
        }
    }
    Ok(mappings)
}

#[cfg(test)]
mod tests {
    use super::GatewayIdentityAuthority;
    use crate::core::pricing_service::PricingService;
    use crate::core::providers::Provider;
    use crate::core::providers::model_identity::ModelIdentityMapping;
    use crate::core::providers::openai_like::{OpenAILikeConfig, OpenAILikeProvider};
    use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
    use crate::core::types::{chat::ChatRequest, context::RequestContext};
    use std::sync::Arc;

    async fn native_xai_provider() -> Provider {
        let config = OpenAILikeConfig::new("https://api.x.ai/v1")
            .with_provider_name("xai")
            .with_skip_api_key(true);
        Provider::OpenAILike(
            OpenAILikeProvider::new(config)
                .await
                .expect("native xAI provider"),
        )
    }

    fn authority() -> GatewayIdentityAuthority {
        let pricing = Arc::new(
            PricingService::with_embedded_default().expect("embedded pricing should load"),
        );
        GatewayIdentityAuthority::new(pricing).expect("identity authority")
    }

    #[tokio::test]
    async fn native_xai_rejects_double_qualifier_before_wire_normalization() {
        let mut provider = native_xai_provider().await;
        let error = authority()
            .bind("native-xai", &mut provider, "xai/xai/grok-4.6", None)
            .expect_err("double xAI qualification must fail before stripping");
        assert!(error.to_string().contains("double provider qualification"));
    }

    #[tokio::test]
    async fn native_xai_unknown_models_bind_unpriced_while_legacy_pricing_remains() {
        for model in ["grok-4.6-latest", "unknown-xai-model"] {
            let mut provider = native_xai_provider().await;
            authority()
                .bind("native-xai", &mut provider, model, None)
                .expect("unknown configured model should bind without privilege");
            let identity = provider
                .deployment_model_identity()
                .expect("unknown configured model must retain an identity");
            assert_eq!(identity.wire_model(), model);
            assert_eq!(identity.capability_catalog_model(), None);
            assert_eq!(identity.pricing_model(), None);
            assert!(provider.calculate_cost(model, 1, 1).await.is_err());
        }

        let mut legacy = native_xai_provider().await;
        authority()
            .bind("native-xai", &mut legacy, "grok-4.3", None)
            .expect("legacy unreviewed model should preserve compatibility");
        assert!(legacy.deployment_model_identity().is_none());
        assert!(
            legacy
                .calculate_cost("grok-4.3", 1_000, 1_000)
                .await
                .unwrap()
                > 0.0
        );
    }

    #[tokio::test]
    async fn native_xai_accepts_its_qualified_explicit_mapping() {
        let mut provider = native_xai_provider().await;
        let mapping = ModelIdentityMapping::new(
            Some("xai/grok-4.6".to_string()),
            Some("xai/grok-4.6".to_string()),
        );
        authority()
            .bind("native-xai", &mut provider, "grok-4.6", Some(&mapping))
            .expect("native xAI should accept its own exact qualifier");
        let identity = provider
            .deployment_model_identity()
            .expect("explicit xAI mapping should bind");
        assert_eq!(identity.capability_catalog_model(), Some("grok-4.6"));
        assert_eq!(identity.pricing_model(), Some("xai/grok-4.6"));
    }

    #[tokio::test]
    async fn bound_openai_compatible_identity_preserves_model_prefix_stripping() {
        let config = OpenAILikeConfig::new("https://vertex.example.com/v1")
            .with_provider_name("vertex_publisher")
            .with_model_prefix("vertex/")
            .with_skip_api_key(true);
        let mut provider = Provider::OpenAILike(
            OpenAILikeProvider::new(config)
                .await
                .expect("mapped provider"),
        );
        let mapping = ModelIdentityMapping::new(Some("xai/grok-4.6".to_string()), None);
        authority()
            .bind(
                "mapped-vertex",
                &mut provider,
                "vertex/grok-4.6",
                Some(&mapping),
            )
            .expect("mapped prefixed deployment should bind");
        let identity = provider
            .deployment_model_identity()
            .expect("mapped identity should bind");
        assert_eq!(identity.wire_model(), "grok-4.6");

        let Provider::OpenAILike(provider) = provider else {
            panic!("mapped provider should use OpenAI-compatible transport");
        };
        let request = ChatRequest {
            model: "vertex/grok-4.6".to_string(),
            messages: vec![],
            ..Default::default()
        };
        let json = LLMProvider::transform_request(&provider, request, RequestContext::default())
            .await
            .expect("mapped prefixed request should transform");
        assert_eq!(json["model"], "grok-4.6");
    }
}
