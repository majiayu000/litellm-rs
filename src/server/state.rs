//! Application state shared across HTTP handlers
//!
//! This module provides the AppState struct and its implementations.

use crate::config::Config;
use crate::core::budget::{BudgetManager, UnifiedBudgetLimits};
use crate::core::cache::{DualCacheConfig, LLMCache, LLMCacheConfig};
use crate::core::integrations::CallbackDispatcher;
use crate::core::keys::{DatabaseKeyRepository, KeyManager};
use crate::core::pricing_service::PricingService;
use crate::core::teams::TeamManager;
use crate::server::routes::ai::budgeted::BudgetedExecutor;
use crate::storage::database::SeaOrmTeamRepository;
use crate::storage::redis::RedisPool;
use crate::utils::sync::AtomicValue;
use std::sync::Arc;
use std::time::Duration;
use tracing::error;

/// HTTP server state shared across handlers
///
/// This struct contains shared resources that need to be accessed across
/// multiple request handlers. All fields are wrapped in Arc for efficient
/// sharing across threads.
///
/// `config` uses [`AtomicValue`] so callers can explicitly swap the entire
/// configuration at runtime while readers obtain lock-free `Arc<Config>`
/// snapshots. This type does not start a file watcher by itself.
#[derive(Clone)]
pub struct AppState {
    /// Gateway configuration (atomically swappable by explicit callers)
    pub config: AtomicValue<Config>,
    /// Authentication system
    pub auth: Arc<crate::auth::AuthSystem>,
    /// Unified router (new UnifiedRouter implementation)
    pub unified_router: Arc<crate::core::router::UnifiedRouter>,
    /// Storage layer
    pub storage: Arc<crate::storage::StorageLayer>,
    /// Unified pricing service
    pub pricing: Arc<PricingService>,
    /// Budget limits for provider and model cost tracking
    pub budget_limits: Arc<UnifiedBudgetLimits>,
    /// General budget manager for keyed budget scopes such as API key budgets
    pub budget_manager: Arc<BudgetManager>,
    /// Team manager for team lifecycle operations (shared, in-memory by default)
    pub team_manager: Arc<TeamManager>,
    /// API key manager for `/v1/keys` route handlers (shared across requests)
    pub key_manager: KeyManager,
    /// Budget orchestration service for AI route reserve/call/settle lifecycles
    pub(crate) budgeted: BudgetedExecutor,
    /// Optional deterministic response cache for non-streaming chat and embeddings
    pub response_cache: Option<Arc<LLMCache>>,
    /// Non-blocking external request lifecycle callback dispatcher
    pub callbacks: CallbackDispatcher,
}

impl AppState {
    /// Create a new AppState with unified router
    pub fn new_with_unified_router(
        config: Config,
        auth: crate::auth::AuthSystem,
        unified_router: crate::core::router::UnifiedRouter,
        storage: crate::storage::StorageLayer,
        pricing: Arc<PricingService>,
        budget_limits: Arc<UnifiedBudgetLimits>,
    ) -> Self {
        let storage = Arc::new(storage);
        let response_cache = build_response_cache(&config, storage.redis.clone());
        let key_manager = KeyManager::new(DatabaseKeyRepository::new(storage.clone()))
            .with_hmac_secret(config.gateway.auth.api_key_hmac_secret.clone());
        let budget_manager = Arc::new(BudgetManager::new());
        let budgeted = BudgetedExecutor::new(
            budget_limits.clone(),
            budget_manager.clone(),
            pricing.clone(),
            key_manager.clone(),
        );
        let team_manager = Arc::new(TeamManager::new(Arc::new(SeaOrmTeamRepository::new(
            storage.database.clone(),
        ))));
        Self {
            config: AtomicValue::new(config),
            auth: Arc::new(auth),
            unified_router: Arc::new(unified_router),
            storage,
            pricing,
            budget_limits,
            budget_manager,
            team_manager,
            key_manager,
            budgeted,
            response_cache,
            callbacks: CallbackDispatcher::disabled(),
        }
    }

    /// Attach a configured callback dispatcher.
    pub fn with_callbacks(mut self, callbacks: CallbackDispatcher) -> Self {
        self.callbacks = callbacks;
        self
    }

    /// Load a snapshot of the current gateway configuration.
    ///
    /// Returns an `Arc<Config>` that is valid for the lifetime of the
    /// caller — subsequent explicit swaps will not affect already-loaded
    /// snapshots.
    pub fn config(&self) -> Arc<Config> {
        self.config.load()
    }
}

fn build_response_cache(config: &Config, redis: Arc<RedisPool>) -> Option<Arc<LLMCache>> {
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
