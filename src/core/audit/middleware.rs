//! Audit middleware for Actix-web
//!
//! This module provides middleware for automatic request/response logging.

use crate::core::types::context::{RequestContext, SharedRequestContext};
use actix_web::body::{BodySize, MessageBody};
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use actix_web::http::header::CONTENT_TYPE;
use actix_web::{Error, HttpMessage};
use futures::future::{LocalBoxFuture, Ready, ready};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::Instant;
use tracing::debug;

use super::events::AuditEvent;
use super::logger::AuditLogger;
pub use super::middleware_body::AuditResponseBody;
use super::middleware_body::{AuditBodyOutcome, AuditTerminalRecorder};
use super::types::{AuditError, RequestLog};

#[derive(Clone, Default)]
struct AuditPrincipal {
    user_id: Option<String>,
    api_key_id: Option<String>,
    team_id: Option<String>,
}

type SharedAuditPrincipal = Arc<RwLock<AuditPrincipal>>;

/// Record authenticated identity in a handle retained by the outer audit layer.
pub(crate) fn record_authenticated_principal(message: &impl HttpMessage, context: &RequestContext) {
    let extensions = message.extensions();
    let Some(principal) = extensions.get::<SharedAuditPrincipal>() else {
        return;
    };
    let mut principal = match principal.write() {
        Ok(principal) => principal,
        Err(poisoned) => {
            tracing::error!("Recovering poisoned audit principal lock");
            poisoned.into_inner()
        }
    };
    principal.user_id.clone_from(&context.user_id);
    principal.api_key_id = context.api_key_id().map(|id| id.to_string());
    principal.team_id = context.team_id().map(|id| id.to_string());
}

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
    type Response = ServiceResponse<AuditResponseBody<B>>;
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
    type Response = ServiceResponse<AuditResponseBody<B>>;
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
            return Box::pin(async move {
                fut.await.map(|response| {
                    response.map_body(|_, body| AuditResponseBody::passthrough(body))
                })
            });
        }

        let principal = Arc::new(RwLock::new(AuditPrincipal::default()));
        req.extensions_mut()
            .insert::<SharedAuditPrincipal>(Arc::clone(&principal));

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
            let terminal_permit = logger
                .start_request(start_event)
                .map_err(audit_service_unavailable)?;
            let result = service.call(req).await;
            let response = match result {
                Ok(response) => response,
                Err(error) => {
                    let event = Self::with_principal(
                        AuditEvent::request_failed(&request_id, error.to_string()),
                        &Self::recorded_principal(&principal),
                    );
                    Self::complete_terminal(&logger, terminal_permit, event);
                    debug!("Request {} failed: {}", request_id, error);
                    return Err(error);
                }
            };

            let status_code = response.status().as_u16();
            let principal = Self::response_principal(&response, &principal);
            let recorder = Self::terminal_recorder(
                Arc::clone(&logger),
                terminal_permit,
                request_id,
                status_code,
                start_time,
                principal,
            );
            let is_stream = matches!(response.response().body().size(), BodySize::Stream);
            let is_sse = response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("text/event-stream"));
            Ok(response.map_body(|_, body| {
                if is_stream {
                    AuditResponseBody::streaming(body, recorder, is_sse)
                } else {
                    recorder.record(AuditBodyOutcome::Completed);
                    AuditResponseBody::passthrough(body)
                }
            }))
        })
    }
}

fn audit_service_unavailable(error: AuditError) -> Error {
    tracing::error!("Audit logging unavailable: {error}");
    actix_web::error::ErrorServiceUnavailable("audit logging unavailable")
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
    fn recorded_principal(principal: &SharedAuditPrincipal) -> AuditPrincipal {
        let principal = match principal.read() {
            Ok(principal) => principal,
            Err(poisoned) => {
                tracing::error!("Recovering poisoned audit principal lock");
                poisoned.into_inner()
            }
        };
        principal.clone()
    }

    fn with_principal(mut event: AuditEvent, principal: &AuditPrincipal) -> AuditEvent {
        if let Some(user_id) = principal.user_id.as_deref() {
            event = event.with_user_id(user_id);
        }
        if let Some(api_key_id) = principal.api_key_id.as_deref() {
            event = event.with_api_key_id(api_key_id);
        }
        if let Some(team_id) = principal.team_id.as_deref() {
            event = event.with_team_id(team_id);
        }
        event
    }

    fn response_principal<B>(
        response: &ServiceResponse<B>,
        recorded: &SharedAuditPrincipal,
    ) -> AuditPrincipal {
        let mut principal = Self::recorded_principal(recorded);
        let extensions = response.request().extensions();
        let Some(context) = extensions.get::<SharedRequestContext>() else {
            return principal;
        };
        principal.user_id.clone_from(&context.user_id);
        principal.api_key_id = context.api_key_id().map(|id| id.to_string());
        principal.team_id = context.team_id().map(|id| id.to_string());
        principal
    }

    fn terminal_recorder(
        logger: Arc<AuditLogger>,
        permit: super::logger::AuditEventPermit,
        request_id: String,
        status_code: u16,
        start_time: Instant,
        principal: AuditPrincipal,
    ) -> AuditTerminalRecorder {
        AuditTerminalRecorder::new(move |outcome| {
            let duration_ms = start_time.elapsed().as_millis() as u64;
            let event = match outcome {
                AuditBodyOutcome::Completed => {
                    AuditEvent::request_completed(&request_id, status_code, duration_ms)
                }
                AuditBodyOutcome::Failed(message) => {
                    AuditEvent::request_failed(&request_id, message)
                }
            };
            Self::complete_terminal(&logger, permit, Self::with_principal(event, &principal));
            debug!(
                "Request {} terminated: status={}, duration={}ms",
                request_id, status_code, duration_ms
            );
        })
    }

    fn complete_terminal(
        logger: &AuditLogger,
        permit: super::logger::AuditEventPermit,
        event: AuditEvent,
    ) {
        if let Err(error) = logger.complete_request(permit, event) {
            tracing::error!("Failed to record reserved audit terminal event: {error}");
        }
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
    use bytes::Bytes;
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
    async fn failed_requests_retain_the_authenticated_principal() {
        let (logger, events) = recording_logger().await;
        let user_id = uuid::Uuid::new_v4();
        let api_key_id = uuid::Uuid::new_v4();
        let team_id = uuid::Uuid::new_v4();
        let context = RequestContext::new()
            .with_user(user_id, Some(team_id))
            .with_api_key(api_key_id);
        let context = Arc::new(context);
        let service = actix_web::dev::fn_service(move |request: ServiceRequest| {
            let context = Arc::clone(&context);
            async move {
                record_authenticated_principal(&request, &context);
                Err::<ServiceResponse<actix_web::body::BoxBody>, _>(
                    actix_web::error::ErrorBadRequest("invalid"),
                )
            }
        });
        let service = AuditMiddleware::new(Arc::clone(&logger))
            .new_transform(service)
            .await
            .expect("audit transform");
        let request = actix_test::TestRequest::get()
            .uri("/failed")
            .to_srv_request();

        assert!(service.call(request).await.is_err());
        logger.shutdown().await.expect("audit shutdown");
        let events = events.lock().await;

        assert_eq!(events.len(), 2);
        assert_eq!(events[1].event_type, EventType::RequestFailed);
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

    #[actix_web::test]
    async fn streaming_audit_waits_for_body_and_records_error_event() {
        let (logger, events) = recording_logger().await;
        let app = actix_test::init_service(
            App::new()
                .wrap(AuditMiddleware::new(Arc::clone(&logger)))
                .route(
                    "/stream",
                    web::get().to(|| async {
                        let body = futures::stream::iter(vec![Ok::<_, actix_web::Error>(
                            Bytes::from_static(b"data: {\"error\":{\"code\":\"timeout\"}}\n\n"),
                        )]);
                        HttpResponse::Ok()
                            .insert_header((CONTENT_TYPE, "text/event-stream"))
                            .streaming(body)
                    }),
                ),
        )
        .await;
        let request = actix_test::TestRequest::get().uri("/stream").to_request();

        let response = actix_test::call_service(&app, request).await;
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if events.lock().await.len() == 1 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("request start audit should be written");
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(events.lock().await.len(), 1);

        drop(actix_test::read_body(response).await);
        logger.shutdown().await.expect("audit shutdown");
        let events = events.lock().await;

        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, EventType::RequestStarted);
        assert_eq!(events[1].event_type, EventType::RequestFailed);
        assert!(events[1].message.contains("stream emitted an error event"));
    }
}
