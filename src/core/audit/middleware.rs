//! Audit middleware for Actix-web
//!
//! This module provides middleware for automatic request/response logging.

use crate::core::request_ledger::{
    RequestLedgerFacts, RequestLedgerRecord, RequestLedgerRuntime, SharedRequestLedgerFacts,
    persist_with_policy, scope_facts, snapshot_facts,
};
use crate::core::types::context::{RequestContext, SharedRequestContext};
use actix_web::body::{BodySize, MessageBody};
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use actix_web::http::header::CONTENT_TYPE;
use actix_web::{Error, HttpMessage};
use chrono::{DateTime, Utc};
use futures::future::{LocalBoxFuture, Ready, ready};
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;
use tracing::debug;

use super::events::AuditEvent;
use super::logger::AuditLogger;
pub use super::middleware_body::AuditResponseBody;
use super::middleware_body::{AuditBodyOutcome, AuditTerminalRecorder};
use super::types::{AuditError, RequestLog};

const OPERATIONAL_LEDGER_PATHS: &[&str] = &["/health", "/metrics", "/ready", "/live", "/healthz"];

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
    ledger: Option<Arc<RequestLedgerRuntime>>,
}

impl AuditMiddleware {
    /// Create a new audit middleware
    pub fn new(logger: Arc<AuditLogger>) -> Self {
        Self {
            logger,
            trusted_proxies: Arc::new(Vec::new()),
            ledger: None,
        }
    }

    /// Create audit middleware with explicitly trusted immediate proxy IPs.
    pub fn with_trusted_proxies(logger: Arc<AuditLogger>, trusted_proxies: Vec<String>) -> Self {
        Self {
            logger,
            trusted_proxies: Arc::new(trusted_proxies),
            ledger: None,
        }
    }

    /// Persist one metadata-only terminal request-ledger row per request.
    pub fn with_request_ledger(mut self, ledger: Arc<RequestLedgerRuntime>) -> Self {
        self.ledger = Some(ledger);
        self
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
            ledger: self.ledger.clone(),
        }))
    }
}

/// Service implementation for audit middleware
pub struct AuditMiddlewareService<S> {
    service: Rc<S>,
    logger: Arc<AuditLogger>,
    trusted_proxies: Arc<Vec<String>>,
    ledger: Option<Arc<RequestLedgerRuntime>>,
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
        let ledger = self.ledger.clone();
        let path = req.path().to_string();
        let method = req.method().to_string();
        let persist_ledger = ledger.is_some() && !OPERATIONAL_LEDGER_PATHS.contains(&path.as_str());

        if !logger.should_log_path(&path) && !persist_ledger {
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
        let facts = SharedRequestLedgerFacts::new(Mutex::new(RequestLedgerFacts::default()));
        req.extensions_mut().insert(Arc::clone(&facts));

        let request_id = req
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        let client_ip = trusted_client_ip(&req, &trusted_proxies);

        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        let mut request_log = RequestLog::new(&request_id, &method, &path);
        if let Some(ip) = client_ip {
            request_log = request_log.with_client_ip(ip);
        }
        if let Some(ua) = user_agent {
            request_log = request_log.with_user_agent(ua);
        }

        let start_event = AuditEvent::request_started(&request_id, &path).with_request(request_log);
        let start_time = Instant::now();
        let started_at = Utc::now();

        Box::pin(scope_facts(Arc::clone(&facts), async move {
            let cancellation_request_id = request_id.clone();
            let cancellation_principal = Arc::clone(&principal);
            let terminal_permit = logger
                .start_request(start_event, move || {
                    Self::with_principal(
                        AuditEvent::request_failed(
                            cancellation_request_id,
                            "request future cancelled",
                        ),
                        &Self::recorded_principal(&cancellation_principal),
                    )
                })
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
                    if persist_ledger {
                        let principal = Self::recorded_principal(&principal);
                        Self::persist_ledger(
                            ledger.as_ref(),
                            &request_id,
                            &method,
                            &path,
                            started_at,
                            start_time,
                            0,
                            "failed",
                            &principal,
                            &facts,
                        )
                        .await?;
                    }
                    debug!("Request {} failed: {}", request_id, error);
                    return Err(error);
                }
            };

            let status_code = response.status().as_u16();
            let principal = Self::response_principal(&response, &principal);
            let is_stream = matches!(response.response().body().size(), BodySize::Stream);
            if persist_ledger && !is_stream {
                Self::persist_ledger(
                    ledger.as_ref(),
                    &request_id,
                    &method,
                    &path,
                    started_at,
                    start_time,
                    status_code,
                    "completed",
                    &principal,
                    &facts,
                )
                .await?;
            }
            let recorder = Self::terminal_recorder(
                Arc::clone(&logger),
                terminal_permit,
                request_id,
                status_code,
                start_time,
                principal,
                ledger.filter(|_| persist_ledger && is_stream),
                facts,
                method,
                path,
                started_at,
            );
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
        }))
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

    #[allow(clippy::too_many_arguments)]
    fn terminal_recorder(
        logger: Arc<AuditLogger>,
        permit: super::logger::AuditEventPermit,
        request_id: String,
        status_code: u16,
        start_time: Instant,
        principal: AuditPrincipal,
        ledger: Option<Arc<RequestLedgerRuntime>>,
        facts: SharedRequestLedgerFacts,
        method: String,
        path: String,
        started_at: DateTime<Utc>,
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
            if let Some(runtime) = ledger {
                let terminal_status = match outcome {
                    AuditBodyOutcome::Completed => "completed",
                    AuditBodyOutcome::Failed(_) => "failed",
                };
                let record = ledger_record(
                    &request_id,
                    &method,
                    &path,
                    started_at,
                    start_time,
                    status_code,
                    terminal_status,
                    &principal,
                    &snapshot_facts(&facts),
                );
                let policy = runtime.write_failure;
                actix_web::rt::spawn(async move {
                    let _ = persist_with_policy(runtime.writer.as_ref(), record, policy).await;
                });
            }
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

    #[allow(clippy::too_many_arguments)]
    async fn persist_ledger(
        ledger: Option<&Arc<RequestLedgerRuntime>>,
        request_id: &str,
        method: &str,
        path: &str,
        started_at: DateTime<Utc>,
        start_time: Instant,
        status_code: u16,
        terminal_status: &str,
        principal: &AuditPrincipal,
        facts: &SharedRequestLedgerFacts,
    ) -> Result<(), Error> {
        let Some(runtime) = ledger else {
            return Ok(());
        };
        let record = ledger_record(
            request_id,
            method,
            path,
            started_at,
            start_time,
            status_code,
            terminal_status,
            principal,
            &snapshot_facts(facts),
        );
        persist_with_policy(runtime.writer.as_ref(), record, runtime.write_failure)
            .await
            .map_err(|error| {
                tracing::error!("request ledger unavailable: {error}");
                actix_web::error::ErrorServiceUnavailable("request ledger unavailable")
            })
    }
}

#[allow(clippy::too_many_arguments)]
fn ledger_record(
    request_id: &str,
    method: &str,
    path: &str,
    started_at: DateTime<Utc>,
    start_time: Instant,
    status_code: u16,
    terminal_status: &str,
    principal: &AuditPrincipal,
    facts: &RequestLedgerFacts,
) -> RequestLedgerRecord {
    RequestLedgerRecord {
        request_id: request_id.to_string(),
        started_at,
        finished_at: Utc::now(),
        method: method.to_string(),
        endpoint: path.to_string(),
        model: facts.model.clone(),
        provider: facts.provider.clone(),
        deployment: facts.deployment.clone(),
        status_code: i32::from(status_code),
        terminal_status: terminal_status.to_string(),
        latency_ms: start_time.elapsed().as_millis() as i64,
        prompt_tokens: facts.prompt_tokens,
        completion_tokens: facts.completion_tokens,
        total_tokens: facts.total_tokens,
        cost: facts.cost,
        user_id: principal.user_id.clone(),
        api_key_id: principal.api_key_id.clone(),
        team_id: principal.team_id.clone(),
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
    use actix_web::{
        App, HttpRequest, HttpResponse, dev::Service, http::StatusCode, test as actix_test, web,
    };
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

    #[actix_web::test]
    async fn long_lived_requests_do_not_hold_audit_queue_slots() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let output = RecordingOutput {
            events: Arc::clone(&events),
        };
        let mut config = AuditConfig::new().enable();
        config.buffer_size = 2;
        let logger = Arc::new(
            AuditLoggerBuilder::new()
                .config(config)
                .add_output(Box::new(output))
                .build()
                .await
                .expect("recording audit logger"),
        );

        let first = logger
            .start_request(AuditEvent::request_started("first", "/stream"), || {
                AuditEvent::request_failed("first", "cancelled")
            })
            .expect("first request should be accepted");
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            while events.lock().await.is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("worker should drain the first start event");
        let second = logger
            .start_request(AuditEvent::request_started("second", "/stream"), || {
                AuditEvent::request_failed("second", "cancelled")
            })
            .expect("an active stream must not reserve a queue slot");

        logger
            .complete_request(first, AuditEvent::request_completed("first", 200, 1))
            .expect("first terminal event should be delivered");
        logger
            .complete_request(second, AuditEvent::request_completed("second", 200, 1))
            .expect("second terminal event should be delivered");
        logger.shutdown().await.expect("audit shutdown");
        assert_eq!(events.lock().await.len(), 4);
    }

    #[actix_web::test]
    async fn cancelled_request_records_failure_without_stopping_worker() {
        let (logger, events) = recording_logger().await;
        let principal = Arc::new(RwLock::new(AuditPrincipal::default()));
        let cancellation_principal = Arc::clone(&principal);
        let terminal = logger
            .start_request(
                AuditEvent::request_started("cancelled", "/slow"),
                move || {
                    AuditMiddlewareService::<()>::with_principal(
                        AuditEvent::request_failed("cancelled", "request future cancelled"),
                        &AuditMiddlewareService::<()>::recorded_principal(&cancellation_principal),
                    )
                },
            )
            .expect("request should be accepted");

        {
            let mut principal = principal.write().expect("audit principal lock");
            principal.user_id = Some("user-after-start".to_string());
            principal.api_key_id = Some("key-after-start".to_string());
            principal.team_id = Some("team-after-start".to_string());
        }
        let cancellation_time = chrono::Utc::now();
        drop(terminal);
        logger.shutdown().await.expect("audit shutdown");
        let events = events.lock().await;
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, EventType::RequestStarted);
        assert_eq!(events[1].event_type, EventType::RequestFailed);
        assert!(events[1].message.contains("request future cancelled"));
        assert!(events[1].timestamp >= cancellation_time);
        assert_eq!(events[1].user_id.as_deref(), Some("user-after-start"));
        assert_eq!(events[1].api_key_id.as_deref(), Some("key-after-start"));
        assert_eq!(events[1].team_id.as_deref(), Some("team-after-start"));
    }

    struct MemoryLedgerWriter {
        rows: std::sync::Mutex<Vec<RequestLedgerRecord>>,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl crate::core::request_ledger::RequestLedgerWriter for MemoryLedgerWriter {
        async fn persist(&self, record: RequestLedgerRecord) -> Result<(), String> {
            if self.fail {
                return Err("ledger unavailable".to_string());
            }
            self.rows.lock().expect("ledger lock").push(record);
            Ok(())
        }
    }

    #[actix_web::test]
    async fn request_ledger_persists_one_unary_metadata_row() {
        let writer = Arc::new(MemoryLedgerWriter {
            rows: std::sync::Mutex::new(Vec::new()),
            fail: false,
        });
        let runtime = Arc::new(RequestLedgerRuntime {
            writer: writer.clone(),
            write_failure:
                crate::config::models::request_ledger::RequestLedgerWriteFailure::Continue,
        });
        let app = actix_test::init_service(
            App::new()
                .wrap(
                    AuditMiddleware::new(Arc::new(AuditLogger::disabled()))
                        .with_request_ledger(runtime),
                )
                .route(
                    "/v1/chat/completions",
                    web::post().to(|| async { HttpResponse::Ok().finish() }),
                ),
        )
        .await;
        let request = actix_test::TestRequest::post()
            .uri("/v1/chat/completions")
            .insert_header(("x-request-id", "req-ledger-unary"))
            .to_request();
        let response = actix_test::call_service(&app, request).await;
        assert_eq!(response.status(), StatusCode::OK);
        let rows = writer.rows.lock().expect("ledger lock");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].request_id, "req-ledger-unary");
        assert_eq!(rows[0].endpoint, "/v1/chat/completions");
        assert_eq!(rows[0].terminal_status, "completed");
        let json = serde_json::to_value(&rows[0]).expect("serialize");
        assert!(json.get("body").is_none());
        assert!(json.get("authorization").is_none());
    }

    #[actix_web::test]
    async fn request_ledger_fail_policy_rejects_unary_write_errors() {
        let writer = Arc::new(MemoryLedgerWriter {
            rows: std::sync::Mutex::new(Vec::new()),
            fail: true,
        });
        let runtime = Arc::new(RequestLedgerRuntime {
            writer: writer.clone(),
            write_failure: crate::config::models::request_ledger::RequestLedgerWriteFailure::Fail,
        });
        let app = actix_test::init_service(
            App::new()
                .wrap(
                    AuditMiddleware::new(Arc::new(AuditLogger::disabled()))
                        .with_request_ledger(runtime),
                )
                .route(
                    "/v1/models",
                    web::get().to(|| async { HttpResponse::Ok().finish() }),
                ),
        )
        .await;
        let request = actix_test::TestRequest::get()
            .uri("/v1/models")
            .to_request();
        let result = app.call(request).await;
        assert!(
            result.is_err(),
            "fail policy must surface persist errors before the response is committed"
        );
    }

    #[actix_web::test]
    async fn request_ledger_records_stream_disconnect() {
        let writer = Arc::new(MemoryLedgerWriter {
            rows: std::sync::Mutex::new(Vec::new()),
            fail: false,
        });
        let runtime = Arc::new(RequestLedgerRuntime {
            writer: writer.clone(),
            write_failure:
                crate::config::models::request_ledger::RequestLedgerWriteFailure::Continue,
        });
        let app = actix_test::init_service(
            App::new()
                .wrap(
                    AuditMiddleware::new(Arc::new(AuditLogger::disabled()))
                        .with_request_ledger(runtime),
                )
                .route(
                    "/v1/chat/completions",
                    web::get().to(|| async {
                        let body = futures::stream::iter(vec![Ok::<_, actix_web::Error>(
                            Bytes::from_static(b"data: hi\n\n"),
                        )]);
                        HttpResponse::Ok()
                            .insert_header((CONTENT_TYPE, "text/event-stream"))
                            .streaming(body)
                    }),
                ),
        )
        .await;
        let request = actix_test::TestRequest::get()
            .uri("/v1/chat/completions")
            .to_request();
        let response = actix_test::call_service(&app, request).await;
        drop(actix_test::read_body(response).await);
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                if writer.rows.lock().expect("ledger lock").len() == 1 {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("stream ledger row");
        let rows = writer.rows.lock().expect("ledger lock");
        assert_eq!(rows[0].endpoint, "/v1/chat/completions");
        assert!(rows[0].terminal_status == "completed" || rows[0].terminal_status == "failed");
    }
}
