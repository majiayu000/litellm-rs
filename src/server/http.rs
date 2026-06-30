//! HTTP server core implementation
//!
//! This module provides the HttpServer struct and its core methods.

use crate::config::models::server::{CorsConfig, ServerConfig};
use crate::config::{Config, Validate};
use crate::core::budget::UnifiedBudgetLimits;
use crate::core::pricing_service::PricingService;
use crate::core::rate_limiter::{get_global_rate_limiter, init_global_rate_limiter_with_redis};
use crate::server::middleware::{
    AuthMiddleware, RateLimitMiddleware, RequestIdMiddleware, SecurityHeadersMiddleware,
    start_auth_rate_limiter_cleanup_task,
};
use crate::server::routes;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::{GatewayError, Result};
use actix_cors::Cors;
use actix_web::{
    App, HttpServer as ActixHttpServer,
    middleware::{Condition, DefaultHeaders, Logger},
    web,
};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

/// HTTP server
pub struct HttpServer {
    /// Server configuration
    config: ServerConfig,
    /// Application state
    state: AppState,
    /// Background worker that drains budget persistence events on shutdown.
    budget_persistence_task: Option<JoinHandle<()>>,
}

impl HttpServer {
    /// Create a new HTTP server
    pub async fn new(config: &Config) -> Result<Self> {
        info!("Creating HTTP server");

        Self::validate_cors_config(&config.gateway.server.cors)?;
        config
            .gateway
            .cache
            .validate()
            .map_err(|e| GatewayError::Config(format!("Invalid cache configuration: {}", e)))?;
        start_auth_rate_limiter_cleanup_task();

        let storage = crate::storage::StorageLayer::new(&config.gateway.storage).await?;
        let mut budget_persistence_task = None;
        // Budget persistence is co-located with the database backend. We treat
        // a missing/disabled database as "budget persistence disabled" (no
        // error), and a failed snapshot load on a configured database as a
        // hard startup failure unless storage.database.allow_degraded=true.
        let budget_limits = match storage.database.load_budget_limit_snapshots().await {
            Ok(snapshots) => {
                let count = snapshots.len();
                let (persistence_tx, persistence_task) =
                    Arc::clone(&storage.database).start_budget_limit_persistence_task();
                budget_persistence_task = Some(persistence_task);
                info!("Loaded {} persisted budget limit snapshots", count);
                Arc::new(UnifiedBudgetLimits::from_snapshots_with_persistence(
                    snapshots,
                    persistence_tx,
                ))
            }
            Err(e) => {
                if config.gateway.storage.database.allow_degraded
                    || !config.gateway.storage.database.enabled
                {
                    error!(
                        "Budget limit persistence is unavailable; using in-memory budgets \
                         only (allow_degraded={}, db_enabled={}). Error: {}",
                        config.gateway.storage.database.allow_degraded,
                        config.gateway.storage.database.enabled,
                        e
                    );
                    Arc::new(UnifiedBudgetLimits::new())
                } else {
                    error!(
                        "Budget limit persistence load failed and \
                         storage.database.allow_degraded=false; failing startup. Set \
                         storage.database.allow_degraded=true to keep running with \
                         in-memory budgets only. Error: {}",
                        e
                    );
                    return Err(e);
                }
            }
        };
        let auth =
            crate::auth::AuthSystem::new(&config.gateway.auth, Arc::new(storage.clone())).await?;

        let pricing = Arc::new(PricingService::new(config.gateway.pricing.source.clone()));
        if let Err(e) = pricing.initialize().await {
            // A `None` pricing source is "pricing disabled" and is not
            // expected to fail; any other failure is a configured-but-broken
            // pricing source. Honor allow_degraded the same way as other
            // dependencies.
            let is_configured = config.gateway.pricing.source.is_some();
            if !is_configured || config.gateway.pricing.allow_degraded {
                error!(
                    "Pricing service initial load failed; gateway will serve traffic \
                     without pricing data (configured={}, allow_degraded={}). Error: {}",
                    is_configured, config.gateway.pricing.allow_degraded, e
                );
            } else {
                error!(
                    "Pricing service initial load failed and pricing.allow_degraded=false; \
                     failing startup. Set pricing.allow_degraded=true to keep running \
                     without pricing data. Error: {}",
                    e
                );
                return Err(e);
            }
        } else {
            info!("Pricing service initial load completed");
        }
        info!("Pricing auto-refresh task is managed by on-demand refresh checks");

        let runtime_router_config =
            crate::core::router::gateway_config::runtime_router_config_from_gateway(
                &config.gateway.router,
            )
            .map_err(|e| GatewayError::Config(format!("Invalid router config: {}", e)))?;

        let unified_router = crate::core::router::UnifiedRouter::from_gateway_config(
            &config.gateway.providers,
            Some(runtime_router_config),
        )
        .await
        .map_err(|e| {
            GatewayError::Config(format!(
                "Failed to initialize unified router from config: {}",
                e
            ))
        })?;

        let state = AppState::new_with_unified_router(
            config.clone(),
            auth,
            unified_router,
            storage,
            pricing,
            budget_limits,
        );

        Ok(Self {
            config: config.gateway.server.clone(),
            state,
            budget_persistence_task,
        })
    }

    /// Create the Actix-web application
    fn create_app(
        state: web::Data<AppState>,
    ) -> App<
        impl actix_web::dev::ServiceFactory<
            actix_web::dev::ServiceRequest,
            Config = (),
            Response = actix_web::dev::ServiceResponse<impl actix_web::body::MessageBody>,
            Error = actix_web::Error,
            InitError = (),
        >,
    > {
        info!("Setting up routes and middleware");

        let cfg = state.config.load();
        if cfg.gateway.rate_limit.enabled && get_global_rate_limiter().is_none() {
            let redis_available = !state.storage.redis.is_noop();
            init_global_rate_limiter_with_redis(
                cfg.gateway.rate_limit.clone(),
                Arc::clone(&state.storage.redis),
            );
            info!(
                "Global rate limiter initialized (strategy={:?}, rpm={}, backend={})",
                cfg.gateway.rate_limit.strategy,
                cfg.gateway.rate_limit.default_rpm,
                if redis_available {
                    "redis"
                } else {
                    "in-process"
                }
            );
        }
        let cors_config = &cfg.gateway.server.cors;
        let rate_limit_enabled = cfg.gateway.rate_limit.enabled;
        let rate_limit_rpm = cfg.gateway.rate_limit.default_rpm;
        let api_key_auth_enabled = cfg.gateway.auth.enable_api_key;
        let default_rate_limit_rpm = rate_limit_enabled.then_some(rate_limit_rpm);
        let cors = Self::build_cors_for_app_factory(cors_config);

        let budget_limits = web::Data::new(Arc::clone(&state.budget_limits));

        App::new()
            .app_data(state)
            .app_data(budget_limits)
            .wrap(cors)
            .wrap(Logger::default())
            .wrap(DefaultHeaders::new().add(("Server", "LiteLLM-RS")))
            .wrap(SecurityHeadersMiddleware)
            // Actix executes wraps in reverse registration order. Register the
            // limiter before auth so successful auth context is available when
            // rate-limit keys are chosen.
            .wrap(Condition::new(
                rate_limit_enabled || api_key_auth_enabled,
                RateLimitMiddleware::optional(default_rate_limit_rpm),
            ))
            .wrap(AuthMiddleware)
            .wrap(RequestIdMiddleware)
            .configure(routes::health::configure_routes)
            .configure(routes::auth::configure_routes)
            .configure(routes::keys::configure_routes)
            .configure(routes::teams::configure_routes)
            .configure(routes::budget::configure_budget_routes)
            .configure(routes::ai::configure_routes)
            .configure(routes::pricing::configure_pricing_routes)
    }

    fn validate_cors_config(cors_config: &CorsConfig) -> Result<()> {
        cors_config
            .validate()
            .map_err(|e| GatewayError::Config(format!("Invalid CORS configuration: {}", e)))
    }

    fn build_cors(cors_config: &CorsConfig) -> Result<Cors> {
        Self::validate_cors_config(cors_config)?;

        let mut cors = Cors::default();
        if !cors_config.enabled {
            return Ok(cors);
        }

        if cors_config.allows_all_origins() {
            cors = cors.allow_any_origin();
        } else {
            for origin in &cors_config.allowed_origins {
                cors = cors.allowed_origin(origin);
            }
        }

        let methods: Vec<actix_web::http::Method> = cors_config
            .allowed_methods
            .iter()
            .filter_map(|m| m.parse().ok())
            .collect();
        if !methods.is_empty() {
            cors = cors.allowed_methods(methods);
        }

        let headers: Vec<actix_web::http::header::HeaderName> = cors_config
            .allowed_headers
            .iter()
            .filter_map(|h| h.parse().ok())
            .collect();
        if !headers.is_empty() {
            cors = cors.allowed_headers(headers);
        }

        cors = cors.max_age(cors_config.max_age as usize);

        if cors_config.allow_credentials {
            cors = cors.supports_credentials();
        }

        Ok(cors)
    }

    fn build_cors_for_app_factory(cors_config: &CorsConfig) -> Cors {
        match Self::build_cors(cors_config) {
            Ok(cors) => cors,
            Err(e) => {
                tracing::error!(
                    "Invalid CORS configuration reached app factory; using restrictive fallback: {}",
                    e
                );
                Cors::default()
            }
        }
    }

    /// Start the HTTP server
    ///
    /// Listens for SIGINT / SIGTERM via [`Self::shutdown_signal`]. When the
    /// signal fires, the actix server is told to stop gracefully (in-flight
    /// requests get a chance to finish), then the storage layer is closed
    /// so connection pools and pending writes are released cleanly.
    pub async fn start(mut self) -> Result<()> {
        let bind_addr = format!("{}:{}", self.config.host, self.config.port);
        let port = self.config.port;
        let budget_persistence_task = self.budget_persistence_task.take();

        info!("Starting HTTP server on {}", bind_addr);

        let state = web::Data::new(self.state);
        let storage = Arc::clone(&state.storage);

        let server = ActixHttpServer::new(move || Self::create_app(state.clone()))
            .bind(&bind_addr)
            .map_err(|e| Self::format_bind_error(e, &bind_addr, port))?
            .run();

        info!("HTTP server listening on {}", bind_addr);

        let server_handle = server.handle();
        let mut server_task = tokio::spawn(server);

        let shutdown = Self::shutdown_signal();
        let mut stopped_by_signal = false;

        tokio::select! {
            result = &mut server_task => {
                match result {
                    Ok(Ok(())) => info!("HTTP server exited"),
                    Ok(Err(e)) => {
                        return Err(GatewayError::server(format!("Server error: {}", e)));
                    }
                    Err(e) => {
                        return Err(GatewayError::server(format!("Server task failed: {}", e)));
                    }
                }
            }
            _ = shutdown => {
                info!("Shutdown signal received; stopping accept loop");
                server_handle.stop(true).await;
                stopped_by_signal = true;
            }
        }

        if stopped_by_signal {
            match server_task.await {
                Ok(Ok(())) => info!("HTTP server exited after graceful stop"),
                Ok(Err(e)) => {
                    return Err(GatewayError::server(format!("Server error: {}", e)));
                }
                Err(e) => {
                    return Err(GatewayError::server(format!("Server task failed: {}", e)));
                }
            }
        }

        // The server task owns the app factory, which owns AppState clones that
        // hold budget persistence senders. It has completed here, so the channel
        // can close after all queued events are drained by the worker.
        drop(server_handle);

        if let Some(task) = budget_persistence_task {
            info!("Waiting for budget persistence worker to drain");
            if let Err(e) = task.await {
                warn!("Budget persistence worker join failed: {}", e);
            }
        }

        info!("Closing storage layer");
        if let Err(e) = storage.close().await {
            warn!("Storage close reported an error: {}", e);
        }

        info!("HTTP server stopped");
        Ok(())
    }

    /// Get server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Get application state
    pub fn state(&self) -> &AppState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn build_cors_rejects_wildcard_with_credentials() {
        let cors_config = CorsConfig {
            allowed_origins: vec!["*".to_string()],
            allow_credentials: true,
            ..Default::default()
        };

        let error = match HttpServer::build_cors(&cors_config) {
            Ok(_) => panic!("invalid CORS configuration should be rejected"),
            Err(error) => error,
        };

        match error {
            GatewayError::Config(message) => {
                assert!(message.contains("Invalid CORS configuration"));
                assert!(message.contains("credentials"));
            }
            other => panic!("expected config error, got: {other:?}"),
        }
    }

    #[test]
    fn app_factory_cors_builder_falls_back_without_panicking() {
        let cors_config = CorsConfig {
            allowed_origins: vec!["*".to_string()],
            allow_credentials: true,
            ..Default::default()
        };

        let _cors = HttpServer::build_cors_for_app_factory(&cors_config);
    }

    #[tokio::test]
    async fn new_rejects_invalid_cors_config_before_startup() {
        let mut config = Config::default();
        config.gateway.server.cors.allowed_origins = vec!["*".to_string()];
        config.gateway.server.cors.allow_credentials = true;

        let error = match HttpServer::new(&config).await {
            Ok(_) => panic!("server startup should reject invalid CORS configuration"),
            Err(error) => error,
        };

        match error {
            GatewayError::Config(message) => {
                assert!(message.contains("Invalid CORS configuration"));
                assert!(message.contains("credentials"));
            }
            other => panic!("expected config error, got: {other:?}"),
        }
    }

    /// Build a config whose only optional dependency that is *configured* is
    /// the pricing source, which points at a non-existent file so the initial
    /// load fails deterministically.
    fn config_with_broken_pricing(allow_degraded: bool) -> Config {
        let mut config = Config::default();
        // Disable enterprise/storage subsystems that would require real I/O.
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source =
            Some("/nonexistent/path/that/cannot/be/loaded.json".to_string());
        config.gateway.pricing.allow_degraded = allow_degraded;
        config
    }

    #[tokio::test]
    async fn new_fails_when_pricing_source_broken_and_not_allowed_to_degrade() {
        let config = config_with_broken_pricing(false);
        let result = HttpServer::new(&config).await;
        assert!(
            result.is_err(),
            "pricing source load failure with allow_degraded=false must fail startup"
        );
    }

    #[tokio::test]
    async fn new_succeeds_when_pricing_source_broken_but_allowed_to_degrade() {
        let config = config_with_broken_pricing(true);
        let result = HttpServer::new(&config).await;
        assert!(
            result.is_ok(),
            "pricing source load failure with allow_degraded=true must keep startup running, \
             got: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn new_rejects_unwired_cache_config() {
        let mut config = Config::default();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;
        config.gateway.cache.enabled = true;

        let error = match HttpServer::new(&config).await {
            Ok(_) => panic!("expected unwired cache configuration to fail startup"),
            Err(error) => error,
        };

        match error {
            GatewayError::Config(message) => {
                assert!(message.contains("Invalid cache configuration"));
                assert!(message.contains("not wired into runtime"));
            }
            other => panic!("expected config error, got: {other:?}"),
        }
    }

    /// In-memory budget snapshots load succeeds (returns empty) on sqlite, so
    /// we can't trigger a real "load failed" path from `Config::default()`
    /// alone without a mock. The disabled-DB branch is covered here: when the
    /// database is disabled we use the in-memory sqlite backend which always
    /// returns an empty snapshot set, exercising the "Ok(snapshots)" arm.
    #[tokio::test]
    async fn new_succeeds_with_in_memory_budgets_when_db_disabled() {
        let mut config = Config::default();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        // Disable pricing so we don't conflate with the broken-pricing tests.
        config.gateway.pricing.source = None;

        let result = HttpServer::new(&config).await;
        assert!(
            result.is_ok(),
            "disabled DB must keep startup running with in-memory budgets, got: {:?}",
            result.err()
        );
    }
}
