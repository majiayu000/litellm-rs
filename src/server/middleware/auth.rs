//! Authentication middleware

use crate::auth::AuthMethod;
use crate::core::models::{ApiKey, user::types::User};
use crate::core::types::context::RequestContext;
use crate::server::middleware::auth_rate_limiter::get_auth_rate_limiter;
use crate::server::middleware::helpers::{extract_auth_method, is_public_route};
use crate::server::state::AppState;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use actix_web::http::header::{HeaderName, HeaderValue};
use actix_web::{HttpMessage, HttpRequest, web};
use futures::future::{Ready, ready};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use tracing::{debug, warn};
use uuid::Uuid;

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

            let context = build_request_context(&mut req);
            let auth_method = extract_auth_method(req.headers());
            let client_id = get_client_identifier(&req);
            let rate_limiter = get_auth_rate_limiter();

            let app_state = match req.app_data::<web::Data<AppState>>().cloned() {
                Some(state) => state,
                None => {
                    return Err(actix_web::error::ErrorInternalServerError(
                        "Missing application state",
                    ));
                }
            };

            if is_public {
                req.extensions_mut().insert(context);
                return service.call(req).await;
            }

            let auth_enabled =
                app_state.config.auth().enable_jwt || app_state.config.auth().enable_api_key;
            if !auth_enabled {
                req.extensions_mut().insert(context);
                return service.call(req).await;
            }

            if let Err(wait_seconds) = rate_limiter.check_allowed(&client_id) {
                return Err(actix_web::error::ErrorTooManyRequests(format!(
                    "Too many failed attempts. Try again in {} seconds",
                    wait_seconds
                )));
            }

            let auth_method = match auth_method {
                AuthMethod::Jwt(_) if !app_state.config.auth().enable_jwt => {
                    rate_limiter.record_failure(&client_id);
                    return Err(actix_web::error::ErrorUnauthorized(
                        "JWT authentication disabled",
                    ));
                }
                AuthMethod::ApiKey(_) if !app_state.config.auth().enable_api_key => {
                    rate_limiter.record_failure(&client_id);
                    return Err(actix_web::error::ErrorUnauthorized(
                        "API key authentication disabled",
                    ));
                }
                other => other,
            };

            if matches!(auth_method, AuthMethod::None) {
                rate_limiter.record_failure(&client_id);
                return Err(actix_web::error::ErrorUnauthorized(
                    "Missing authentication",
                ));
            }

            match app_state.auth.authenticate(auth_method, context).await {
                Ok(result) if result.success => {
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
                    Err(actix_web::error::ErrorUnauthorized(
                        result.error.unwrap_or_else(|| "Unauthorized".to_string()),
                    ))
                }
                Err(err) => {
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

/// Extract request context from request
pub fn get_request_context(req: &HttpRequest) -> Result<RequestContext, actix_web::Error> {
    req.extensions()
        .get::<RequestContext>()
        .cloned()
        .ok_or_else(|| actix_web::error::ErrorInternalServerError("Missing request context"))
}

/// Extract a client identifier for rate limiting
fn get_client_identifier(req: &ServiceRequest) -> String {
    let ip = req
        .connection_info()
        .peer_addr()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    if let Some(api_key) = req
        .headers()
        .get("x-api-key")
        .or_else(|| req.headers().get("authorization"))
        .and_then(|h| h.to_str().ok())
    {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        api_key.hash(&mut hasher);
        format!("{}:{:x}", ip, hasher.finish())
    } else {
        format!("ip:{}", ip)
    }
}

fn build_request_context(req: &mut ServiceRequest) -> RequestContext {
    let mut context = RequestContext::new();

    let existing_id = req
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);

    let request_id = if let Some(id) = existing_id {
        id
    } else {
        let id = Uuid::new_v4().to_string();
        req.headers_mut().insert(
            HeaderName::from_static("x-request-id"),
            HeaderValue::from_str(&id).unwrap_or_else(|_| HeaderValue::from_static("invalid")),
        );
        id
    };

    context.request_id = request_id;
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
