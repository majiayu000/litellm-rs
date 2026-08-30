use uuid::Uuid;

use crate::config::models::gateway::GatewayPricingConfig;
use crate::core::budget::{BudgetReservation, UnifiedBudgetLimits, UnifiedBudgetReservation};
use crate::core::keys::KeyManager;
use crate::core::pricing_service::{
    CostResult, CostType, PricingCostBreakdown, PricingCostEstimate, PricingService,
    PricingSnapshot, PricingUsage,
};
use crate::core::providers::Provider;
use crate::core::providers::model_identity::{DeploymentModelIdentity, DeploymentPricingIdentity};
use crate::core::providers::provider_type::ProviderType;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::embedding::EmbeddingInput;
use crate::core::types::model::ProviderCapability;
use crate::utils::ai::counter::token_counter::{TokenCounter, TokenizerIdentity};
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(in crate::server::routes::ai) enum PricingIdentity {
    Priced { provider: String, model: String },
    Unpriced { provider: String },
}

#[derive(Clone, Debug)]
pub(in crate::server::routes::ai) struct RequestPricing {
    snapshot: PricingSnapshot,
    identity: PricingIdentity,
    token_identity: TokenizerIdentity,
}

impl RequestPricing {
    #[cfg(test)]
    pub(in crate::server::routes::ai) fn from_exact(
        pricing: &PricingService,
        provider: impl Into<String>,
        model: impl Into<String>,
    ) -> Self {
        let provider = provider.into();
        let model = model.into();
        let snapshot = pricing.snapshot();
        let identity = snapshot
            .get_model_info_for_provider(&provider, &model)
            .map_or_else(
                || PricingIdentity::Unpriced {
                    provider: provider.clone(),
                },
                |(resolved, _)| PricingIdentity::Priced {
                    provider: provider.clone(),
                    model: resolved,
                },
            );
        let token_identity = if provider == "openai" {
            TokenizerIdentity::exact_openai(model)
        } else {
            TokenizerIdentity::approximate(provider, model)
        };
        Self {
            snapshot,
            identity,
            token_identity,
        }
    }

    pub(in crate::server::routes::ai) fn priced_parts(&self) -> Option<(&str, &str)> {
        match &self.identity {
            PricingIdentity::Priced { provider, model } => Some((provider, model)),
            PricingIdentity::Unpriced { .. } => None,
        }
    }

    pub(in crate::server::routes::ai) fn token_identity(&self) -> &TokenizerIdentity {
        &self.token_identity
    }

    pub(in crate::server::routes::ai) fn with_exact_priced_model(
        &self,
        model: &str,
    ) -> Option<Self> {
        let (provider, _) = self.priced_parts()?;
        let (resolved, _) = self.snapshot.get_model_info_for_provider(provider, model)?;
        (resolved == model).then(|| Self {
            snapshot: self.snapshot.clone(),
            identity: PricingIdentity::Priced {
                provider: provider.to_string(),
                model: resolved,
            },
            token_identity: self.token_identity.clone(),
        })
    }

    pub(in crate::server::routes::ai) fn model_info(
        &self,
    ) -> Option<crate::core::pricing_service::LiteLLMModelInfo> {
        let (provider, model) = self.priced_parts()?;
        self.snapshot
            .get_model_info_for_provider(provider, model)
            .map(|(_, info)| info)
    }

    fn pricing_error(&self) -> crate::utils::error::gateway_error::GatewayError {
        let provider = match &self.identity {
            PricingIdentity::Priced { provider, .. } | PricingIdentity::Unpriced { provider } => {
                provider
            }
        };
        crate::utils::error::gateway_error::GatewayError::Config(format!(
            "request pricing is explicitly unavailable for provider '{provider}'"
        ))
    }

    pub(in crate::server::routes::ai) fn estimate_completion(
        &self,
        input_tokens: u32,
        max_output_tokens: Option<u32>,
    ) -> crate::utils::error::gateway_error::Result<PricingCostEstimate> {
        let Some((provider, model)) = self.priced_parts() else {
            return Err(self.pricing_error());
        };
        self.snapshot.estimate_loaded_completion_cost_for_provider(
            provider,
            model,
            input_tokens,
            max_output_tokens,
        )
    }

    pub(in crate::server::routes::ai) fn calculate_usage(
        &self,
        usage: &PricingUsage,
    ) -> crate::utils::error::gateway_error::Result<PricingCostBreakdown> {
        let Some((provider, model)) = self.priced_parts() else {
            return Err(self.pricing_error());
        };
        self.snapshot
            .calculate_loaded_usage_cost_for_provider(provider, model, usage)
    }

    pub(in crate::server::routes::ai) fn calculate_settlement(
        &self,
        usage: &PricingUsage,
    ) -> crate::utils::error::gateway_error::Result<PricingCostBreakdown> {
        let Some((provider, model)) = self.priced_parts() else {
            return Err(self.pricing_error());
        };
        self.snapshot
            .calculate_loaded_settlement_cost_for_provider(provider, model, usage)
    }

    pub(in crate::server::routes::ai) fn has_time_pricing(&self) -> bool {
        self.priced_parts()
            .and_then(|(provider, model)| {
                self.snapshot.get_model_info_for_provider(provider, model)
            })
            .is_some_and(|(_, info)| info.cost_per_second.is_some())
    }

    pub(in crate::server::routes::ai) fn calculate_time(
        &self,
        total_time_seconds: f64,
    ) -> crate::utils::error::gateway_error::Result<CostResult> {
        let Some((provider, model)) = self.priced_parts() else {
            return Err(self.pricing_error());
        };
        let (resolved, info) = self
            .snapshot
            .get_model_info_for_provider(provider, model)
            .ok_or_else(|| self.pricing_error())?;
        let rate = info.cost_per_second.ok_or_else(|| {
            crate::utils::error::gateway_error::GatewayError::Config(format!(
                "time pricing is unavailable for '{provider}/{resolved}'"
            ))
        })?;
        let total_cost = total_time_seconds * rate;
        Ok(CostResult {
            input_cost: 0.0,
            output_cost: 0.0,
            total_cost,
            input_tokens: 0,
            output_tokens: 0,
            model: resolved,
            provider: info.litellm_provider,
            cost_type: CostType::TimeBased,
        })
    }
}

pub(in crate::server::routes::ai) fn request_pricing_for_provider(
    pricing_service: &Arc<PricingService>,
    provider: &Provider,
    model: &str,
    surface: ProviderCapability,
) -> Result<RequestPricing, ProviderError> {
    request_pricing_for_provider_with_snapshot_hook(
        pricing_service,
        provider,
        model,
        surface,
        || {},
    )
}

fn request_pricing_for_provider_with_snapshot_hook(
    pricing_service: &Arc<PricingService>,
    provider: &Provider,
    model: &str,
    surface: ProviderCapability,
    after_snapshot: impl FnOnce(),
) -> Result<RequestPricing, ProviderError> {
    let snapshot = pricing_service.snapshot();
    after_snapshot();
    let bound_identity = provider.deployment_model_identity();
    if bound_identity.is_some()
        || matches!(
            provider.provider_type(),
            ProviderType::OpenAI | ProviderType::Azure | ProviderType::AzureAI
        )
    {
        let bound_pricing = provider.runtime_pricing().ok_or_else(|| {
            ProviderError::configuration(
                "model_identity",
                format!(
                    "deployment '{model}' has no validated runtime model identity; configure settings.model_identity_mappings"
                ),
            )
        })?;
        if !Arc::ptr_eq(pricing_service, &bound_pricing) {
            return Err(ProviderError::configuration(
                "model_identity",
                "selected deployment is bound to a different runtime pricing authority",
            ));
        }
        let identity = bound_identity.ok_or_else(|| {
            ProviderError::configuration(
                "model_identity",
                format!("deployment '{model}' lost its validated model identity"),
            )
        })?;
        let token_identity = token_identity_for_binding(identity, provider.name(), model)?;
        let identity = match identity.pricing_identity_for_surface(&surface) {
            DeploymentPricingIdentity::Priced { provider, model } => PricingIdentity::Priced {
                provider: provider.to_string(),
                model: model.to_string(),
            },
            DeploymentPricingIdentity::Unpriced => PricingIdentity::Unpriced {
                provider: provider.name().to_string(),
            },
            DeploymentPricingIdentity::NotApplicable => {
                let (pricing_provider, pricing_model) =
                    pricing_identity_for_provider(&snapshot, provider, model, surface);
                let identity = snapshot
                    .get_model_info_for_provider(&pricing_provider, &pricing_model)
                    .map_or_else(
                        || PricingIdentity::Unpriced {
                            provider: pricing_provider.clone(),
                        },
                        |(resolved, _)| PricingIdentity::Priced {
                            provider: pricing_provider.clone(),
                            model: resolved,
                        },
                    );
                return Ok(RequestPricing {
                    snapshot,
                    identity,
                    token_identity,
                });
            }
        };
        return Ok(RequestPricing {
            snapshot,
            identity,
            token_identity,
        });
    }

    let (pricing_provider, pricing_model) =
        pricing_identity_for_provider(&snapshot, provider, model, surface);
    let identity = snapshot
        .get_model_info_for_provider(&pricing_provider, &pricing_model)
        .map_or_else(
            || PricingIdentity::Unpriced {
                provider: pricing_provider.clone(),
            },
            |(resolved, _)| PricingIdentity::Priced {
                provider: pricing_provider.clone(),
                model: resolved,
            },
        );
    Ok(RequestPricing {
        snapshot,
        identity,
        token_identity: TokenizerIdentity::approximate(provider.name(), model),
    })
}

fn token_identity_for_binding(
    identity: &DeploymentModelIdentity,
    provider: &str,
    model: &str,
) -> Result<TokenizerIdentity, ProviderError> {
    match (
        identity.capability_catalog_provider(),
        identity.capability_catalog_model(),
    ) {
        (Some("openai"), Some(model)) => Ok(TokenizerIdentity::exact_openai(model)),
        (Some(provider), Some(model)) => Ok(TokenizerIdentity::approximate(provider, model)),
        _ => Err(ProviderError::configuration(
            "token_count",
            format!(
                "selected deployment '{provider}/{model}' has no validated capability token identity"
            ),
        )),
    }
}

pub(in crate::server::routes::ai) fn pricing_identity_for_provider(
    pricing_snapshot: &PricingSnapshot,
    provider: &Provider,
    model: &str,
    surface: ProviderCapability,
) -> (String, String) {
    let provider_name = provider.name();
    let mut provider_candidates = vec![provider_name.to_string()];

    match provider.provider_type() {
        ProviderType::OpenAI => provider_candidates.push("openai".to_string()),
        ProviderType::OpenAICompatible => {
            provider_candidates.push("openai_like".to_string());
            provider_candidates.push("openai".to_string());
        }
        ProviderType::Azure => provider_candidates.push("azure".to_string()),
        ProviderType::AzureAI => provider_candidates.push("azure_ai".to_string()),
        other => provider_candidates.push(other.to_string()),
    }

    let provider_candidates =
        provider_candidates
            .into_iter()
            .fold(Vec::new(), |mut unique, candidate| {
                if !unique.contains(&candidate) {
                    unique.push(candidate);
                }
                unique
            });

    let mapped_model = if matches!(
        surface,
        ProviderCapability::ChatCompletion | ProviderCapability::ChatCompletionStream
    ) {
        if let Provider::OpenAI(provider) = provider {
            Some(provider.config.get_model_mapping(model))
        } else {
            None
        }
    } else {
        None
    };
    let model_candidates = mapped_model
        .as_ref()
        .filter(|mapped| mapped.as_str() != model)
        .map_or_else(|| vec![model.to_string()], |mapped| vec![mapped.clone()]);

    for pricing_provider in &provider_candidates {
        for pricing_model in &model_candidates {
            if let Some((resolved_model, _)) =
                pricing_snapshot.get_model_info_for_provider(pricing_provider, pricing_model)
            {
                return (pricing_provider.clone(), resolved_model);
            }
        }
    }

    if let Some(identity) = mapped_model.as_deref().and_then(|mapped| {
        unpriced_openai_mapping_identity(&provider.provider_type(), provider_name, model, mapped)
    }) {
        return identity;
    }

    (provider_name.to_string(), model.to_string())
}

fn unpriced_openai_mapping_identity(
    provider_type: &ProviderType,
    pricing_provider: &str,
    requested_model: &str,
    mapped_model: &str,
) -> Option<(String, String)> {
    (mapped_model != requested_model
        && matches!(
            provider_type,
            ProviderType::OpenAI | ProviderType::OpenAICompatible
        ))
    .then(|| (pricing_provider.to_string(), mapped_model.to_string()))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::server::routes::ai) fn reserve_embedding_budget_with_request_pricing(
    request_pricing: &RequestPricing,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    input: &EmbeddingInput,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    let prompt_tokens = estimate_embedding_input_tokens(request_pricing.token_identity(), input)
        .map_err(|error| {
            super::token_count_error(
                budget_provider,
                budget_model,
                request_pricing.token_identity(),
                error,
            )
        })?;
    reserve_completion_budget_with_request_pricing(
        request_pricing,
        pricing_config,
        budget_limits,
        budget_provider,
        budget_model,
        prompt_tokens,
        Some(0),
    )
}

pub(in crate::server::routes::ai) fn reserve_pricing_usage_budget_with_request_pricing(
    request_pricing: &RequestPricing,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    usage: &PricingUsage,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    let cost = match request_pricing.calculate_usage(usage) {
        Ok(breakdown) => breakdown.total_cost,
        Err(error) => {
            return super::unpriced::reserve_unpriced_usage_budget(
                pricing_config,
                budget_limits,
                budget_provider,
                budget_model,
                usage,
                error,
            );
        }
    };
    if cost <= 0.0 {
        super::ensure_budget_available(budget_limits, budget_provider, budget_model)?;
        return Ok(None);
    }
    budget_limits
        .reserve_spend(budget_provider, budget_model, cost)
        .map(Some)
        .map_err(|error| {
            super::reservation_error_to_provider_error(error, budget_provider, budget_model)
        })
}

#[allow(clippy::too_many_arguments)]
pub(in crate::server::routes::ai) async fn record_pricing_usage_spend_with_request_pricing(
    request_pricing: &RequestPricing,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    key_manager: &KeyManager,
    api_key_id: Option<Uuid>,
    budget_provider: &str,
    budget_model: &str,
    usage: &PricingUsage,
    budget_reservation: Option<UnifiedBudgetReservation>,
    key_budget_reservation: Option<BudgetReservation>,
) {
    let cost = match request_pricing.calculate_settlement(usage) {
        Ok(breakdown) => breakdown.total_cost,
        Err(_) => {
            super::unpriced::settle_unpriced_usage(
                pricing_config,
                budget_limits,
                key_manager,
                api_key_id,
                budget_provider,
                budget_model,
                usage,
                budget_reservation,
                key_budget_reservation,
                "request pricing unavailable",
            )
            .await;
            return;
        }
    };
    if let Some(reservation) = budget_reservation {
        if let Err(error) = reservation.settle(cost) {
            tracing::error!("failed to settle reserved budget: {error:?}");
        }
    } else {
        budget_limits.record_spend(budget_provider, budget_model, cost);
    }
    super::settle_api_key_budget_reservation(
        key_budget_reservation,
        cost,
        &format!("{budget_provider}/{budget_model}"),
    );
    if let Some(key_id) = api_key_id {
        let total_tokens = super::unpriced::usage_units(usage);
        if let Err(error) = key_manager
            .record_usage(key_id, u64::from(total_tokens), cost)
            .await
        {
            tracing::error!("failed to record usage for key {key_id}: {error}");
        }
    }
}

fn reserve_completion_budget_with_request_pricing(
    request_pricing: &RequestPricing,
    pricing_config: &GatewayPricingConfig,
    budget_limits: &UnifiedBudgetLimits,
    budget_provider: &str,
    budget_model: &str,
    estimated_prompt_tokens: u32,
    max_output_tokens: Option<u32>,
) -> Result<Option<UnifiedBudgetReservation>, ProviderError> {
    let estimate =
        match request_pricing.estimate_completion(estimated_prompt_tokens, max_output_tokens) {
            Ok(estimate) => estimate,
            Err(error) => {
                return super::unpriced::reserve_unpriced_completion_budget(
                    pricing_config,
                    budget_limits,
                    budget_provider,
                    budget_model,
                    estimated_prompt_tokens,
                    max_output_tokens,
                    error,
                );
            }
        };
    if estimate.max_cost <= 0.0 {
        super::ensure_budget_available(budget_limits, budget_provider, budget_model)?;
        return Ok(None);
    }
    budget_limits
        .reserve_spend(budget_provider, budget_model, estimate.max_cost)
        .map(Some)
        .map_err(|error| {
            super::reservation_error_to_provider_error(error, budget_provider, budget_model)
        })
}

pub(in crate::server::routes::ai) fn estimate_embedding_input_tokens(
    identity: &TokenizerIdentity,
    input: &EmbeddingInput,
) -> crate::utils::error::gateway_error::Result<u32> {
    let counter = TokenCounter::new();
    input.iter().try_fold(0u32, |total, text| {
        let estimate = counter.count_completion_tokens(identity, text)?;
        if estimate.is_approximate {
            tracing::warn!(
                token_provider = identity.provider(),
                token_model = identity.model(),
                input = "embedding",
                is_approximate = true,
                confidence = estimate.confidence,
                "explicit approximate token count used"
            );
        }
        Ok(total.saturating_add(estimate.input_tokens))
    })
}

#[cfg(test)]
#[path = "pricing_tests.rs"]
mod tests;
