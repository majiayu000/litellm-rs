//! HTTP server core implementation
//!
//! This module provides the HttpServer struct and its core methods.

use crate::config::Config;
use crate::config::models::server::ServerConfig;
use crate::core::rate_limiter::{get_global_rate_limiter, init_global_rate_limiter};
use crate::server::middleware::{
    AuthMiddleware, RateLimitMiddleware, RequestIdMiddleware, SecurityHeadersMiddleware,
};
use crate::server::routes;
use crate::server::state::AppState;
use crate::services::pricing::PricingService;
use crate::utils::error::gateway_error::{GatewayError, Result};
use actix_cors::Cors;
use actix_web::{
    App, HttpServer as ActixHttpServer,
    middleware::{Condition, DefaultHeaders, Logger},
    web,
};
use std::sync::Arc;
use tracing::{info, warn};

/// HTTP server
pub struct HttpServer {
    /// Server configuration
    config: ServerConfig,
    /// Application state
    state: AppState,
}

impl HttpServer {
    /// Create a new HTTP server
    pub async fn new(config: &Config) -> Result<Self> {
        info!("Creating HTTP server");

        let storage = crate::storage::StorageLayer::new(&config.gateway.storage).await?;
        let auth =
            crate::auth::AuthSystem::new(&config.gateway.auth, Arc::new(storage.clone())).await?;

        let pricing_source = if config.gateway.cache.semantic_cache {
            None
        } else {
            config.gateway.pricing.source.clone()
        };
        let pricing = Arc::new(PricingService::new(pricing_source));
        if let Err(e) = pricing.initialize().await {
            warn!("Pricing service initial load failed: {}", e);
        } else {
            info!("Pricing service initial load completed");
        }
        info!("Pricing auto-refresh task is managed by on-demand refresh checks");

        let runtime_router_config =
            crate::core::router::gateway_config::runtime_router_config_from_gateway(
                &config.gateway.router,
            );

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
        );

        Ok(Self {
            config: config.gateway.server.clone(),
            state,
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
            init_global_rate_limiter(cfg.gateway.rate_limit.clone());
            info!(
                "Global rate limiter initialized (strategy={:?}, rpm={})",
                cfg.gateway.rate_limit.strategy, cfg.gateway.rate_limit.default_rpm
            );
        }
        let cors_config = &cfg.gateway.server.cors;
        let rate_limit_enabled = cfg.gateway.rate_limit.enabled;
        let rate_limit_rpm = cfg.gateway.rate_limit.default_rpm;
        let mut cors = Cors::default();

        if cors_config.enabled {
            if cors_config.allows_all_origins() {
                cors = cors.allow_any_origin();
                cors_config.validate().unwrap_or_else(|e| {
                    warn!(error = %e, "CORS Configuration Warning");
                });
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
        }

        let budget_limits = web::Data::new(Arc::clone(&state.budget_limits));

        App::new()
            .app_data(state)
            .app_data(budget_limits)
            .wrap(cors)
            .wrap(Logger::default())
            .wrap(DefaultHeaders::new().add(("Server", "LiteLLM-RS")))
            .wrap(SecurityHeadersMiddleware)
            .wrap(AuthMiddleware)
            .wrap(Condition::new(
                rate_limit_enabled,
                RateLimitMiddleware::new(rate_limit_rpm),
            ))
            .wrap(RequestIdMiddleware)
            .configure(routes::health::configure_routes)
            .configure(routes::auth::configure_routes)
            .configure(routes::keys::configure_routes)
            .configure(routes::teams::configure_routes)
            .configure(routes::budget::configure_budget_routes)
            .configure(routes::ai::configure_routes)
            .configure(routes::pricing::configure_pricing_routes)
    }

    /// Start the HTTP server
    pub async fn start(self) -> Result<()> {
        let bind_addr = format!("{}:{}", self.config.host, self.config.port);
        let port = self.config.port;

        info!("Starting HTTP server on {}", bind_addr);

        let state = web::Data::new(self.state);

        let server = ActixHttpServer::new(move || Self::create_app(state.clone()))
            .bind(&bind_addr)
            .map_err(|e| Self::format_bind_error(e, &bind_addr, port))?
            .run();

        info!("HTTP server listening on {}", bind_addr);

        server
            .await
            .map_err(|e| GatewayError::server(format!("Server error: {}", e)))?;

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
