//! Authentication middleware

use crate::auth::{AUTHENTICATION_SERVICE_UNAVAILABLE_MESSAGE, AuthMethod};
use crate::core::audit::middleware::record_authenticated_principal;
use crate::core::models::{ApiKey, user::types::User};
use crate::core::types::context::{RequestContext, SharedRequestContext};
use crate::server::middleware::auth_rate_limiter::get_auth_rate_limiter;
use crate::server::middleware::helpers::{
    extract_auth_method_with_api_key_header, is_public_route, middleware_gateway_error_response,
};
use crate::server::middleware::rate_limit::{
    AuthRateLimitReservation, RateLimitError, enforce_rate_limit_for_rejected_auth,
    network_client_key, reserve_rate_limit_for_auth_attempt,
};
use crate::server::routes::ai::{self, api_key_allows_endpoint, check_permission};
use crate::server::state::AppState;
use crate::utils::error::gateway_error::GatewayError;
use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use actix_web::{HttpMessage, HttpRequest, web};
use futures::future::{Ready, ready};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use tracing::{debug, error, warn};

/// Auth middleware for Actix-web
pub struct AuthMiddleware;

fn bypasses_header_auth(path: &str) -> bool {
    is_public_route(path) || path == "/auth/refresh"
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service: Rc::new(service),
        }))
    }
}

/// Service implementation for auth middleware
pub struct AuthMiddlewareService<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);

        Box::pin(async move {
            // Check public route with &str reference before any mutable borrows,
            // avoiding a per-request String allocation for the path.
            let is_public = bypasses_header_auth(req.path());

            let app_state = match req.app_data::<web::Data<AppState>>().cloned() {
                Some(state) => state,
                None => {
                    return Err(actix_web::error::ErrorInternalServerError(
                        "Missing application state",
                    ));
                }
            };
            let cfg = app_state.config.load();
            let enable_jwt = cfg.auth().enable_jwt;
            let enable_api_key = cfg.auth().enable_api_key;
            let api_key_header = cfg.auth().api_key_header.clone();
            let rate_limit_enabled = cfg.gateway.rate_limit.enabled;
            let rate_limit_rpm = cfg.gateway.rate_limit.effective_rpm();
            let trusted_proxies = cfg.server().trusted_proxies.clone();

            let context = build_request_context(&mut req);
            let auth_method =
                extract_auth_method_with_api_key_header(req.headers(), api_key_header.as_str());
            let client_id = get_client_identifier(&req, &trusted_proxies);
            let network_rate_limit_enabled = rate_limit_enabled && client_id.is_some();
            let rate_limiter = get_auth_rate_limiter();

            if is_public {
                insert_request_context(&mut req, context);
                return service
                    .call(req)
                    .await
                    .map(ServiceResponse::map_into_left_body);
            }

            let auth_enabled = enable_jwt || enable_api_key;
            if !auth_enabled {
                // Fail closed: both auth methods are disabled. Only allow the
                // request through when anonymous access was explicitly opted
                // into. AuthConfig::validate() already rejects this combination,
                // but guard here as defense in depth in case validation was
                // bypassed.
                if !cfg.auth().allow_anonymous {
                    error!(
                        "All authentication methods are disabled and allow_anonymous is false; \
                         rejecting request to non-public route. Enable JWT or API key auth, or \
                         set allow_anonymous: true (development only)."
                    );
                    return Ok(unauthorized_response(
                        req,
                        "Authentication is not configured",
                    ));
                }
                insert_request_context(&mut req, context);
                return service
                    .call(req)
                    .await
                    .map(ServiceResponse::map_into_left_body);
            }

            let auth_attempt = match rate_limiter.reserve_network_attempt(client_id.as_deref()) {
                Ok(reservation) => reservation,
                Err(wait_seconds) => {
                    if let Err(error) = enforce_gateway_rate_limit_for_auth_rejection(
                        &req,
                        network_rate_limit_enabled,
                        rate_limit_rpm,
                        &trusted_proxies,
                    )
                    .await
                    {
                        return Ok(rate_limit_response(req, error));
                    }
                    return Ok(failed_attempt_rate_limit_response(req, wait_seconds));
                }
            };

            let auth_method = match auth_method {
                AuthMethod::Jwt(_) if !enable_jwt => {
                    auth_attempt.record_failure();
                    if let Err(error) = enforce_gateway_rate_limit_for_auth_rejection(
                        &req,
                        network_rate_limit_enabled,
                        rate_limit_rpm,
                        &trusted_proxies,
                    )
                    .await
                    {
                        return Ok(rate_limit_response(req, error));
                    }
                    return Ok(unauthorized_response(req, "JWT authentication disabled"));
                }
                AuthMethod::ApiKey(_) if !enable_api_key => {
                    auth_attempt.record_failure();
                    if let Err(error) = enforce_gateway_rate_limit_for_auth_rejection(
                        &req,
                        network_rate_limit_enabled,
                        rate_limit_rpm,
                        &trusted_proxies,
                    )
                    .await
                    {
                        return Ok(rate_limit_response(req, error));
                    }
                    return Ok(unauthorized_response(
                        req,
                        "API key authentication disabled",
                    ));
                }
                other => other,
            };

            if matches!(auth_method, AuthMethod::None) {
                auth_attempt.record_failure();
                if let Err(error) = enforce_gateway_rate_limit_for_auth_rejection(
                    &req,
                    network_rate_limit_enabled,
                    rate_limit_rpm,
                    &trusted_proxies,
                )
                .await
                {
                    return Ok(rate_limit_response(req, error));
                }
                return Ok(unauthorized_response(req, "Missing authentication"));
            }

            let mut auth_rate_limit_reservation = if requires_auth_verification(&auth_method) {
                match reserve_gateway_rate_limit_before_auth(
                    &req,
                    network_rate_limit_enabled,
                    rate_limit_rpm,
                    &trusted_proxies,
                )
                .await
                {
                    Ok(reservation) => Some(reservation),
                    Err(error) => return Ok(rate_limit_response(req, error)),
                }
            } else {
                None
            };

            match app_state.auth.authenticate(auth_method, context).await {
                Ok(result) if result.success => {
                    auth_attempt.release();
                    if let Some(reservation) = auth_rate_limit_reservation.take() {
                        reservation.release().await;
                    }
                    // This is an IP-wide failure bucket, so a valid credential
                    // cannot prove that earlier failures came from the same
                    // principal. Preserve the failures to prevent an attacker
                    // with one valid credential from resetting brute-force
                    // attempts against another credential.
                    debug!("Authentication succeeded");

                    // Attach the authenticated principal before authorization
                    // checks so audit middleware can attribute 403 responses.
                    record_authenticated_principal(&req, &result.context);
                    insert_request_context(&mut req, result.context);
                    match api_key_allows_endpoint(result.api_key.as_ref(), req.path()) {
                        Ok(true) => {}
                        Ok(false) => {
                            warn!("Authenticated API key is not permitted to access this endpoint");
                            return Ok(forbidden_response(
                                req,
                                "API key is not permitted for this endpoint",
                            ));
                        }
                        Err(error) => {
                            warn!("Authenticated API key policy is invalid: {}", error);
                            return Ok(authentication_unavailable_response(req));
                        }
                    }
                    if let Some(operation) = ai::operation_for_path(req.path())
                        && !check_permission(
                            result.user.as_ref(),
                            result.api_key.as_ref(),
                            operation,
                        )
                    {
                        warn!(
                            "Authenticated caller is not permitted to access AI operation '{}'",
                            operation
                        );
                        return Ok(forbidden_response(
                            req,
                            "API key is not permitted for this operation",
                        ));
                    }

                    if let Some(user) = result.user {
                        req.extensions_mut().insert::<User>(user);
                    }
                    if let Some(api_key) = result.api_key {
                        req.extensions_mut().insert::<ApiKey>(api_key);
                    }

                    service
                        .call(req)
                        .await
                        .map(ServiceResponse::map_into_left_body)
                }
                Ok(result) => {
                    auth_attempt.record_failure();
                    warn!(
                        "Authentication failed: {}",
                        result
                            .error
                            .clone()
                            .unwrap_or_else(|| "unauthorized".to_string())
                    );
                    if auth_rate_limit_reservation.is_none()
                        && let Err(error) = enforce_gateway_rate_limit_for_auth_rejection(
                            &req,
                            network_rate_limit_enabled,
                            rate_limit_rpm,
                            &trusted_proxies,
                        )
                        .await
                    {
                        return Ok(rate_limit_response(req, error));
                    }
                    Ok(unauthorized_response(
                        req,
                        result.error.unwrap_or_else(|| "Unauthorized".to_string()),
                    ))
                }
                Err(err) => {
                    auth_attempt.release();
                    if let Some(reservation) = auth_rate_limit_reservation.take() {
                        reservation.release().await;
                    }
                    error!(error = %err, "Authentication infrastructure failure");
                    Ok(authentication_unavailable_response(req))
                }
            }
        })
    }
}

async fn enforce_gateway_rate_limit_for_auth_rejection(
    req: &ServiceRequest,
    enabled: bool,
    requests_per_minute: u32,
    trusted_proxies: &[String],
) -> Result<(), RateLimitError> {
    if !enabled {
        return Ok(());
    }

    enforce_rate_limit_for_rejected_auth(req, requests_per_minute, trusted_proxies).await
}

async fn reserve_gateway_rate_limit_before_auth(
    req: &ServiceRequest,
    enabled: bool,
    requests_per_minute: u32,
    trusted_proxies: &[String],
) -> Result<AuthRateLimitReservation, RateLimitError> {
    if !enabled {
        return Ok(AuthRateLimitReservation::noop());
    }

    reserve_rate_limit_for_auth_attempt(req, requests_per_minute, trusted_proxies).await
}

fn requires_auth_verification(auth_method: &AuthMethod) -> bool {
    matches!(
        auth_method,
        AuthMethod::Jwt(_) | AuthMethod::ApiKey(_) | AuthMethod::Session(_)
    )
}

fn unauthorized_response<B>(
    req: ServiceRequest,
    message: impl Into<String>,
) -> ServiceResponse<EitherBody<B>> {
    let message = message.into();
    middleware_gateway_error_response(
        req,
        actix_web::error::ErrorUnauthorized(message.clone()),
        GatewayError::Auth(message),
    )
}

fn forbidden_response<B>(
    req: ServiceRequest,
    message: impl Into<String>,
) -> ServiceResponse<EitherBody<B>> {
    let message = message.into();
    middleware_gateway_error_response(
        req,
        actix_web::error::ErrorForbidden(message.clone()),
        GatewayError::Forbidden(message),
    )
}

fn authentication_unavailable_response<B>(req: ServiceRequest) -> ServiceResponse<EitherBody<B>> {
    if ai::is_openai_compatible_path(req.path()) {
        return req
            .into_response(ai::openai_internal_error_response(
                AUTHENTICATION_SERVICE_UNAVAILABLE_MESSAGE,
            ))
            .map_into_right_body();
    }

    req.error_response(actix_web::error::ErrorInternalServerError(
        AUTHENTICATION_SERVICE_UNAVAILABLE_MESSAGE,
    ))
    .map_into_right_body()
}

fn rate_limit_response<B>(
    req: ServiceRequest,
    error: RateLimitError,
) -> ServiceResponse<EitherBody<B>> {
    let gateway_error = error.gateway_error();
    middleware_gateway_error_response(req, actix_web::Error::from(error), gateway_error)
}

fn failed_attempt_rate_limit_response<B>(
    req: ServiceRequest,
    wait_seconds: u64,
) -> ServiceResponse<EitherBody<B>> {
    let message = format!(
        "Too many failed attempts. Try again in {} seconds",
        wait_seconds
    );
    middleware_gateway_error_response(
        req,
        actix_web::error::ErrorTooManyRequests(message.clone()),
        GatewayError::RateLimit {
            message,
            retry_after: Some(wait_seconds),
            rpm_limit: None,
            tpm_limit: None,
        },
    )
}

/// Extract request context from request
pub fn get_request_context(req: &HttpRequest) -> Result<SharedRequestContext, actix_web::Error> {
    if let Some(context) = req.extensions().get::<SharedRequestContext>() {
        return Ok(Arc::clone(context));
    }

    req.extensions()
        .get::<RequestContext>()
        .map(|context| Arc::new(context.clone()))
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("Missing request context"))
}

fn insert_request_context(req: &mut ServiceRequest, context: RequestContext) {
    req.extensions_mut()
        .insert::<SharedRequestContext>(Arc::new(context));
}

/// Extract a client identifier for the brute-force lockout limiter.
///
/// The bucket must be keyed by network identity only. Mixing the presented
/// credential into the key gives every guessed secret its own bucket, so an
/// attacker rotating random credentials never accumulates failures in any
/// one bucket and is never locked out — which defeats the limiter entirely.
/// Requests without a transport peer return `None` so unrelated internal
/// callers never collide in a process-wide `ip:unknown` bucket.
fn get_client_identifier(req: &ServiceRequest, trusted_proxies: &[String]) -> Option<String> {
    req.peer_addr()
        .map(|_| network_client_key(req, trusted_proxies))
}

fn build_request_context(req: &mut ServiceRequest) -> RequestContext {
    let mut context = RequestContext::new();

    // Use the request ID set by RequestIdMiddleware when present; otherwise keep
    // the UUID that RequestContext::new() already generated so that AuthMiddleware
    // remains self-sufficient when used without RequestIdMiddleware in the stack.
    if let Some(id) = req
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .filter(|s| !s.is_empty())
    {
        context.request_id = id.to_string();
    }

    context.user_agent = req
        .headers()
        .get("user-agent")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    context.client_ip = req.connection_info().peer_addr().map(|ip| ip.to_string());

    let mut headers = HashMap::new();
    for (name, value) in req.headers().iter() {
        if name.as_str().eq_ignore_ascii_case("authorization")
            || name.as_str().eq_ignore_ascii_case("x-api-key")
        {
            continue;
        }
        if let Ok(value) = value.to_str() {
            headers.insert(name.as_str().to_string(), value.to_string());
        }
    }
    context.headers = headers;

    context
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;

    fn client_id_for_header(header_name: &'static str, header_value: &'static str) -> String {
        let req = TestRequest::default()
            .peer_addr("203.0.113.55:1000".parse().unwrap())
            .insert_header((header_name, header_value))
            .to_srv_request();
        get_client_identifier(&req, &[]).unwrap()
    }

    #[test]
    fn client_identifier_groups_api_key_transports() {
        let configured = client_id_for_header("x-litellm-key", "gw-same-key");
        let fallback = client_id_for_header("x-api-key", "gw-same-key");
        let authorization_scheme = client_id_for_header("authorization", "ApiKey gw-same-key");
        let authorization_raw = client_id_for_header("authorization", "gw-same-key");

        assert_eq!(configured, fallback);
        assert_eq!(configured, authorization_scheme);
        assert_eq!(configured, authorization_raw);
    }

    #[test]
    fn client_identifier_groups_rotated_credentials_into_one_bucket() {
        // The previous behavior asserted distinct buckets per credential,
        // which let an attacker rotating random secrets dodge the lockout
        // entirely. Failures must accumulate in one per-IP bucket.
        let first = client_id_for_header("x-api-key", "gw-first-key");
        let second = client_id_for_header("x-api-key", "gw-second-key");

        assert_eq!(first, second);
        assert_eq!(first, "ip:203.0.113.55");
    }

    #[test]
    fn client_identifier_groups_jwt_and_api_key_guesses_from_same_ip() {
        let api_key = client_id_for_header("x-api-key", "gw-some-key");
        let jwt = client_id_for_header("authorization", "Bearer some.jwt.token");

        assert_eq!(api_key, jwt);
        assert_eq!(jwt, "ip:203.0.113.55");
    }

    #[test]
    fn client_identifier_ignores_peer_port() {
        let req_a = TestRequest::default()
            .peer_addr("203.0.113.60:1000".parse().unwrap())
            .insert_header(("x-api-key", "gw-same-key"))
            .to_srv_request();
        let req_b = TestRequest::default()
            .peer_addr("203.0.113.60:2000".parse().unwrap())
            .insert_header(("x-api-key", "gw-same-key"))
            .to_srv_request();
        assert_eq!(
            get_client_identifier(&req_a, &[]),
            get_client_identifier(&req_b, &[])
        );
    }

    #[test]
    fn client_identifier_falls_back_to_ip_without_auth() {
        let req = TestRequest::default()
            .peer_addr("203.0.113.70:1000".parse().unwrap())
            .to_srv_request();

        assert_eq!(
            get_client_identifier(&req, &[]).as_deref(),
            Some("ip:203.0.113.70")
        );
    }

    #[test]
    fn client_identifier_keeps_session_failures_in_ip_bucket() {
        let req_a = TestRequest::default()
            .peer_addr("203.0.113.80:1000".parse().unwrap())
            .insert_header(("cookie", "session=session-a"))
            .to_srv_request();
        let req_b = TestRequest::default()
            .peer_addr("203.0.113.80:1000".parse().unwrap())
            .insert_header(("cookie", "session=session-b"))
            .to_srv_request();
        assert_eq!(
            get_client_identifier(&req_a, &[]),
            get_client_identifier(&req_b, &[])
        );
        assert_eq!(
            get_client_identifier(&req_a, &[]).as_deref(),
            Some("ip:203.0.113.80")
        );
    }

    #[test]
    fn client_identifier_uses_forwarded_client_only_for_trusted_peer() {
        let trusted = vec!["192.0.2.10".to_string()];
        let trusted_req = TestRequest::default()
            .peer_addr("192.0.2.10:443".parse().unwrap())
            .insert_header(("x-forwarded-for", "198.51.100.20, 192.0.2.10"))
            .to_srv_request();
        let untrusted_req = TestRequest::default()
            .peer_addr("192.0.2.11:443".parse().unwrap())
            .insert_header(("x-forwarded-for", "198.51.100.20"))
            .to_srv_request();

        assert_eq!(
            get_client_identifier(&trusted_req, &trusted).as_deref(),
            Some("ip:198.51.100.20")
        );
        assert_eq!(
            get_client_identifier(&untrusted_req, &trusted).as_deref(),
            Some("ip:192.0.2.11")
        );
    }

    #[test]
    fn client_identifier_without_transport_peer_is_untracked() {
        let req = TestRequest::default()
            .insert_header(("x-forwarded-for", "198.51.100.20"))
            .to_srv_request();

        assert_eq!(get_client_identifier(&req, &[]), None);
    }

    #[test]
    fn auth_verification_precheck_only_applies_to_present_credentials() {
        assert!(requires_auth_verification(&AuthMethod::Jwt(
            "jwt-token".to_string()
        )));
        assert!(requires_auth_verification(&AuthMethod::ApiKey(
            "api-key".to_string()
        )));
        assert!(requires_auth_verification(&AuthMethod::Session(
            "session-id".to_string()
        )));
        assert!(!requires_auth_verification(&AuthMethod::None));
    }

    #[test]
    fn insert_request_context_stores_shared_extension_handle() {
        let api_key_id = uuid::Uuid::new_v4();
        let mut req = TestRequest::default().to_srv_request();

        insert_request_context(&mut req, RequestContext::new().with_api_key(api_key_id));

        let extensions = req.extensions();
        let stored = extensions
            .get::<SharedRequestContext>()
            .expect("request context should be stored as a shared handle");
        assert_eq!(stored.api_key_id(), Some(api_key_id));
        assert!(extensions.get::<RequestContext>().is_none());
    }

    #[test]
    fn forbidden_response_retains_authenticated_principal_context() {
        let user_id = uuid::Uuid::new_v4();
        let api_key_id = uuid::Uuid::new_v4();
        let team_id = uuid::Uuid::new_v4();
        let mut req = TestRequest::default().to_srv_request();
        insert_request_context(
            &mut req,
            RequestContext::new()
                .with_user(user_id, Some(team_id))
                .with_api_key(api_key_id),
        );

        let response = forbidden_response::<actix_web::body::BoxBody>(req, "denied");
        let extensions = response.request().extensions();
        let context = extensions
            .get::<SharedRequestContext>()
            .expect("authenticated context should survive a 403 response");

        assert_eq!(
            context.user_id.as_deref(),
            Some(user_id.to_string().as_str())
        );
        assert_eq!(context.api_key_id(), Some(api_key_id));
        assert_eq!(context.team_id(), Some(team_id));
    }

    #[test]
    fn build_request_context_excludes_sensitive_auth_headers() {
        let mut req = TestRequest::default()
            .insert_header(("authorization", "Bearer secret"))
            .insert_header(("x-api-key", "sk-secret"))
            .insert_header(("x-request-id", "req-123"))
            .insert_header(("x-observable", "kept"))
            .to_srv_request();

        let context = build_request_context(&mut req);

        assert_eq!(context.request_id, "req-123");
        assert_eq!(
            context.headers.get("x-observable").map(String::as_str),
            Some("kept")
        );
        assert!(!context.headers.contains_key("authorization"));
        assert!(!context.headers.contains_key("x-api-key"));
    }

    #[actix_web::test]
    async fn authentication_unavailable_response_is_generic_server_error() {
        let req = TestRequest::with_uri("/v1/chat/completions").to_srv_request();
        let response = authentication_unavailable_response::<actix_web::body::BoxBody>(req);

        assert_eq!(
            response.status(),
            actix_web::http::StatusCode::INTERNAL_SERVER_ERROR
        );
        let body = actix_web::body::to_bytes(response.into_body())
            .await
            .expect("generic authentication error body should render");
        let body: serde_json::Value = serde_json::from_slice(&body)
            .expect("generic authentication error body should be valid JSON");
        assert_eq!(
            body["error"]["message"],
            AUTHENTICATION_SERVICE_UNAVAILABLE_MESSAGE
        );
        let body = body.to_string();
        for internal_detail in [
            "Storage error",
            "Database error",
            "Redis error",
            "Connection closed",
        ] {
            assert!(!body.contains(internal_detail));
        }
    }
}
