//! Application state shared across HTTP handlers
//!
//! This module provides the AppState struct and its implementations.

use crate::config::Config;
use crate::core::budget::UnifiedBudgetLimits;
use crate::core::teams::{InMemoryTeamRepository, TeamManager};
use crate::services::pricing::PricingService;
use crate::utils::sync::AtomicValue;
use std::sync::Arc;

/// HTTP server state shared across handlers
///
/// This struct contains shared resources that need to be accessed across
/// multiple request handlers. All fields are wrapped in Arc for efficient
/// sharing across threads.
///
/// `config` uses [`AtomicValue`] so the entire configuration can be swapped
/// atomically at runtime (hot reload) while readers obtain lock-free
/// `Arc<Config>` snapshots.
#[derive(Clone)]
pub struct AppState {
    /// Gateway configuration (atomically swappable for hot reload)
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
    /// Team manager for team lifecycle operations (shared, in-memory by default)
    pub team_manager: Arc<TeamManager>,
}

impl AppState {
    /// Create a new AppState with unified router
    pub fn new_with_unified_router(
        config: Config,
        auth: crate::auth::AuthSystem,
        unified_router: crate::core::router::UnifiedRouter,
        storage: crate::storage::StorageLayer,
        pricing: Arc<PricingService>,
    ) -> Self {
        let team_manager = Arc::new(TeamManager::new(Arc::new(InMemoryTeamRepository::new())));
        Self {
            config: AtomicValue::new(config),
            auth: Arc::new(auth),
            unified_router: Arc::new(unified_router),
            storage: Arc::new(storage),
            pricing,
            budget_limits: Arc::new(UnifiedBudgetLimits::new()),
            team_manager,
        }
    }

    /// Load a snapshot of the current gateway configuration.
    ///
    /// Returns an `Arc<Config>` that is valid for the lifetime of the
    /// caller — subsequent hot-reload swaps will not affect already-loaded
    /// snapshots.
    pub fn config(&self) -> Arc<Config> {
        self.config.load()
    }
}
