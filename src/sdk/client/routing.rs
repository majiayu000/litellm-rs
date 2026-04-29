//! Provider selection and routing methods

use super::llm_client::LLMClient;
use super::types::{LoadBalancingStrategy, ProviderStats};
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
            return self.provider_for_model(&request.model);
        }

        if let Some(provider) = self.default_enabled_provider() {
            return Ok(provider);
        }

        self.load_balancer
            .select_provider(&self.config.providers, &self.provider_stats)
            .await
    }

    /// Select provider for streaming
    pub(crate) async fn select_provider_for_stream(
        &self,
        _messages: &[Message],
    ) -> Result<&crate::sdk::config::SdkProviderConfig> {
        if let Some(provider) = self.default_enabled_provider() {
            return Ok(provider);
        }

        self.first_enabled_provider()
    }
}

// Load balancer implementation
use std::sync::atomic::Ordering;

use super::types::LoadBalancer;

impl LoadBalancer {
    /// Select provider using load balancing strategy
    pub(crate) async fn select_provider<'a>(
        &self,
        providers: &'a [crate::sdk::config::SdkProviderConfig],
        stats: &Arc<RwLock<HashMap<String, ProviderStats>>>,
    ) -> Result<&'a crate::sdk::config::SdkProviderConfig> {
        let enabled_providers: Vec<&crate::sdk::config::SdkProviderConfig> =
            providers.iter().filter(|p| p.enabled).collect();

        if enabled_providers.is_empty() {
            return Err(SDKError::NoDefaultProvider);
        }

        match self.strategy {
            LoadBalancingStrategy::RoundRobin => {
                let idx = self.round_robin_counter.fetch_add(1, Ordering::Relaxed)
                    % enabled_providers.len();
                Ok(enabled_providers[idx])
            }
            LoadBalancingStrategy::WeightedRandom => {
                // Weighted random selection
                use rand::Rng;
                let total_weight: f32 = enabled_providers.iter().map(|p| p.weight).sum();
                let mut rng = rand::rng();
                let mut random_weight = rng.random::<f32>() * total_weight;

                for provider in &enabled_providers {
                    random_weight -= provider.weight;
                    if random_weight <= 0.0 {
                        return Ok(provider);
                    }
                }

                Ok(enabled_providers[0])
            }
            LoadBalancingStrategy::HealthBased => {
                // Health-based selection
                let stats_guard = stats.read().await;
                let mut best_provider = enabled_providers[0];
                let mut best_score = 0.0f64;

                for provider in enabled_providers {
                    let health_score = stats_guard
                        .get(&provider.id)
                        .map(|s| s.health_score)
                        .unwrap_or(1.0);

                    if health_score > best_score {
                        best_score = health_score;
                        best_provider = provider;
                    }
                }

                Ok(best_provider)
            }
            LoadBalancingStrategy::LeastLatency => {
                // Latency-based selection
                let stats_guard = stats.read().await;
                let mut best_provider = enabled_providers[0];
                let mut best_latency = f64::INFINITY;

                for provider in enabled_providers {
                    let latency = stats_guard
                        .get(&provider.id)
                        .map(|s| s.avg_latency_ms)
                        .unwrap_or(0.0);

                    if latency < best_latency {
                        best_latency = latency;
                        best_provider = provider;
                    }
                }

                Ok(best_provider)
            }
        }
    }
}
