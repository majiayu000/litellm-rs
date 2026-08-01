use actix_web::http::StatusCode;
use actix_web::{App, HttpMessage, HttpRequest, HttpResponse, test, web};
use litellm_rs::Config;
use litellm_rs::core::models::user::types::{User, UserRole, UserStatus};
use litellm_rs::core::models::{ApiKey, Metadata, RateLimits, UsageStats};
use litellm_rs::core::types::context::{RequestContext, SharedRequestContext};
use litellm_rs::server::http::HttpServer;
use litellm_rs::server::middleware::{
    AuthMiddleware, RateLimitMiddleware, RequestIdMiddleware, is_public_route,
};
use litellm_rs::server::routes;
use litellm_rs::server::state::AppState;
use litellm_rs::utils::auth::crypto::keys::{extract_api_key_prefix, hash_api_key};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const AUTH_PROBE_PATH: &str = "/v1/private/auth-probe";

#[tokio::test]
async fn gh1130_refresh_route_uses_body_token_as_primary_credential() {
    assert!(
        is_public_route("/auth/refresh"),
        "global auth middleware must allow the refresh handler to validate its body token"
    );
    assert!(!is_public_route("/auth/refresh/extra"));

    let state = build_test_state(true, true).await;
    let principal = seed_valid_principal(&state).await;
    let user_id = uuid::Uuid::parse_str(&principal.user_id).expect("seeded user UUID");
    let refresh_token = state
        .auth
        .jwt()
        .create_refresh_token(user_id, None)
        .await
        .expect("refresh token");
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .wrap(AuthMiddleware)
            .configure(routes::auth::configure_routes),
    )
    .await;
    let valid_response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/refresh")
            .set_json(serde_json::json!({"refresh_token": refresh_token}))
            .to_request(),
    )
    .await;
    assert_eq!(valid_response.status(), StatusCode::OK);

    let invalid_response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/auth/refresh")
            .set_json(serde_json::json!({"refresh_token": "invalid"}))
            .to_request(),
    )
    .await;

    assert_eq!(
        invalid_response.status(),
        StatusCode::BAD_REQUEST,
        "the request must reach the refresh handler instead of failing as missing authentication"
    );
}

#[derive(Debug, Clone)]
struct SeededPrincipal {
    raw_api_key: String,
    user_id: String,
    api_key_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthProbePayload {
    context_present: bool,
    user_present: bool,
    api_key_present: bool,
    request_id: Option<String>,
    user_id: Option<String>,
    api_key_id: Option<String>,
    api_key_budget_id: Option<String>,
    team_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct LegacyAccessClaims {
    sub: uuid::Uuid,
    iat: u64,
    exp: u64,
    iss: String,
    aud: String,
    jti: String,
    role: String,
    permissions: Vec<String>,
    team_id: Option<uuid::Uuid>,
    session_id: Option<String>,
    token_type: String,
}

async fn auth_probe(req: HttpRequest, hit_counter: web::Data<Arc<AtomicUsize>>) -> HttpResponse {
    hit_counter.fetch_add(1, Ordering::SeqCst);

    let context = req
        .extensions()
        .get::<SharedRequestContext>()
        .cloned()
        .or_else(|| {
            req.extensions()
                .get::<RequestContext>()
                .map(|context| Arc::new(context.clone()))
        });
    let user = req.extensions().get::<User>().cloned();
    let api_key = req.extensions().get::<ApiKey>().cloned();

    let payload = AuthProbePayload {
        context_present: context.is_some(),
        user_present: user.is_some(),
        api_key_present: api_key.is_some(),
        request_id: context.as_ref().map(|ctx| ctx.request_id.clone()),
        user_id: context.as_ref().and_then(|ctx| ctx.user_id.clone()),
        api_key_id: context
            .as_ref()
            .and_then(|ctx| ctx.api_key_id().map(|id| id.to_string())),
        api_key_budget_id: context
            .as_ref()
            .and_then(|ctx| ctx.api_key_budget_id().map(|id| id.to_string())),
        team_id: context
            .as_ref()
            .and_then(|ctx| ctx.team_id().map(|id| id.to_string())),
    };

    HttpResponse::Ok().json(payload)
}

async fn build_test_state_with_rate_limit(
    enable_jwt: bool,
    enable_api_key: bool,
    allow_anonymous: bool,
    default_rpm: Option<u32>,
) -> AppState {
    let mut config = Config::default();
    config.gateway.auth.enable_jwt = enable_jwt;
    config.gateway.auth.enable_api_key = enable_api_key;
    config.gateway.auth.allow_anonymous = allow_anonymous;
    config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
    if let Some(default_rpm) = default_rpm {
        config.gateway.rate_limit.enabled = true;
        config.gateway.rate_limit.default_rpm = default_rpm;
    }

    let server = HttpServer::new(&config)
        .await
        .expect("failed to build HTTP server for auth middleware integration test");
    let state = server.state().clone();
    state
        .storage
        .migrate()
        .await
        .expect("failed to run in-memory DB migrations for auth middleware integration test");
    state
}

async fn build_test_state_with_requests_per_minute_alias(
    default_rpm: u32,
    requests_per_minute: u32,
) -> AppState {
    let mut config = Config::default();
    config.gateway.auth.enable_jwt = true;
    config.gateway.auth.enable_api_key = true;
    config.gateway.auth.allow_anonymous = false;
    config.gateway.auth.jwt_secret = "AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!".to_string();
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = Some("config/model_prices_extended.json".to_string());
    config.gateway.rate_limit.enabled = true;
    config.gateway.rate_limit.default_rpm = default_rpm;
    config.gateway.rate_limit.requests_per_minute = Some(requests_per_minute);

    let server = HttpServer::new(&config)
        .await
        .expect("failed to build HTTP server for requests_per_minute integration test");
    let state = server.state().clone();
    state
        .storage
        .migrate()
        .await
        .expect("failed to run in-memory DB migrations for requests_per_minute test");
    state
}

async fn build_test_state(enable_jwt: bool, enable_api_key: bool) -> AppState {
    build_test_state_with_rate_limit(enable_jwt, enable_api_key, false, None).await
}

async fn seed_valid_principal(state: &AppState) -> SeededPrincipal {
    seed_principal_with_role_and_api_key(
        state,
        UserRole::User,
        vec!["use:api".to_string()],
        Metadata::new(),
    )
    .await
}

async fn seed_principal_with_api_key(
    state: &AppState,
    permissions: Vec<String>,
    metadata: Metadata,
) -> SeededPrincipal {
    seed_principal_with_role_and_api_key(state, UserRole::User, permissions, metadata).await
}

async fn seed_principal_with_role_and_api_key(
    state: &AppState,
    role: UserRole,
    permissions: Vec<String>,
    metadata: Metadata,
) -> SeededPrincipal {
    let mut user = User::new(
        "auth-mw-user".to_string(),
        "auth-mw-user@example.com".to_string(),
        "hashed-password".to_string(),
    );
    user.role = role;
    user.status = UserStatus::Active;

    let user = state
        .storage
        .db()
        .create_user(&user)
        .await
        .expect("failed to insert user for auth middleware integration test");

    let raw_api_key = "gw-valid-auth-middleware-key-123456".to_string();
    let api_key = ApiKey {
        metadata,
        name: "auth-middleware-test-key".to_string(),
        key_hash: hash_api_key(&raw_api_key, None),
        key_prefix: extract_api_key_prefix(&raw_api_key),
        user_id: Some(user.id()),
        team_id: None,
        permissions,
        rate_limits: None,
        expires_at: None,
        is_active: true,
        last_used_at: None,
        usage_stats: UsageStats::default(),
    };

    let api_key = state
        .storage
        .db()
        .create_api_key(&api_key)
        .await
        .expect("failed to insert API key for auth middleware integration test");

    SeededPrincipal {
        raw_api_key,
        user_id: user.id().to_string(),
        api_key_id: api_key.metadata.id.to_string(),
    }
}

#[tokio::test]
async fn gh1130_legacy_access_token_ignores_unproven_team_claim() {
    let state = build_test_state(true, false).await;
    let guessed_team = uuid::Uuid::new_v4();
    let mut user = User::new(
        "legacy-jwt-user".to_string(),
        "legacy-jwt@example.com".to_string(),
        "hashed-password".to_string(),
    );
    user.status = UserStatus::Active;
    user.team_ids.push(guessed_team);
    let user = state.storage.db().create_user(&user).await.unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let legacy_claims = LegacyAccessClaims {
        sub: user.id(),
        iat: now,
        exp: now + 3600,
        iss: "litellm-rs".to_string(),
        aud: "api".to_string(),
        jti: uuid::Uuid::new_v4().to_string(),
        role: "user".to_string(),
        permissions: vec!["files".to_string()],
        team_id: Some(guessed_team),
        session_id: None,
        token_type: "access".to_string(),
    };
    let token = jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &legacy_claims,
        &jsonwebtoken::EncodingKey::from_secret(b"AaaAaaAaaAaaAaaAaaAaaAaaAaaAaa1!"),
    )
    .unwrap();
    let hit_counter = Arc::new(AtomicUsize::new(0));
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .app_data(web::Data::new(hit_counter))
            .wrap(AuthMiddleware)
            .route(AUTH_PROBE_PATH, web::get().to(auth_probe)),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::get()
            .uri(AUTH_PROBE_PATH)
            .insert_header(("authorization", format!("Bearer {token}")))
            .to_request(),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let payload: AuthProbePayload = test::read_body_json(response).await;
    assert_eq!(
        payload.user_id.as_deref(),
        Some(user.id().to_string().as_str())
    );
    assert_eq!(payload.team_id, None);
}

mod disabled_auth;
mod permissions_context;
mod rejection_rate_limit;
