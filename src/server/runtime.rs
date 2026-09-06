//! Validated atomic runtime revision for provider/router/guardrail/cache state.
//!
//! [`AppState::apply_runtime`] builds a complete candidate [`RuntimeRevision`]
//! before one lock-free store publishes it. [`AppState::pin_runtime`] holds that
//! generation for a request lifecycle. This module does not watch config files
//! or expose admin CRUD.

use crate::config::Config;
use crate::core::cache::{DualCacheConfig, LLMCache, LLMCacheConfig};
use crate::core::guardrails::GuardrailEngine;
use crate::core::pricing_service::PricingService;
use crate::core::router::UnifiedRouter;
use crate::storage::redis::RedisPool;
use crate::utils::error::gateway_error::{GatewayError, Result};
use std::sync::Arc;
use std::time::Duration;
use tracing::error;

use super::http_runtime::build_router_from_config;

/// One consistent generation of swappable runtime-owned components.
#[derive(Clone)]
pub struct RuntimeRevision {
    /// Monotonic apply generation. Startup is `0`; each successful apply adds `1`.
    pub generation: u64,
    /// Validated gateway configuration for this generation.
    pub config: Arc<Config>,
    /// Router (including provider health probes) built from this generation.
    pub unified_router: Arc<UnifiedRouter>,
    /// Content guardrails built from this generation.
    pub guardrails: Arc<GuardrailEngine>,
    /// Response cache policy built from this generation.
    pub response_cache: Option<Arc<LLMCache>>,
}

pub(super) async fn build_runtime_revision(
    config: Config,
    generation: u64,
    pricing: Arc<PricingService>,
    redis: Arc<RedisPool>,
) -> Result<RuntimeRevision> {
    config.validate()?;
    let unified_router = Arc::new(
        build_router_from_config(&config, pricing)
            .await?
            .with_admission_redis(Arc::clone(&redis)),
    );
    let guardrails =
        GuardrailEngine::shared(config.gateway.guardrails.clone()).map_err(|error| {
            GatewayError::Config(format!("Invalid guardrails configuration: {error}"))
        })?;
    let response_cache = build_response_cache(&config, redis);
    Ok(RuntimeRevision {
        generation,
        config: Arc::new(config),
        unified_router,
        guardrails,
        response_cache,
    })
}

pub(super) fn build_response_cache(
    config: &Config,
    redis: Arc<RedisPool>,
) -> Option<Arc<LLMCache>> {
    if !config.gateway.cache.enabled {
        return None;
    }

    if config.gateway.cache.ttl == 0 {
        error!("cache.enabled=true requires cache.ttl > 0; response cache disabled");
        return None;
    }

    let ttl = Duration::from_secs(config.gateway.cache.ttl);
    let redis_pool = (!redis.is_noop()).then_some(redis);
    let cache_config = if redis_pool.is_some() {
        DualCacheConfig::default()
    } else {
        DualCacheConfig::memory_only()
    }
    .with_max_size(config.gateway.cache.max_size)
    .with_ttl(ttl);
    let llm_config = LLMCacheConfig {
        cache_config,
        chat_ttl: ttl,
        embedding_ttl: ttl,
        user_specific: true,
        semantic_cache_enabled: false,
        similarity_threshold: config.gateway.cache.similarity_threshold,
    };
    let cache = Arc::new(LLMCache::new(llm_config, redis_pool));
    cache.start_cleanup_tasks();
    Some(cache)
}
