//! Audit middleware for Actix-web
//!
//! This module provides middleware for automatic request/response logging.

use crate::core::types::context::SharedRequestContext;
use actix_web::body::MessageBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use actix_web::{Error, HttpMessage};
use futures::future::{LocalBoxFuture, Ready, ready};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;
use tracing::debug;

use super::events::AuditEvent;
use super::logger::AuditLogger;
use super::types::RequestLog;

/// Audit middleware for Actix-web
pub struct AuditMiddleware {
    logger: Arc<AuditLogger>,
    trusted_proxies: Arc<Vec<String>>,
}

impl AuditMiddleware {
    /// Create a new audit middleware
    pub fn new(logger: Arc<AuditLogger>) -> Self {
        Self {
            logger,
            trusted_proxies: Arc::new(Vec::new()),
        }
    }

    /// Create audit middleware with explicitly trusted immediate proxy IPs.
    pub fn with_trusted_proxies(logger: Arc<AuditLogger>, trusted_proxies: Vec<String>) -> Self {
        Self {
            logger,
            trusted_proxies: Arc::new(trusted_proxies),
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuditMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuditMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuditMiddlewareService {
            service: Rc::new(service),
            logger: self.logger.clone(),
            trusted_proxies: Arc::clone(&self.trusted_proxies),
        }))
    }
}

/// Service implementation for audit middleware
pub struct AuditMiddlewareService<S> {
    service: Rc<S>,
    logger: Arc<AuditLogger>,
    trusted_proxies: Arc<Vec<String>>,
}

impl<S, B> Service<ServiceRequest> for AuditMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let logger = self.logger.clone();
        let service = Rc::clone(&self.service);
        let trusted_proxies = Arc::clone(&self.trusted_proxies);
        let path = req.path().to_string();
        let method = req.method().to_string();

        // Check if path should be logged
        if !logger.should_log_path(&path) {
            let fut = service.call(req);
            return Box::pin(fut);
        }

        // Generate request ID
        let request_id = req
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Extract request info
        let client_ip = trusted_client_ip(&req, &trusted_proxies);

        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        // Create request log
        let mut request_log = RequestLog::new(&request_id, &method, &path);
        if let Some(ip) = client_ip {
            request_log = request_log.with_client_ip(ip);
        }
        if let Some(ua) = user_agent {
            request_log = request_log.with_user_agent(ua);
        }

        // Log request started
        let start_event = AuditEvent::request_started(&request_id, &path).with_request(request_log);

        let start_time = Instant::now();

        Box::pin(async move {
            // Awaiting the enqueue preserves start-before-terminal ordering for
            // every request on the bounded audit channel.
            logger.log(start_event).await;
            let result = service.call(req).await;
            let duration_ms = start_time.elapsed().as_millis() as u64;

            match &result {
                Ok(response) => {
                    let status_code = response.status().as_u16();
                    let event = Self::with_authenticated_principal(
                        AuditEvent::request_completed(&request_id, status_code, duration_ms),
                        response,
                    );
                    logger.log(event).await;
                    debug!(
                        "Request {} completed: status={}, duration={}ms",
                        request_id, status_code, duration_ms
                    );
                }
                Err(e) => {
                    let event = AuditEvent::request_failed(&request_id, e.to_string());
                    logger.log(event).await;
                    debug!("Request {} failed: {}", request_id, e);
                }
            }

            result
        })
    }
}

fn trusted_client_ip(req: &ServiceRequest, trusted_proxies: &[String]) -> Option<String> {
    let peer = req.connection_info().peer_addr()?.to_string();
    let peer_ip = peer
        .parse::<std::net::SocketAddr>()
        .map(|address| address.ip().to_string())
        .unwrap_or(peer);

    if trusted_proxies.iter().any(|proxy| proxy == &peer_ip)
        && let Some(forwarded) = req.headers().get("x-forwarded-for")
        && let Ok(forwarded) = forwarded.to_str()
        && let Some(client_ip) = forwarded
            .split(',')
            .next()
            .map(str::trim)
            .and_then(|value| value.parse::<std::net::IpAddr>().ok())
    {
        return Some(client_ip.to_string());
    }

    Some(peer_ip)
}

impl<S> AuditMiddlewareService<S> {
    fn with_authenticated_principal<B>(
        mut event: AuditEvent,
        response: &ServiceResponse<B>,
    ) -> AuditEvent {
        let extensions = response.request().extensions();
        let Some(context) = extensions.get::<SharedRequestContext>() else {
            return event;
        };

        if let Some(user_id) = context.user_id.as_deref() {
            event = event.with_user_id(user_id);
        }
        if let Some(api_key_id) = context.api_key_id() {
            event = event.with_api_key_id(&api_key_id.to_string());
        }
        if let Some(team_id) = context.team_id() {
            event = event.with_team_id(&team_id.to_string());
        }
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::audit::AuditConfig;
    use crate::core::audit::events::EventType;
    use crate::core::audit::logger::AuditLoggerBuilder;
    use crate::core::audit::outputs::AuditOutput;
    use crate::core::audit::types::AuditResult;
    use crate::core::ip_access::{IpAccessConfig, IpAccessControl, IpAccessMiddleware};
    use crate::core::types::context::RequestContext;
    use actix_web::{App, HttpRequest, HttpResponse, http::StatusCode, test as actix_test, web};
    use tokio::sync::Mutex;

    #[derive(Clone)]
    struct RecordingOutput {
        events: Arc<Mutex<Vec<AuditEvent>>>,
    }

    #[async_trait::async_trait]
    impl AuditOutput for RecordingOutput {
        fn name(&self) -> &str {
            "recording"
        }

        async fn write(&self, event: &AuditEvent) -> AuditResult<()> {
            self.events.lock().await.push(event.clone());
            Ok(())
        }

        async fn flush(&self) -> AuditResult<()> {
            Ok(())
        }

        async fn close(&self) -> AuditResult<()> {
            Ok(())
        }
    }

    async fn recording_logger() -> (Arc<AuditLogger>, Arc<Mutex<Vec<AuditEvent>>>) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let output = RecordingOutput {
            events: Arc::clone(&events),
        };
        let logger = AuditLoggerBuilder::new()
            .config(AuditConfig::new().enable())
            .add_output(Box::new(output))
            .build()
            .await
            .expect("recording audit logger");
        (Arc::new(logger), events)
    }

    #[test]
    fn test_middleware_creation() {
        let logger = Arc::new(AuditLogger::disabled());
        let _middleware = AuditMiddleware::new(logger);
    }

    #[actix_web::test]
    async fn records_start_before_completion_and_attaches_authenticated_principal() {
        let (logger, events) = recording_logger().await;
        let user_id = uuid::Uuid::new_v4();
        let api_key_id = uuid::Uuid::new_v4();
        let team_id = uuid::Uuid::new_v4();
        let context = Arc::new(
            RequestContext::new()
                .with_user(user_id, Some(team_id))
                .with_api_key(api_key_id),
        );
        let app = actix_test::init_service(
            App::new()
                .app_data(web::Data::new(context))
                .wrap(AuditMiddleware::new(Arc::clone(&logger)))
                .route(
                    "/secured",
                    web::get().to(
                        |request: HttpRequest, context: web::Data<SharedRequestContext>| async move {
                            request.extensions_mut().insert(context.get_ref().clone());
                            HttpResponse::Ok().finish()
                        },
                    ),
                ),
        )
        .await;
        let request = actix_test::TestRequest::get()
            .uri("/secured")
            .insert_header(("x-request-id", "req-principal"))
            .insert_header(("x-forwarded-for", "198.51.100.250"))
            .peer_addr("203.0.113.20:9000".parse().expect("valid socket address"))
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        logger.shutdown().await.expect("audit shutdown");
        let events = events.lock().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, EventType::RequestStarted);
        assert_eq!(events[1].event_type, EventType::RequestCompleted);
        assert_eq!(
            events[0]
                .request
                .as_ref()
                .and_then(|request| request.client_ip.as_deref()),
            Some("203.0.113.20")
        );
        assert_eq!(
            events[1].user_id.as_deref(),
            Some(user_id.to_string().as_str())
        );
        assert_eq!(
            events[1].api_key_id.as_deref(),
            Some(api_key_id.to_string().as_str())
        );
        assert_eq!(
            events[1].team_id.as_deref(),
            Some(team_id.to_string().as_str())
        );
    }

    #[actix_web::test]
    async fn audit_accepts_forwarded_ip_only_from_configured_proxy() {
        let (logger, events) = recording_logger().await;
        let app = actix_test::init_service(
            App::new()
                .wrap(AuditMiddleware::with_trusted_proxies(
                    Arc::clone(&logger),
                    vec!["203.0.113.20".to_string()],
                ))
                .route(
                    "/proxied",
                    web::get().to(|| async { HttpResponse::Ok().finish() }),
                ),
        )
        .await;
        let request = actix_test::TestRequest::get()
            .uri("/proxied")
            .insert_header(("x-forwarded-for", "198.51.100.25, 203.0.113.20"))
            .peer_addr("203.0.113.20:9000".parse().expect("valid socket address"))
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        logger.shutdown().await.expect("audit shutdown");
        let events = events.lock().await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            events[0]
                .request
                .as_ref()
                .and_then(|request| request.client_ip.as_deref()),
            Some("198.51.100.25")
        );
    }

    #[actix_web::test]
    async fn audit_layer_outside_ip_policy_records_blocked_requests() {
        let (logger, events) = recording_logger().await;
        let ip_access = Arc::new(
            IpAccessControl::new(IpAccessConfig::new().enable().block_ip("127.0.0.1"))
                .expect("valid IP policy"),
        );
        let app = actix_test::init_service(
            App::new()
                .wrap(IpAccessMiddleware::new(ip_access))
                .wrap(AuditMiddleware::new(Arc::clone(&logger)))
                .route(
                    "/blocked",
                    web::get().to(|| async { HttpResponse::Ok().finish() }),
                ),
        )
        .await;
        let request = actix_test::TestRequest::get()
            .uri("/blocked")
            .peer_addr("127.0.0.1:9000".parse().expect("valid socket address"))
            .to_request();

        let response = actix_test::call_service(&app, request).await;
        logger.shutdown().await.expect("audit shutdown");
        let events = events.lock().await;

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, EventType::RequestStarted);
        assert_eq!(events[1].event_type, EventType::RequestCompleted);
    }
}
