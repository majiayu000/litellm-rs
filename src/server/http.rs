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
use crate::core::rate_limiter::{get_global_rate_limiter, init_global_rate_limiter_with_redis};
use crate::server::http_listener::{build_actix_server, validated_listener_settings};
use crate::server::middleware::{
    AuthMiddleware, MetricsMiddleware, RateLimitMiddleware, RequestIdMiddleware,
    SecurityHeadersMiddleware, start_auth_rate_limiter_cleanup_task,
};
use crate::server::routes;
use crate::server::state::AppState;
use crate::utils::error::gateway_error::{GatewayError, Result};
use actix_cors::Cors;
use actix_web::{
    App,
    body::MessageBody,
    dev::{ServiceRequest, ServiceResponse},
    http::{Method, header},
    middleware::{Condition, DefaultHeaders, Logger, Next, from_fn},
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
    tls: Option<crate::server::tls::ListenerTls>,
    /// Background worker that drains budget persistence events on shutdown.
    budget_persistence_task: Option<JoinHandle<()>>,
    /// Background worker that delivers configured callback events.
    callback_runtime: CallbackRuntime,
}

impl HttpServer {
    /// Create a new HTTP server
    pub async fn new(config: &Config) -> Result<Self> {
        validated_listener_settings(&config.gateway.server)?;
        config.gateway.storage.redis.validate().map_err(|error| {
            GatewayError::Config(format!("Invalid Redis configuration: {error}"))
        })?;
        config.validate()?;

        let tls = crate::server::tls::load_listener_tls(&config.gateway.server)?;
        info!("Creating HTTP server");
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

        let (pricing, unified_router) =
            super::http_runtime::build_pricing_and_router(config).await?;

        let callback_runtime = crate::server::callbacks::build_callback_runtime(
            &config.gateway.monitoring.callbacks,
            &config.gateway.monitoring.metrics,
        )
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
            tls,
            budget_persistence_task,
            callback_runtime,
        })
    }

    /// Create the Actix-web application
    pub(super) fn create_app(
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
        let listener_settings = validated_listener_settings(&self.config)?;
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
        let listener = match self.tls.take() {
            Some(tls) => crate::server::tls::build_tls_server(
                state,
                &listener_settings,
                bind_addr.as_str(),
                tls,
            ),
            None => build_actix_server(state, &listener_settings, bind_addr.as_str()),
        };
        let (server, selected_address) =
            listener.map_err(|e| Self::format_bind_error(e, &bind_addr, port))?;

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
#[path = "http_metrics_tests.rs"]
mod metrics_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use actix_web::{
        http::{StatusCode, header},
        test as actix_test,
    };

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
        let mut config = valid_http_test_config();
        config.gateway.server.cors.allowed_origins = vec!["*".to_string()];
        config.gateway.server.cors.allow_credentials = true;

        let error = match HttpServer::new(&config).await {
            Ok(_) => panic!("server startup should reject invalid CORS configuration"),
            Err(error) => error,
        };

        match error {
            GatewayError::Config(message) => {
                assert!(message.contains("Gateway config error"));
                assert!(message.contains("CORS"));
                assert!(message.contains("credentials"));
            }
            other => panic!("expected config error, got: {other:?}"),
        }
    }

    /// Build a config whose only optional dependency that is *configured* is
    /// the pricing source, which points at a non-existent file so the initial
    /// load fails deterministically.
    fn config_with_broken_pricing(allow_degraded: bool) -> Config {
        let mut config = valid_http_test_config();
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
    async fn new_wires_enabled_cache_config() {
        let mut config = valid_http_test_config();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;
        config.gateway.cache.enabled = true;

        let server = match HttpServer::new(&config).await {
            Ok(server) => server,
            Err(error) => panic!("enabled cache should wire runtime cache: {error}"),
        };

        assert!(server.state().response_cache.is_some());
    }

    #[tokio::test]
    async fn new_wires_configured_callback_backend() {
        let mut config = valid_http_test_config();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;
        config.gateway.monitoring.metrics.enabled = false;
        config.gateway.monitoring.callbacks.backends = vec![
            crate::config::models::monitoring::CallbackBackendConfig::OpenTelemetry(
                crate::core::integrations::OpenTelemetryConfig::default(),
            ),
        ];

        let server = match HttpServer::new(&config).await {
            Ok(server) => server,
            Err(error) => panic!("configured callbacks should wire at startup: {error}"),
        };

        assert!(server.state().callbacks.is_enabled());
        assert_eq!(
            server.state().callbacks.registered_integrations().await,
            vec!["opentelemetry"]
        );
    }

    /// In-memory budget snapshots load succeeds (returns empty) on sqlite, so
    /// we can't trigger a real "load failed" path from `Config::default()`
    /// alone without a mock. The disabled-DB branch is covered here: when the
    /// database is disabled we use the in-memory sqlite backend which always
    /// returns an empty snapshot set, exercising the "Ok(snapshots)" arm.
    #[tokio::test]
    async fn new_succeeds_with_in_memory_budgets_when_db_disabled() {
        let mut config = valid_http_test_config();
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

    #[tokio::test]
    async fn app_factory_metrics_endpoint_includes_recorded_http_requests() {
        let _metrics_guard = MetricsMiddleware::test_lock().await;
        MetricsMiddleware::reset_for_tests();
        crate::server::middleware::reset_unpriced_metrics_for_tests();
        crate::server::middleware::record_unpriced_event(
            "metrics-http-provider",
            "tenant-http-private-model",
            "reject",
            "reject_preflight",
        );

        let mut config = valid_http_test_config();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;

        let server = match HttpServer::new(&config).await {
            Ok(server) => server,
            Err(error) => panic!("server startup failed: {error}"),
        };

        let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
            server.state().clone(),
        )))
        .await;

        let health_req = actix_test::TestRequest::get().uri("/health").to_request();
        let health_resp = actix_test::call_service(&app, health_req).await;
        assert_eq!(health_resp.status(), StatusCode::OK);
        drop(actix_test::read_body(health_resp).await);

        let metrics_req = actix_test::TestRequest::get().uri("/metrics").to_request();
        let metrics_resp = actix_test::call_service(&app, metrics_req).await;
        assert_eq!(metrics_resp.status(), StatusCode::OK);

        let body = actix_test::read_body(metrics_resp).await;
        let body = match std::str::from_utf8(&body) {
            Ok(body) => body,
            Err(error) => panic!("metrics response was not utf-8: {error}"),
        };

        assert!(body.contains("gateway_http_requests_total 1"));
        assert!(body.contains("gateway_http_responses_total{class=\"2xx\"} 1"));
        assert!(body.contains(
            "gateway_unpriced_events_total{provider=\"metrics-http-provider\",model_bucket=\"other\",policy=\"reject\",outcome=\"reject_preflight\"} 1"
        ));
        assert!(!body.contains("tenant-http-private-model"));

        let rendered_after_scrape = MetricsMiddleware::render_prometheus();
        assert!(rendered_after_scrape.contains("gateway_http_requests_total 1"));
    }

    #[tokio::test]
    async fn app_factory_metrics_records_auth_rejections_before_handler() {
        let _metrics_guard = MetricsMiddleware::test_lock().await;
        MetricsMiddleware::reset_for_tests();

        let mut config = valid_http_test_config();
        config.gateway.auth.enable_jwt = true;
        config.gateway.auth.enable_api_key = true;
        config.gateway.auth.allow_anonymous = false;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
        config.gateway.rate_limit.enabled = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;

        let server = match HttpServer::new(&config).await {
            Ok(server) => server,
            Err(error) => panic!("server startup failed: {error}"),
        };

        let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
            server.state().clone(),
        )))
        .await;

        let models_req = actix_test::TestRequest::get()
            .uri("/v1/models")
            .to_request();
        match actix_test::try_call_service(&app, models_req).await {
            Ok(response) => {
                assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
                drop(actix_test::read_body(response).await);
            }
            Err(error) => assert_eq!(
                error.as_response_error().status_code(),
                StatusCode::UNAUTHORIZED
            ),
        }

        let body = MetricsMiddleware::render_prometheus();
        assert!(body.contains("gateway_http_requests_total 1"));
        assert!(body.contains("gateway_http_request_errors_total 1"));
        assert!(body.contains("gateway_http_responses_total{class=\"4xx\"} 1"));
    }

    include!("http_cors_tests.rs");
    include!("http_validation_tests.rs");

    #[tokio::test]
    async fn app_factory_does_not_collect_http_metrics_when_metrics_disabled() {
        let _metrics_guard = MetricsMiddleware::test_lock().await;
        MetricsMiddleware::reset_for_tests();

        let mut config = valid_http_test_config();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.monitoring.metrics.enabled = false;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;

        let server = match HttpServer::new(&config).await {
            Ok(server) => server,
            Err(error) => panic!("server startup failed: {error}"),
        };

        let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
            server.state().clone(),
        )))
        .await;

        let health_req = actix_test::TestRequest::get().uri("/health").to_request();
        let health_resp = actix_test::call_service(&app, health_req).await;
        assert_eq!(health_resp.status(), StatusCode::OK);
        drop(actix_test::read_body(health_resp).await);

        let body = MetricsMiddleware::render_prometheus();
        assert!(body.contains("gateway_http_requests_total 0"));
        assert!(body.contains("gateway_http_responses_total{class=\"2xx\"} 0"));
    }

    #[tokio::test]
    async fn app_factory_mounts_explicit_cache_admin_surface() {
        let mut config = valid_http_test_config();
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.monitoring.metrics.enabled = false;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;

        let server = match HttpServer::new(&config).await {
            Ok(server) => server,
            Err(error) => panic!("server startup failed: {error}"),
        };

        let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
            server.state().clone(),
        )))
        .await;

        let req = actix_test::TestRequest::get()
            .uri("/admin/cache/status")
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);

        let body: serde_json::Value = actix_test::read_body_json(resp).await;
        assert_eq!(body["status"], "unsupported");
        assert!(
            body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("not wired")
        );
    }
}
