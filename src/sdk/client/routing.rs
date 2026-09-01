//! Provider selection and routing methods

use super::llm_client::LLMClient;
use super::types::{LoadBalancingStrategy, ProviderStats};
use crate::core::providers::registry::{LegacyAdapterSurface, supports_legacy_adapter};
use crate::core::router::UnifiedRouter;
use crate::core::router::UnifiedRoutingStrategy;
use crate::core::router::deployment::DeploymentId;
use crate::core::router::strategy_impl::RoutingContext;
use crate::sdk::config::{ProviderType, SdkProviderConfig};
use crate::sdk::errors::*;
use crate::sdk::types::{Message, SdkChatRequest};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

impl LLMClient {
    /// Select best provider for a request
    pub(crate) async fn select_provider(
        &self,
        request: &SdkChatRequest,
    ) -> Result<&crate::sdk::config::SdkProviderConfig> {
        if !request.model.is_empty() {
            return self
                .load_balancer
                .select_chat_provider(
                    &self.config.providers,
                    &self.provider_stats,
                    Some(&request.model),
                )
                .await;
        }

        if let Some(provider) = self.default_enabled_provider() {
            return if sdk_provider_has_legacy_adapter(provider, LegacyAdapterSurface::SdkChat) {
                Ok(provider)
            } else {
                Err(unsupported_legacy_sdk_adapter_error(
                    provider,
                    LegacyAdapterSurface::SdkChat,
                ))
            };
        }

        self.load_balancer
            .select_chat_provider(&self.config.providers, &self.provider_stats, None)
            .await
    }

    /// Select provider for streaming
    pub(crate) async fn select_provider_for_stream(
        &self,
        _messages: &[Message],
    ) -> Result<&crate::sdk::config::SdkProviderConfig> {
        if let Some(provider) = self.default_enabled_provider() {
            return if sdk_provider_has_legacy_adapter(provider, LegacyAdapterSurface::SdkChatStream)
            {
                Ok(provider)
            } else {
                Err(unsupported_legacy_sdk_adapter_error(
                    provider,
                    LegacyAdapterSurface::SdkChatStream,
                ))
            };
        }

        self.load_balancer
            .select_stream_provider(&self.config.providers, &self.provider_stats)
            .await
    }
}

// Load balancer implementation
use super::types::LoadBalancer;

impl LoadBalancer {
    /// Select a chat-capable provider using the configured load balancing strategy.
    pub(crate) async fn select_chat_provider<'a>(
        &self,
        providers: &'a [SdkProviderConfig],
        stats: &Arc<RwLock<HashMap<String, ProviderStats>>>,
        model: Option<&str>,
    ) -> Result<&'a SdkProviderConfig> {
        self.select_provider_with_capability(
            providers,
            stats,
            model,
            LegacyAdapterSurface::SdkChat,
            supports_chat,
        )
        .await
    }

    /// Select a streaming-capable provider using the configured load balancing strategy.
    pub(crate) async fn select_stream_provider<'a>(
        &self,
        providers: &'a [SdkProviderConfig],
        stats: &Arc<RwLock<HashMap<String, ProviderStats>>>,
    ) -> Result<&'a SdkProviderConfig> {
        self.select_provider_with_capability(
            providers,
            stats,
            None,
            LegacyAdapterSurface::SdkChatStream,
            supports_stream,
        )
        .await
    }

    async fn select_provider_with_capability<'a>(
        &self,
        providers: &'a [SdkProviderConfig],
        stats: &Arc<RwLock<HashMap<String, ProviderStats>>>,
        model: Option<&str>,
        surface: LegacyAdapterSurface,
        supports_capability: impl Fn(&SdkProviderConfig) -> bool,
    ) -> Result<&'a SdkProviderConfig> {
        let model_candidates: Vec<&SdkProviderConfig> = providers
            .iter()
            .filter(|provider| {
                provider.enabled
                    && model.is_none_or(|model| {
                        provider.models.iter().any(|candidate| candidate == model)
                    })
            })
            .collect();

        let enabled_providers: Vec<&SdkProviderConfig> = model_candidates
            .iter()
            .copied()
            .filter(|provider| supports_capability(provider))
            .collect();

        if enabled_providers.is_empty() {
            if let Some(provider) = model_candidates.first() {
                return Err(unsupported_legacy_sdk_adapter_error(provider, surface));
            }

            return match model {
                Some(model) => Err(SDKError::ModelNotFound(format!(
                    "Model '{}' not supported by any provider",
                    model
                ))),
                None => Err(SDKError::NoDefaultProvider),
            };
        }

        let stats_guard = stats.read().await;
        let deployment_ids: Vec<DeploymentId> = enabled_providers
            .iter()
            .map(|provider| provider.id.clone())
            .collect();
        let contexts: Vec<RoutingContext<'_>> = enabled_providers
            .iter()
            .zip(deployment_ids.iter())
            .map(|(provider, deployment_id)| {
                let provider_stats = stats_guard.get(&provider.id);
                RoutingContext {
                    deployment_id,
                    weight: sdk_weight_to_router_weight(provider.weight),
                    priority: health_score_to_priority(
                        provider_stats
                            .map(|stats| stats.health_score)
                            .unwrap_or(1.0),
                    ),
                    active_requests: 0,
                    tpm_current: 0,
                    tpm_limit: provider.rate_limit_tpm.map(u64::from),
                    rpm_current: 0,
                    rpm_limit: provider.rate_limit_rpm.map(u64::from),
                    avg_latency_us: provider_stats
                        .map(|stats| latency_ms_to_us(stats.avg_latency_ms))
                        .unwrap_or(0),
                }
            })
            .collect();

        let route_key = model.unwrap_or("__sdk_default__");
        let selected_id = UnifiedRouter::select_from_routing_contexts(
            self.core_strategy(),
            route_key,
            &contexts,
            &self.round_robin_counters,
        )
        .ok_or_else(|| match model {
            Some(model) => {
                SDKError::ModelNotFound(format!("Model '{}' not supported by any provider", model))
            }
            None => SDKError::NoDefaultProvider,
        })?;

        enabled_providers
            .into_iter()
            .find(|provider| provider.id == *selected_id)
            .ok_or_else(|| SDKError::ProviderNotFound(selected_id.clone()))
    }

    fn core_strategy(&self) -> UnifiedRoutingStrategy {
        match self.strategy {
            LoadBalancingStrategy::RoundRobin => UnifiedRoutingStrategy::RoundRobin,
            LoadBalancingStrategy::LeastLatency => UnifiedRoutingStrategy::LatencyBased,
            LoadBalancingStrategy::WeightedRandom => UnifiedRoutingStrategy::SimpleShuffle,
            LoadBalancingStrategy::HealthBased => UnifiedRoutingStrategy::PriorityBased,
        }
    }
}

fn supports_chat(provider: &SdkProviderConfig) -> bool {
    sdk_provider_has_legacy_adapter(provider, LegacyAdapterSurface::SdkChat)
}

fn supports_stream(provider: &SdkProviderConfig) -> bool {
    sdk_provider_has_legacy_adapter(provider, LegacyAdapterSurface::SdkChatStream)
}

pub(crate) fn sdk_provider_has_legacy_adapter(
    provider: &SdkProviderConfig,
    surface: LegacyAdapterSurface,
) -> bool {
    supports_legacy_adapter(&sdk_provider_matrix_selector(provider), surface)
}

pub(crate) fn sdk_provider_matrix_selector(provider: &SdkProviderConfig) -> String {
    match &provider.provider_type {
        ProviderType::OpenAI => "openai",
        ProviderType::Anthropic => "anthropic",
        ProviderType::Azure => "azure",
        ProviderType::Google => "google",
        ProviderType::Cohere => "cohere",
        ProviderType::HuggingFace => "huggingface",
        ProviderType::Ollama => "ollama",
        ProviderType::AwsBedrock => "bedrock",
        ProviderType::GoogleVertex => "vertex_ai",
        ProviderType::Mistral => "mistral",
        ProviderType::Custom(_) => "sdk_custom",
    }
    .to_string()
}

pub(crate) fn unsupported_legacy_sdk_adapter_error(
    provider: &SdkProviderConfig,
    surface: LegacyAdapterSurface,
) -> SDKError {
    SDKError::NotSupported(format!(
        "SDK {} is not supported for provider '{}' ({:?})",
        legacy_sdk_adapter_name(surface),
        provider.id,
        provider.provider_type
    ))
}

fn legacy_sdk_adapter_name(surface: LegacyAdapterSurface) -> &'static str {
    match surface {
        LegacyAdapterSurface::SdkChat => "chat",
        LegacyAdapterSurface::SdkChatStream => "chat streaming",
        LegacyAdapterSurface::SdkEmbeddings => "embeddings",
        _ => "surface",
    }
}

fn sdk_weight_to_router_weight(weight: f32) -> u32 {
    if !weight.is_finite() || weight <= 0.0 {
        return 0;
    }

    (weight * 1_000.0).round().clamp(1.0, u32::MAX as f32) as u32
}

fn health_score_to_priority(score: f64) -> u32 {
    let normalized = if score.is_finite() {
        score.clamp(0.0, 1.0)
    } else {
        0.0
    };
    ((1.0 - normalized) * 100_000.0).round() as u32
}

fn latency_ms_to_us(latency_ms: f64) -> u64 {
    if !latency_ms.is_finite() || latency_ms <= 0.0 {
        return 0;
    }

    (latency_ms * 1_000.0).round().clamp(0.0, u64::MAX as f64) as u64
}
