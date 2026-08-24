//! HTTP server core implementation
//!
//! This module provides the HttpServer struct and its core methods.

use crate::config::models::server::{CorsConfig, ServerConfig};
use crate::config::{Config, Validate};
use crate::core::audit::{AuditConfig, AuditLogger, AuditMiddleware};
use crate::core::budget::UnifiedBudgetLimits;
use crate::core::guardrails::GuardrailEngine;
use crate::core::integrations::CallbackRuntime;
use crate::core::ip_access::{IpAccessControl, IpAccessMiddleware};
use crate::core::pricing_service::PricingService;
use crate::core::rate_limiter::{get_global_rate_limiter, init_global_rate_limiter_with_redis};
use crate::server::middleware::{
    AuthMiddleware, MetricsMiddleware, RateLimitMiddleware, RequestIdMiddleware,
    SecurityHeadersMiddleware, start_auth_rate_limiter_cleanup_task,
};
use crate::server::routes;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::{GatewayError, Result};
use actix_cors::Cors;
use actix_web::{
    App, HttpServer as ActixHttpServer,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    http::{Method, header},
    middleware::{Condition, DefaultHeaders, Logger, Next, from_fn},
    web,
};
use std::net::ToSocketAddrs;
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
    /// Background worker that delivers configured callback events.
    callback_runtime: CallbackRuntime,
}

#[derive(Debug, PartialEq, Eq)]
struct ListenerSettings {
    configured_workers: usize,
    effective_workers: usize,
    first_request_head_timeout: std::time::Duration,
    max_connections_per_worker: Option<usize>,
}

impl HttpServer {
    fn validated_listener_settings(config: &ServerConfig) -> Result<ListenerSettings> {
        Validate::validate(config).map_err(|error| {
            GatewayError::Config(format!("Invalid server configuration: {error}"))
        })?;

        let configured_workers = config.worker_count();
        let first_request_head_timeout = std::time::Duration::from_secs(config.timeout);
        let Some(total_connections) = config.max_connections else {
            return Ok(ListenerSettings {
                configured_workers,
                effective_workers: configured_workers,
                first_request_head_timeout,
                max_connections_per_worker: None,
            });
        };

        // Actix's max_connections setting is per worker, and actix-server
        // 2.6 cannot re-enable a worker after a limit of 1 is released. Keep
        // each worker at 2 or more connections and round down so the effective
        // server-wide capacity never exceeds the configured total.
        let workers = configured_workers.min(total_connections / 2).max(1);
        Ok(ListenerSettings {
            configured_workers,
            effective_workers: workers,
            first_request_head_timeout,
            max_connections_per_worker: Some(total_connections / workers),
        })
    }

    fn build_actix_server(
        state: web::Data<AppState>,
        settings: &ListenerSettings,
        addresses: impl ToSocketAddrs,
    ) -> std::io::Result<(actix_web::dev::Server, std::net::SocketAddr)> {
        let mut last_error = None;
        for address in addresses.to_socket_addrs()? {
            let app_state = state.clone();
            let mut builder = ActixHttpServer::new(move || Self::create_app(app_state.clone()))
                .workers(settings.effective_workers)
                .client_request_timeout(settings.first_request_head_timeout);
            if let Some(per_worker) = settings.max_connections_per_worker {
                builder = builder.max_connections(per_worker);
            }
            match builder.bind(address) {
                Ok(builder) => {
                    let bound_addresses = builder.addrs();
                    let [selected_address] = bound_addresses.as_slice() else {
                        return Err(std::io::Error::other(
                            "Actix must bind exactly one resolved address",
                        ));
                    };
                    return Ok((builder.run(), *selected_address));
                }
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| std::io::Error::other("Could not bind to address")))
    }

    /// Create a new HTTP server
    pub async fn new(config: &Config) -> Result<Self> {
        Self::validated_listener_settings(&config.gateway.server)?;
        info!("Creating HTTP server");

        crate::config::models::gateway::GatewayConfig::validate_model_alias_map(
            &config.gateway.model_aliases,
        )
        .map_err(|error| GatewayError::Config(format!("Invalid model aliases: {error}")))?;
        Self::validate_cors_config(&config.gateway.server.cors)?;
        config
            .gateway
            .cache
            .validate()
            .map_err(|e| GatewayError::Config(format!("Invalid cache configuration: {}", e)))?;
        config
            .gateway
            .monitoring
            .callbacks
            .validate()
            .map_err(|e| GatewayError::Config(format!("Invalid callback configuration: {}", e)))?;
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

        let unified_router = crate::core::router::UnifiedRouter::from_gateway_config_with_aliases(
            &config.gateway.providers,
            Some(runtime_router_config),
            &config.gateway.model_aliases,
        )
        .await
        .map_err(|e| {
            GatewayError::Config(format!(
                "Failed to initialize unified router from config: {}",
                e
            ))
        })?;

        let callback_runtime =
            crate::server::callbacks::build_callback_runtime(&config.gateway.monitoring.callbacks)
                .await;
        let audit_logger = if config.gateway.enterprise.audit_logging {
            AuditLogger::shared(AuditConfig::default().enable())
                .await
                .map_err(|error| {
                    GatewayError::Config(format!("Failed to initialize audit logging: {error}"))
                })?
        } else {
            Arc::new(AuditLogger::disabled())
        };
        let guardrails =
            GuardrailEngine::shared(config.gateway.guardrails.clone()).map_err(|error| {
                GatewayError::Config(format!("Invalid guardrails configuration: {error}"))
            })?;
        let ip_access =
            IpAccessControl::shared(config.gateway.ip_access.clone()).map_err(|error| {
                GatewayError::Config(format!("Invalid IP access configuration: {error}"))
            })?;
        let state = AppState::new_with_unified_router(
            config.clone(),
            auth,
            unified_router,
            storage,
            pricing,
            budget_limits,
        )
        .with_callbacks(callback_runtime.dispatcher())
        .with_audit_logger(audit_logger)
        .with_request_policies(guardrails, ip_access);

        Ok(Self {
            config: config.gateway.server.clone(),
            state,
            budget_persistence_task,
            callback_runtime,
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
                cfg.gateway.rate_limit.effective_rpm(),
                if redis_available {
                    "redis"
                } else {
                    "in-process"
                }
            );
        }
        let cors_config = &cfg.gateway.server.cors;
        let rate_limit_enabled = cfg.gateway.rate_limit.enabled;
        let rate_limit_rpm = cfg.gateway.rate_limit.effective_rpm();
        let api_key_auth_enabled = cfg.gateway.auth.enable_api_key;
        let default_rate_limit_rpm = rate_limit_enabled.then_some(rate_limit_rpm);
        let metrics_enabled = cfg.gateway.monitoring.metrics.enabled;
        let audit_enabled = state.audit_logger.is_enabled();
        let audit_logger = Arc::clone(&state.audit_logger);
        let trusted_proxies = cfg.gateway.server.trusted_proxies.clone();
        let ip_access = Arc::clone(&state.ip_access);
        let cors = Self::build_cors_for_app_factory(cors_config);
        let max_body_size = cfg.gateway.server.max_body_size;

        let budget_limits = web::Data::new(Arc::clone(&state.budget_limits));

        App::new()
            .app_data(state)
            .app_data(budget_limits)
            // server.max_body_size bounds JSON and form bodies; file/audio
            // uploads enforce their own larger multipart limits instead.
            .app_data(web::JsonConfig::default().limit(max_body_size))
            .app_data(web::FormConfig::default().limit(max_body_size))
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
            .wrap(Condition::new(metrics_enabled, MetricsMiddleware))
            // CORS must run outside auth/rate-limit, but only standard browser
            // preflight may be short-circuited before those layers.
            .wrap(Condition::new(cors_config.enabled, cors))
            .wrap(from_fn(normalize_non_cors_options_before_cors))
            // IP policy remains before authentication/provider side effects.
            .wrap(IpAccessMiddleware::new(ip_access))
            // Audit wraps IP denials; request IDs wrap the complete lifecycle.
            .wrap(Condition::new(
                audit_enabled,
                AuditMiddleware::with_trusted_proxies(audit_logger, trusted_proxies),
            ))
            .wrap(RequestIdMiddleware)
            .configure(routes::health::configure_routes)
            .configure(routes::auth::configure_routes)
            .configure(routes::keys::configure_routes)
            .configure(routes::teams::configure_routes)
            .configure(routes::budget::configure_budget_routes)
            .configure(routes::admin::configure_routes)
            .configure(routes::admin_dashboard::configure_routes)
            .configure(|cfg| routes::ai::configure_routes_with_body_limit(cfg, max_body_size))
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
    /// Gracefully stops requests, drains workers, then closes storage.
    pub async fn start(mut self) -> Result<()> {
        let listener_settings = Self::validated_listener_settings(&self.config)?;
        let bind_addr = format!("{}:{}", self.config.host, self.config.port);
        let port = self.config.port;
        let budget_persistence_task = self.budget_persistence_task.take();
        let callback_runtime =
            std::mem::replace(&mut self.callback_runtime, CallbackRuntime::disabled());

        // server.timeout bounds only how long a client may take to deliver
        // the first request head (408 afterwards); it never bounds handler or
        // streaming duration — those follow the outbound-client policy.
        info!(
            "Starting HTTP server on {} (workers={}, first_request_head_timeout={}s)",
            bind_addr, listener_settings.effective_workers, self.config.timeout
        );
        if let Some(configured_total) = self.config.max_connections {
            if listener_settings.effective_workers != listener_settings.configured_workers {
                info!(
                    "Reducing HTTP workers from {} to {} so server.max_connections={} can use Actix's minimum safe per-worker limit of 2",
                    listener_settings.configured_workers,
                    listener_settings.effective_workers,
                    configured_total
                );
            }
            if let Some(per_worker) = listener_settings.max_connections_per_worker {
                info!(
                    "Connection limit: {} per worker, {} server-wide effective (configured total={})",
                    per_worker,
                    per_worker * listener_settings.effective_workers,
                    configured_total
                );
            }
        }

        let state = web::Data::new(self.state);
        let storage = Arc::clone(&state.storage);
        let audit_logger = Arc::clone(&state.audit_logger);

        // Try resolved addresses one at a time. Passing the hostname directly
        // to Actix would create one full worker set per successful address,
        // multiplying the configured server-wide connection cap.
        let (server, selected_address) =
            Self::build_actix_server(state, &listener_settings, bind_addr.as_str())
                .map_err(|e| Self::format_bind_error(e, &bind_addr, port))?;

        info!(
            "HTTP server listening on {} (selected from {})",
            selected_address, bind_addr
        );

        let server_handle = server.handle();
        let mut server_task = tokio::spawn(server);

        let shutdown = Self::shutdown_signal();
        let mut stopped_by_signal = false;

        let mut server_result = tokio::select! {
            result = &mut server_task => {
                match result {
                    Ok(Ok(())) => { info!("HTTP server exited"); Ok(()) }
                    Ok(Err(e)) => Err(GatewayError::server(format!("Server error: {}", e))),
                    Err(e) => Err(GatewayError::server(format!("Server task failed: {}", e))),
                }
            }
            _ = shutdown => {
                info!("Shutdown signal received; stopping accept loop");
                server_handle.stop(true).await;
                stopped_by_signal = true;
                Ok(())
            }
        };

        if stopped_by_signal {
            server_result = match server_task.await {
                Ok(Ok(())) => {
                    info!("HTTP server exited after graceful stop");
                    Ok(())
                }
                Ok(Err(e)) => Err(GatewayError::server(format!("Server error: {}", e))),
                Err(e) => Err(GatewayError::server(format!("Server task failed: {}", e))),
            };
        }

        // AppState clones are gone, so worker queues can now drain.
        drop(server_handle);

        if let Some(task) = budget_persistence_task {
            info!("Waiting for budget persistence worker to drain");
            if let Err(e) = task.await {
                warn!("Budget persistence worker join failed: {}", e);
            }
        }

        info!("Draining external callback worker");
        if let Err(e) = callback_runtime.shutdown().await {
            warn!("Callback worker shutdown reported an error: {}", e);
        }

        info!("Draining audit worker");
        let audit_shutdown = audit_logger.shutdown().await;

        info!("Closing storage layer");
        if let Err(e) = storage.close().await {
            warn!("Storage close reported an error: {}", e);
        }

        server_result?;
        audit_shutdown.map_err(|e| GatewayError::server(format!("Audit shutdown failed: {e}")))?;

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

async fn normalize_non_cors_options_before_cors(
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> std::result::Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    if req.method() == Method::OPTIONS
        && req
            .headers()
            .contains_key(header::ACCESS_CONTROL_REQUEST_METHOD)
        && !req.headers().contains_key(header::ORIGIN)
    {
        req.headers_mut()
            .remove(header::ACCESS_CONTROL_REQUEST_METHOD);
    }

    next.call(req).await
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
