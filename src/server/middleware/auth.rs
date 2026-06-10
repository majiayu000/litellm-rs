//! Authentication middleware

use crate::auth::AuthMethod;
use crate::core::models::{ApiKey, user::types::User};
use crate::core::types::context::RequestContext;
use crate::server::middleware::auth_rate_limiter::get_auth_rate_limiter;
use crate::server::middleware::helpers::{
    extract_auth_method_with_api_key_header, is_public_route,
};
use crate::server::middleware::rate_limit::{
    AuthRateLimitReservation, enforce_rate_limit_for_rejected_auth,
    reserve_rate_limit_for_auth_attempt,
};
use crate::server::state::AppState;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use actix_web::{HttpMessage, HttpRequest, web};
use futures::future::{Ready, ready};
use std::collections::HashMap;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::rc::Rc;
use tracing::{debug, error, warn};

/// Auth middleware for Actix-web
pub struct AuthMiddleware;

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
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
    type Response = ServiceResponse<B>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);

        Box::pin(async move {
            // Check public route with &str reference before any mutable borrows,
            // avoiding a per-request String allocation for the path.
            let is_public = is_public_route(req.path());

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
            let rate_limit_rpm = cfg.gateway.rate_limit.default_rpm;
            let trusted_proxies = cfg.server().trusted_proxies.clone();

            let context = build_request_context(&mut req);
            let auth_method =
                extract_auth_method_with_api_key_header(req.headers(), api_key_header.as_str());
            let client_id = get_client_identifier(&req, &auth_method);
            let rate_limiter = get_auth_rate_limiter();

            if is_public {
                req.extensions_mut().insert(context);
                return service.call(req).await;
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
                    return Err(actix_web::error::ErrorUnauthorized(
                        "Authentication is not configured",
                    ));
                }
                req.extensions_mut().insert(context);
                return service.call(req).await;
            }

            if let Err(wait_seconds) = rate_limiter.check_allowed(&client_id) {
                enforce_gateway_rate_limit_for_auth_rejection(
                    &req,
                    rate_limit_enabled,
                    rate_limit_rpm,
                    &trusted_proxies,
                )
                .await?;
                return Err(actix_web::error::ErrorTooManyRequests(format!(
                    "Too many failed attempts. Try again in {} seconds",
                    wait_seconds
                )));
            }

            let auth_method = match auth_method {
                AuthMethod::Jwt(_) if !enable_jwt => {
                    rate_limiter.record_failure(&client_id);
                    enforce_gateway_rate_limit_for_auth_rejection(
                        &req,
                        rate_limit_enabled,
                        rate_limit_rpm,
                        &trusted_proxies,
                    )
                    .await?;
                    return Err(actix_web::error::ErrorUnauthorized(
                        "JWT authentication disabled",
                    ));
                }
                AuthMethod::ApiKey(_) if !enable_api_key => {
                    rate_limiter.record_failure(&client_id);
                    enforce_gateway_rate_limit_for_auth_rejection(
                        &req,
                        rate_limit_enabled,
                        rate_limit_rpm,
                        &trusted_proxies,
                    )
                    .await?;
                    return Err(actix_web::error::ErrorUnauthorized(
                        "API key authentication disabled",
                    ));
                }
                other => other,
            };

            if matches!(auth_method, AuthMethod::None) {
                rate_limiter.record_failure(&client_id);
                enforce_gateway_rate_limit_for_auth_rejection(
                    &req,
                    rate_limit_enabled,
                    rate_limit_rpm,
                    &trusted_proxies,
                )
                .await?;
                return Err(actix_web::error::ErrorUnauthorized(
                    "Missing authentication",
                ));
            }

            let mut auth_rate_limit_reservation = if requires_auth_verification(&auth_method) {
                Some(
                    reserve_gateway_rate_limit_before_auth(
                        &req,
                        rate_limit_enabled,
                        rate_limit_rpm,
                        &trusted_proxies,
                    )
                    .await?,
                )
            } else {
                None
            };

            match app_state.auth.authenticate(auth_method, context).await {
                Ok(result) if result.success => {
                    if let Some(reservation) = auth_rate_limit_reservation.take() {
                        reservation.release().await;
                    }
                    rate_limiter.record_success(&client_id);
                    debug!("Authentication succeeded");

                    req.extensions_mut().insert(result.context.clone());
                    if let Some(user) = result.user {
                        req.extensions_mut().insert::<User>(user);
                    }
                    if let Some(api_key) = result.api_key {
                        req.extensions_mut().insert::<ApiKey>(api_key);
                    }

                    service.call(req).await
                }
                Ok(result) => {
                    rate_limiter.record_failure(&client_id);
                    warn!(
                        "Authentication failed: {}",
                        result
                            .error
                            .clone()
                            .unwrap_or_else(|| "unauthorized".to_string())
                    );
                    if auth_rate_limit_reservation.is_none() {
                        enforce_gateway_rate_limit_for_auth_rejection(
                            &req,
                            rate_limit_enabled,
                            rate_limit_rpm,
                            &trusted_proxies,
                        )
                        .await?;
                    }
                    Err(actix_web::error::ErrorUnauthorized(
                        result.error.unwrap_or_else(|| "Unauthorized".to_string()),
                    ))
                }
                Err(err) => {
                    if let Some(reservation) = auth_rate_limit_reservation.take() {
                        reservation.release().await;
                    }
                    rate_limiter.record_failure(&client_id);
                    Err(actix_web::error::ErrorInternalServerError(format!(
                        "Authentication error: {}",
                        err
                    )))
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
) -> Result<(), actix_web::Error> {
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
) -> Result<AuthRateLimitReservation, actix_web::Error> {
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

/// Extract request context from request
pub fn get_request_context(req: &HttpRequest) -> Result<RequestContext, actix_web::Error> {
    req.extensions()
        .get::<RequestContext>()
        .cloned()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("Missing request context"))
}

/// Extract a client identifier for rate limiting
fn get_client_identifier(req: &ServiceRequest, auth_method: &AuthMethod) -> String {
    let ip = req
        .connection_info()
        .peer_addr()
        .map(parse_peer_ip)
        .unwrap_or_else(|| "unknown".to_string());

    match auth_method {
        AuthMethod::ApiKey(key) => format!("{}:api_key:{}", ip, hash_credential(key)),
        AuthMethod::Jwt(token) => format!("{}:jwt:{}", ip, hash_credential(token)),
        // Session cookies are untrusted until authentication succeeds, so keep
        // failed session attempts in one stable per-IP lockout bucket.
        AuthMethod::Session(_) => format!("ip:{}", ip),
        AuthMethod::None => format!("ip:{}", ip),
    }
}

fn parse_peer_ip(peer: &str) -> String {
    peer.parse::<SocketAddr>()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|_| peer.to_string())
}

fn hash_credential(credential: &str) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(credential.as_bytes()))
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
    use crate::server::middleware::helpers::extract_auth_method_with_api_key_header;
    use actix_web::test::TestRequest;

    fn client_id_for_header(
        header_name: &'static str,
        header_value: &'static str,
        api_key_header: &str,
    ) -> String {
        let req = TestRequest::default()
            .peer_addr("203.0.113.55:1000".parse().unwrap())
            .insert_header((header_name, header_value))
            .to_srv_request();
        let auth_method = extract_auth_method_with_api_key_header(req.headers(), api_key_header);
        get_client_identifier(&req, &auth_method)
    }

    #[test]
    fn client_identifier_normalizes_api_key_transports() {
        let configured = client_id_for_header("x-litellm-key", "gw-same-key", "x-litellm-key");
        let fallback = client_id_for_header("x-api-key", "gw-same-key", "x-litellm-key");
        let authorization_scheme =
            client_id_for_header("authorization", "ApiKey gw-same-key", "x-litellm-key");
        let authorization_raw =
            client_id_for_header("authorization", "gw-same-key", "x-litellm-key");

        assert_eq!(configured, fallback);
        assert_eq!(configured, authorization_scheme);
        assert_eq!(configured, authorization_raw);
    }

    #[test]
    fn client_identifier_distinguishes_different_credentials() {
        let first = client_id_for_header("x-api-key", "gw-first-key", "x-api-key");
        let second = client_id_for_header("x-api-key", "gw-second-key", "x-api-key");

        assert_ne!(first, second);
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
        let auth_a = extract_auth_method_with_api_key_header(req_a.headers(), "x-api-key");
        let auth_b = extract_auth_method_with_api_key_header(req_b.headers(), "x-api-key");

        assert_eq!(
            get_client_identifier(&req_a, &auth_a),
            get_client_identifier(&req_b, &auth_b)
        );
    }

    #[test]
    fn client_identifier_falls_back_to_ip_without_auth() {
        let req = TestRequest::default()
            .peer_addr("203.0.113.70:1000".parse().unwrap())
            .to_srv_request();

        assert_eq!(
            get_client_identifier(&req, &AuthMethod::None),
            "ip:203.0.113.70"
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
        let auth_a = extract_auth_method_with_api_key_header(req_a.headers(), "x-api-key");
        let auth_b = extract_auth_method_with_api_key_header(req_b.headers(), "x-api-key");

        assert_eq!(
            get_client_identifier(&req_a, &auth_a),
            get_client_identifier(&req_b, &auth_b)
        );
        assert_eq!(get_client_identifier(&req_a, &auth_a), "ip:203.0.113.80");
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
}
