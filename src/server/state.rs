//! Application state shared across HTTP handlers
//!
//! This module provides the AppState struct and its implementations.

use crate::config::Config;
use crate::core::audit::AuditLogger;
use crate::core::budget::{BudgetManager, UnifiedBudgetLimits};
use crate::core::cache::LLMCache;
use crate::core::guardrails::GuardrailEngine;
use crate::core::ip_access::IpAccessControl;
use crate::core::keys::{DatabaseKeyRepository, KeyManager};
use crate::core::observability::RuntimeObservability;
use crate::core::pricing_service::PricingService;
use crate::core::router::UnifiedRouter;
use crate::core::teams::TeamManager;
use crate::core::virtual_keys::RuntimeVirtualKeyManager;
use crate::server::routes::ai::budgeted::BudgetedExecutor;
use crate::storage::database::SeaOrmTeamRepository;
use crate::utils::error::gateway_error::{GatewayError, Result};
use crate::utils::sync::AtomicValue;
use std::sync::Arc;
use tokio::sync::Mutex;

pub use super::runtime::RuntimeRevision;
use super::runtime::{build_response_cache, build_runtime_revision};

/// HTTP server state shared across handlers
///
/// This struct contains shared resources that need to be accessed across
/// multiple request handlers. All fields are wrapped in Arc for efficient
/// sharing across threads.
///
/// `config` uses [`AtomicValue`] so callers can explicitly swap the entire
/// configuration at runtime while readers obtain lock-free `Arc<Config>`
/// snapshots. This type does not start a file watcher by itself.
///
/// Router, guardrails, and response cache are published together through
/// [`Self::apply_runtime`]. [`Self::pin_runtime`] holds one generation for a
/// request lifecycle. Live accessors load from that same bundle so HTTP traffic
/// never stays on construction-time Arcs after a successful apply.
#[derive(Clone)]
pub struct AppState {
    /// Gateway configuration (atomically swappable by explicit callers)
    pub config: AtomicValue<Config>,
    /// Authentication system
    pub auth: Arc<crate::auth::AuthSystem>,
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
    pub key_manager: RuntimeVirtualKeyManager,
    /// Budget orchestration service for AI route reserve/call/settle lifecycles
    pub(crate) budgeted: BudgetedExecutor,
    /// Non-blocking external request lifecycle callback dispatcher
    pub callbacks: RuntimeObservability,
    /// Explicitly configured request audit logger.
    pub audit_logger: Arc<AuditLogger>,
    /// IP policy consumed by the outer HTTP middleware.
    pub ip_access: Arc<IpAccessControl>,
    runtime: AtomicValue<RuntimeRevision>,
    apply_lock: Arc<Mutex<()>>,
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
        let redis = storage.redis.clone();
        let config = Arc::new(config);
        let revision = RuntimeRevision {
            generation: 0,
            unified_router: Arc::new(unified_router),
            guardrails: Arc::new(GuardrailEngine::disabled()),
            response_cache: build_response_cache(&config, redis),
            config: Arc::clone(&config),
        };
        Self::new_with_runtime(revision, auth, storage, pricing, budget_limits)
    }

    /// Create AppState from a fully built runtime revision.
    pub fn new_with_runtime(
        revision: RuntimeRevision,
        auth: crate::auth::AuthSystem,
        storage: crate::storage::StorageLayer,
        pricing: Arc<PricingService>,
        budget_limits: Arc<UnifiedBudgetLimits>,
    ) -> Self {
        let storage = Arc::new(storage);
        let key_manager = KeyManager::new(DatabaseKeyRepository::new(storage.clone()))
            .with_hmac_secret(revision.config.gateway.auth.api_key_hmac_secret.clone());
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
            config: AtomicValue::from(Arc::clone(&revision.config)),
            auth: Arc::new(auth),
            storage,
            pricing,
            budget_limits,
            budget_manager,
            team_manager,
            key_manager,
            budgeted,
            callbacks: RuntimeObservability::disabled(),
            audit_logger: Arc::new(AuditLogger::disabled()),
            ip_access: Arc::new(IpAccessControl::disabled()),
            runtime: AtomicValue::new(revision),
            apply_lock: Arc::new(Mutex::new(())),
        }
    }

    /// Attach a configured callback dispatcher.
    pub fn with_callbacks(mut self, callbacks: RuntimeObservability) -> Self {
        self.callbacks = callbacks;
        self
    }

    /// Attach the request audit logger built during startup.
    pub fn with_audit_logger(mut self, audit_logger: Arc<AuditLogger>) -> Self {
        self.audit_logger = audit_logger;
        self
    }

    /// Attach validated content and network policy engines.
    pub fn with_request_policies(
        self,
        guardrails: Arc<GuardrailEngine>,
        ip_access: Arc<IpAccessControl>,
    ) -> Self {
        let current = self.runtime.load();
        self.runtime.store(RuntimeRevision {
            generation: current.generation,
            config: Arc::clone(&current.config),
            unified_router: Arc::clone(&current.unified_router),
            guardrails,
            response_cache: current.response_cache.clone(),
        });
        let mut this = self;
        this.ip_access = ip_access;
        this
    }

    /// Lock-free pin of the active runtime generation.
    ///
    /// Clone the returned `Arc` (or clone this pin) to keep router, guardrails,
    /// cache, config, and generation stable for the rest of a request even if
    /// [`Self::apply_runtime`] publishes a newer revision.
    pub fn pin_runtime(&self) -> Arc<RuntimeRevision> {
        self.runtime.load()
    }

    /// Live router for the current generation.
    pub fn unified_router(&self) -> Arc<UnifiedRouter> {
        Arc::clone(&self.pin_runtime().unified_router)
    }

    /// Live guardrail engine for the current generation.
    pub fn guardrails(&self) -> Arc<GuardrailEngine> {
        Arc::clone(&self.pin_runtime().guardrails)
    }

    /// Live response cache for the current generation.
    pub fn response_cache(&self) -> Option<Arc<LLMCache>> {
        self.pin_runtime().response_cache.clone()
    }

    /// Validate and build a complete candidate revision, then publish it atomically.
    ///
    /// Any build failure leaves the previous revision fully active. On success,
    /// router, provider health, guardrails, response cache, generation, and
    /// [`Self::config`] observe one consistent generation. Pricing is reused
    /// from [`Self::pricing`] rather than rebuilt.
    pub async fn apply_runtime(&self, candidate: Config) -> Result<u64> {
        let _guard = self.apply_lock.lock().await;
        let generation = self
            .pin_runtime()
            .generation
            .checked_add(1)
            .ok_or_else(|| GatewayError::Config("runtime generation overflow".into()))?;
        let revision = build_runtime_revision(
            candidate,
            generation,
            Arc::clone(&self.pricing),
            Arc::clone(&self.storage.redis),
        )
        .await?;
        let config = Arc::clone(&revision.config);
        self.runtime.store(revision);
        self.config.store_arc(config);
        Ok(generation)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::provider::ProviderConfig;
    use crate::core::guardrails::PromptInjectionConfig;
    use crate::server::HttpServer;

    async fn test_app_state() -> AppState {
        let mut config = crate::server::valid_test_config();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;
        match HttpServer::new(&config).await {
            Ok(server) => server.state().clone(),
            Err(error) => panic!("test AppState startup failed: {error}"),
        }
    }

    fn extra_provider(name: &str) -> ProviderConfig {
        ProviderConfig {
            name: name.to_string(),
            provider_type: "openai".to_string(),
            api_key: "sk-test".to_string(),
            ..ProviderConfig::default()
        }
    }

    #[tokio::test]
    async fn apply_runtime_successful_swap_publishes_one_generation() {
        let state = test_app_state().await;
        let pinned = state.pin_runtime();
        assert_eq!(pinned.generation, 0);
        assert!(pinned.response_cache.is_none());

        let mut next = (*state.config()).clone();
        next.gateway.cache.enabled = true;
        next.gateway.guardrails.enabled = false;
        next.gateway.providers.push(extra_provider("test-openai-2"));

        let generation = state
            .apply_runtime(next)
            .await
            .expect("valid candidate should apply");
        assert_eq!(generation, 1);

        let live = state.pin_runtime();
        assert_eq!(live.generation, 1);
        assert!(live.response_cache.is_some());
        assert!(!live.guardrails.is_enabled());
        assert!(
            live.unified_router.list_deployments().len()
                > pinned.unified_router.list_deployments().len()
        );
        assert!(Arc::ptr_eq(&live.config, &state.config.load()));
        assert!(!Arc::ptr_eq(&live.unified_router, &pinned.unified_router));
        assert!(!Arc::ptr_eq(&live.guardrails, &pinned.guardrails));

        assert_eq!(pinned.generation, 0);
        assert!(pinned.response_cache.is_none());
        assert!(pinned.guardrails.is_enabled());
    }

    #[tokio::test]
    async fn apply_runtime_failed_build_leaves_previous_revision_active() {
        let state = test_app_state().await;
        let before = state.pin_runtime();
        let router = Arc::clone(&before.unified_router);

        let mut invalid = (*state.config()).clone();
        let mut injection = invalid
            .gateway
            .guardrails
            .prompt_injection
            .clone()
            .unwrap_or_else(PromptInjectionConfig::new);
        injection.enabled = true;
        injection.custom_patterns.push("(".to_string());
        invalid.gateway.guardrails.prompt_injection = Some(injection);

        let error = state
            .apply_runtime(invalid)
            .await
            .expect_err("invalid guardrails must fail apply");
        match error {
            GatewayError::Config(message) => {
                assert!(
                    message.to_lowercase().contains("guardrail")
                        || message.to_lowercase().contains("pattern"),
                    "unexpected apply error: {message}"
                );
            }
            other => panic!("expected config error, got {other:?}"),
        }

        let after = state.pin_runtime();
        assert_eq!(after.generation, before.generation);
        assert!(Arc::ptr_eq(&after.unified_router, &router));
        assert!(Arc::ptr_eq(&after.guardrails, &before.guardrails));
        assert_eq!(after.generation, 0);
    }

    #[tokio::test]
    async fn in_flight_pin_stays_on_old_generation_after_apply() {
        let state = test_app_state().await;
        let in_flight = Arc::clone(&state.pin_runtime());

        let mut next = (*state.config()).clone();
        next.gateway.cache.enabled = true;
        next.gateway.guardrails.enabled = false;
        next.gateway
            .providers
            .push(extra_provider("test-openai-inflight"));

        state
            .apply_runtime(next)
            .await
            .expect("valid candidate should apply");

        let live = state.pin_runtime();
        assert_eq!(live.generation, in_flight.generation + 1);
        assert_eq!(in_flight.generation, 0);
        assert!(!Arc::ptr_eq(
            &in_flight.unified_router,
            &live.unified_router
        ));
        assert!(!Arc::ptr_eq(&in_flight.guardrails, &live.guardrails));
        assert!(in_flight.response_cache.is_none());
        assert!(live.response_cache.is_some());
    }
}
