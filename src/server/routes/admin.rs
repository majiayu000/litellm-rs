//! Admin-only operational route surfaces.

use crate::core::models::{
    ApiKey,
    user::types::{User, UserRole},
};
use crate::server::state::AppState;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, error::ErrorInternalServerError, web};
use serde::Serialize;
use tracing::warn;

const CACHE_UNWIRED_MESSAGE: &str = "Response cache is not wired into runtime request handling";
const CACHE_WIRED_MESSAGE: &str = "Response cache is wired into runtime request handling";

#[derive(Debug, Serialize)]
struct CacheAdminResponse {
    success: bool,
    status: &'static str,
    cache_enabled: bool,
    semantic_cache_enabled: bool,
    message: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    stats: Option<crate::core::cache::CombinedCacheStats>,
    #[serde(skip_serializing_if = "Option::is_none")]
    redis_available: Option<bool>,
}

async fn cache_admin_response(state: &web::Data<AppState>) -> CacheAdminResponse {
    let cfg = state.config.load();
    if let Some(cache) = state.response_cache.as_ref() {
        return CacheAdminResponse {
            success: true,
            status: "enabled",
            cache_enabled: cfg.gateway.cache.enabled,
            semantic_cache_enabled: cfg.gateway.cache.semantic_cache,
            message: CACHE_WIRED_MESSAGE,
            stats: Some(cache.combined_stats()),
            redis_available: Some(cache.is_redis_available().await),
        };
    }

    CacheAdminResponse {
        success: false,
        status: "unsupported",
        cache_enabled: cfg.gateway.cache.enabled,
        semantic_cache_enabled: cfg.gateway.cache.semantic_cache,
        message: CACHE_UNWIRED_MESSAGE,
        stats: None,
        redis_available: None,
    }
}

fn require_cache_admin(
    req: &HttpRequest,
    state: &web::Data<AppState>,
    action: &str,
) -> Option<HttpResponse> {
    let cfg = state.config.load();
    if !cfg.auth().enable_jwt && !cfg.auth().enable_api_key {
        return None;
    }

    let extensions = req.extensions();
    if let Some(user) = extensions.get::<User>() {
        if user.has_role(&UserRole::Admin) {
            return None;
        }
        warn!(
            "User '{}' attempted to {} without admin role",
            user.username, action
        );
    } else if extensions.get::<ApiKey>().is_some() {
        warn!("API key attempted to {} without admin user context", action);
    } else {
        warn!("Unidentified caller attempted to {}", action);
    }

    Some(HttpResponse::Forbidden().json(serde_json::json!({
        "success": false,
        "error": "Admin role required for cache administration"
    })))
}

/// GET /admin/cache and /admin/cache/status
pub async fn cache_status(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_cache_admin(&req, &state, "inspect cache status") {
        return Ok(forbidden);
    }

    let response = cache_admin_response(&state).await;
    if response.success {
        Ok(HttpResponse::Ok().json(response))
    } else {
        Ok(HttpResponse::NotImplemented().json(response))
    }
}

/// POST /admin/cache/clear
pub async fn clear_response_cache(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> actix_web::Result<HttpResponse> {
    if let Some(forbidden) = require_cache_admin(&req, &state, "clear cache") {
        return Ok(forbidden);
    }

    if let Some(cache) = state.response_cache.as_ref() {
        cache
            .clear()
            .await
            .map_err(|e| ErrorInternalServerError(format!("Failed to clear cache: {e}")))?;
        return Ok(HttpResponse::Ok().json(cache_admin_response(&state).await));
    }

    Ok(HttpResponse::NotImplemented().json(cache_admin_response(&state).await))
}

/// Configure admin routes.
pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/admin/cache")
            .route("", web::get().to(cache_status))
            .route("/", web::get().to(cache_status))
            .route("/status", web::get().to(cache_status))
            .route("/clear", web::post().to(clear_response_cache)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::core::models::{
        Metadata, UsageStats,
        user::{
            preferences::UserPreferences,
            types::{UserProfile, UserStatus},
        },
    };
    use crate::server::HttpServer;
    use actix_web::dev::Service;
    use actix_web::{App, http::StatusCode, test};

    fn base_test_config(auth_enabled: bool) -> Config {
        let mut config = crate::server::valid_test_config();
        config.gateway.auth.enable_jwt = auth_enabled;
        config.gateway.auth.enable_api_key = auth_enabled;
        config.gateway.auth.allow_anonymous = !auth_enabled;
        config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;
        config
    }

    async fn test_state(config: Config) -> web::Data<AppState> {
        let server = match HttpServer::new(&config).await {
            Ok(server) => server,
            Err(error) => panic!("server startup failed: {error}"),
        };

        web::Data::new(server.state().clone())
    }

    fn make_test_user(role: UserRole) -> User {
        User {
            metadata: Metadata::new(),
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            display_name: None,
            password_hash: "hash".to_string(),
            role,
            status: UserStatus::Active,
            team_ids: vec![],
            preferences: UserPreferences::default(),
            usage_stats: UsageStats::default(),
            rate_limits: None,
            last_login_at: None,
            email_verified: true,
            two_factor_enabled: false,
            profile: UserProfile::default(),
        }
    }

    #[actix_web::test]
    async fn cache_status_requires_admin_identity() {
        let state = test_state(base_test_config(true)).await;
        let app = test::init_service(App::new().app_data(state).configure(configure_routes)).await;

        let req = test::TestRequest::get().uri("/admin/cache").to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn cache_status_rejects_non_admin_user() {
        let user = make_test_user(UserRole::User);
        let state = test_state(base_test_config(true)).await;
        let app = test::init_service(
            App::new()
                .app_data(state)
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<User>(user.clone());
                    srv.call(req)
                })
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::get().uri("/admin/cache").to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::FORBIDDEN);
    }

    #[actix_web::test]
    async fn cache_status_reports_explicit_unsupported_for_admin() {
        let admin = make_test_user(UserRole::Admin);
        let state = test_state(base_test_config(true)).await;
        let app = test::init_service(
            App::new()
                .app_data(state)
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<User>(admin.clone());
                    srv.call(req)
                })
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/admin/cache/status")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["success"], false);
        assert_eq!(body["status"], "unsupported");
        assert_eq!(body["cache_enabled"], false);
        assert_eq!(body["semantic_cache_enabled"], false);
        assert!(
            body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("not wired")
        );
    }

    #[actix_web::test]
    async fn cache_status_reports_enabled_cache_for_admin() {
        let admin = make_test_user(UserRole::Admin);
        let mut config = base_test_config(true);
        config.gateway.cache.enabled = true;
        let state = test_state(config).await;
        let app = test::init_service(
            App::new()
                .app_data(state)
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<User>(admin.clone());
                    srv.call(req)
                })
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::get()
            .uri("/admin/cache/status")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["status"], "enabled");
        assert_eq!(body["cache_enabled"], true);
        assert!(body["stats"].is_object());
    }

    #[actix_web::test]
    async fn clear_cache_reports_explicit_unsupported_for_admin() {
        let admin = make_test_user(UserRole::Admin);
        let state = test_state(base_test_config(true)).await;
        let app = test::init_service(
            App::new()
                .app_data(state)
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<User>(admin.clone());
                    srv.call(req)
                })
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/admin/cache/clear")
            .to_request();
        let resp = test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED);
    }

    #[actix_web::test]
    async fn clear_cache_succeeds_when_cache_is_enabled() {
        let admin = make_test_user(UserRole::Admin);
        let mut config = base_test_config(true);
        config.gateway.cache.enabled = true;
        let state = test_state(config).await;
        let app = test::init_service(
            App::new()
                .app_data(state)
                .wrap_fn(move |req, srv| {
                    req.extensions_mut().insert::<User>(admin.clone());
                    srv.call(req)
                })
                .configure(configure_routes),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/admin/cache/clear")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["success"], true);
        assert_eq!(body["status"], "enabled");
    }
}
