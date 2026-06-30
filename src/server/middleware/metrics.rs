//! Metrics middleware for request monitoring

use actix_web::body::{BodySize, MessageBody};
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform, forward_ready};
use bytes::Bytes;
use futures::future::{Ready, ready};
use pin_project_lite::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tracing::info;

/// Metrics middleware for Actix-web
pub struct MetricsMiddleware;

impl MetricsMiddleware {
    /// Render process-local HTTP request metrics in Prometheus text format.
    pub fn render_prometheus() -> String {
        let snapshot = http_metrics_snapshot();
        format!(
            r#"# HELP gateway_http_requests_total Total HTTP requests observed by the gateway middleware
# TYPE gateway_http_requests_total counter
gateway_http_requests_total {}

# HELP gateway_http_request_errors_total Total HTTP requests with status code >= 400
# TYPE gateway_http_request_errors_total counter
gateway_http_request_errors_total {}

# HELP gateway_http_responses_total Total HTTP responses by status class
# TYPE gateway_http_responses_total counter
gateway_http_responses_total{{class="1xx"}} {}
gateway_http_responses_total{{class="2xx"}} {}
gateway_http_responses_total{{class="3xx"}} {}
gateway_http_responses_total{{class="4xx"}} {}
gateway_http_responses_total{{class="5xx"}} {}

# HELP gateway_http_request_duration_ms_sum Sum of observed HTTP request durations in milliseconds
# TYPE gateway_http_request_duration_ms_sum counter
gateway_http_request_duration_ms_sum {:.3}

# HELP gateway_http_request_duration_ms_count Count of observed HTTP request durations
# TYPE gateway_http_request_duration_ms_count counter
gateway_http_request_duration_ms_count {}
"#,
            snapshot.requests_total,
            snapshot.errors_total,
            snapshot.status_1xx_total,
            snapshot.status_2xx_total,
            snapshot.status_3xx_total,
            snapshot.status_4xx_total,
            snapshot.status_5xx_total,
            snapshot.latency_micros_sum as f64 / 1000.0,
            snapshot.latency_ms_count
        )
    }

    #[cfg(test)]
    pub(crate) fn reset_for_tests() {
        reset_http_metrics_for_tests();
    }

    #[cfg(test)]
    pub(crate) async fn test_lock() -> tokio::sync::MutexGuard<'static, ()> {
        HTTP_METRICS_TEST_LOCK.lock().await
    }
}

impl<S, B> Transform<S, ServiceRequest> for MetricsMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<MetricsResponseBody<B>>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = MetricsMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(MetricsMiddlewareService { service }))
    }
}

/// Service implementation for metrics middleware
pub struct MetricsMiddlewareService<S> {
    service: S,
}

/// Request metrics data
#[derive(Clone)]
pub struct MiddlewareRequestMetrics {
    pub method: String,
    pub path: String,
    pub status_code: u16,
    pub response_time_ms: u64,
    pub request_size: usize,
    pub response_size: usize,
    pub user_agent: Option<String>,
    pub client_ip: Option<String>,
    pub user_id: Option<String>,
    pub api_key_id: Option<String>,
}

/// Snapshot of process-local HTTP request metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HttpMetricsSnapshot {
    requests_total: u64,
    errors_total: u64,
    status_1xx_total: u64,
    status_2xx_total: u64,
    status_3xx_total: u64,
    status_4xx_total: u64,
    status_5xx_total: u64,
    latency_micros_sum: u64,
    latency_ms_count: u64,
}

struct HttpMetricsRegistry {
    requests_total: AtomicU64,
    errors_total: AtomicU64,
    status_1xx_total: AtomicU64,
    status_2xx_total: AtomicU64,
    status_3xx_total: AtomicU64,
    status_4xx_total: AtomicU64,
    status_5xx_total: AtomicU64,
    latency_micros_sum: AtomicU64,
    latency_ms_count: AtomicU64,
}

static HTTP_METRICS: HttpMetricsRegistry = HttpMetricsRegistry {
    requests_total: AtomicU64::new(0),
    errors_total: AtomicU64::new(0),
    status_1xx_total: AtomicU64::new(0),
    status_2xx_total: AtomicU64::new(0),
    status_3xx_total: AtomicU64::new(0),
    status_4xx_total: AtomicU64::new(0),
    status_5xx_total: AtomicU64::new(0),
    latency_micros_sum: AtomicU64::new(0),
    latency_ms_count: AtomicU64::new(0),
};

fn should_record_request_path(path: &str) -> bool {
    path != "/metrics"
}

#[cfg(test)]
static HTTP_METRICS_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Return the current process-local HTTP request metrics.
fn http_metrics_snapshot() -> HttpMetricsSnapshot {
    HttpMetricsSnapshot {
        requests_total: HTTP_METRICS.requests_total.load(Ordering::Relaxed),
        errors_total: HTTP_METRICS.errors_total.load(Ordering::Relaxed),
        status_1xx_total: HTTP_METRICS.status_1xx_total.load(Ordering::Relaxed),
        status_2xx_total: HTTP_METRICS.status_2xx_total.load(Ordering::Relaxed),
        status_3xx_total: HTTP_METRICS.status_3xx_total.load(Ordering::Relaxed),
        status_4xx_total: HTTP_METRICS.status_4xx_total.load(Ordering::Relaxed),
        status_5xx_total: HTTP_METRICS.status_5xx_total.load(Ordering::Relaxed),
        latency_micros_sum: HTTP_METRICS.latency_micros_sum.load(Ordering::Relaxed),
        latency_ms_count: HTTP_METRICS.latency_ms_count.load(Ordering::Relaxed),
    }
}

fn record_http_metrics(status_code: u16, latency: Duration) {
    let latency_micros = latency.as_micros().min(u128::from(u64::MAX)) as u64;

    HTTP_METRICS.requests_total.fetch_add(1, Ordering::Relaxed);
    HTTP_METRICS
        .latency_micros_sum
        .fetch_add(latency_micros, Ordering::Relaxed);
    HTTP_METRICS
        .latency_ms_count
        .fetch_add(1, Ordering::Relaxed);

    if status_code >= 400 {
        HTTP_METRICS.errors_total.fetch_add(1, Ordering::Relaxed);
    }

    match status_code {
        100..=199 => {
            HTTP_METRICS
                .status_1xx_total
                .fetch_add(1, Ordering::Relaxed);
        }
        200..=299 => {
            HTTP_METRICS
                .status_2xx_total
                .fetch_add(1, Ordering::Relaxed);
        }
        300..=399 => {
            HTTP_METRICS
                .status_3xx_total
                .fetch_add(1, Ordering::Relaxed);
        }
        400..=499 => {
            HTTP_METRICS
                .status_4xx_total
                .fetch_add(1, Ordering::Relaxed);
        }
        500..=599 => {
            HTTP_METRICS
                .status_5xx_total
                .fetch_add(1, Ordering::Relaxed);
        }
        _ => {}
    }
}

fn record_and_log_http_metrics(method: &str, path: &str, status_code: u16, start_time: Instant) {
    let response_time = start_time.elapsed();
    record_http_metrics(status_code, response_time);

    info!(
        "{} {} -> {} in {:?}",
        method, path, status_code, response_time
    );
}

struct ResponseMetricsRecorder {
    method: String,
    path: String,
    status_code: u16,
    start_time: Instant,
}

impl ResponseMetricsRecorder {
    fn record(self) {
        record_and_log_http_metrics(&self.method, &self.path, self.status_code, self.start_time);
    }
}

pin_project! {
    pub struct MetricsResponseBody<B> {
        #[pin]
        body: B,
        recorder: Option<ResponseMetricsRecorder>,
    }

    impl<B> PinnedDrop for MetricsResponseBody<B> {
        fn drop(this: Pin<&mut Self>) {
            let this = this.project();
            if let Some(recorder) = this.recorder.take() {
                recorder.record();
            }
        }
    }
}

impl<B> MessageBody for MetricsResponseBody<B>
where
    B: MessageBody,
{
    type Error = B::Error;

    fn size(&self) -> BodySize {
        self.body.size()
    }

    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Bytes, Self::Error>>> {
        let this = self.project();

        match this.body.poll_next(cx) {
            Poll::Ready(None) => {
                if let Some(recorder) = this.recorder.take() {
                    recorder.record();
                }
                Poll::Ready(None)
            }
            other => other,
        }
    }
}

#[cfg(test)]
pub(crate) fn reset_http_metrics_for_tests() {
    HTTP_METRICS.requests_total.store(0, Ordering::Relaxed);
    HTTP_METRICS.errors_total.store(0, Ordering::Relaxed);
    HTTP_METRICS.status_1xx_total.store(0, Ordering::Relaxed);
    HTTP_METRICS.status_2xx_total.store(0, Ordering::Relaxed);
    HTTP_METRICS.status_3xx_total.store(0, Ordering::Relaxed);
    HTTP_METRICS.status_4xx_total.store(0, Ordering::Relaxed);
    HTTP_METRICS.status_5xx_total.store(0, Ordering::Relaxed);
    HTTP_METRICS.latency_micros_sum.store(0, Ordering::Relaxed);
    HTTP_METRICS.latency_ms_count.store(0, Ordering::Relaxed);
}

impl<S, B> Service<ServiceRequest> for MetricsMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<MetricsResponseBody<B>>;
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let start_time = Instant::now();
        let should_record = should_record_request_path(req.path());
        let request_summary =
            should_record.then(|| (req.method().to_string(), req.path().to_string()));

        let fut = self.service.call(req);

        Box::pin(async move {
            let res = match fut.await {
                Ok(res) => res,
                Err(err) => {
                    if let Some((method, path)) = request_summary {
                        let status_code = err.as_response_error().status_code().as_u16();
                        record_and_log_http_metrics(&method, &path, status_code, start_time);
                    }
                    return Err(err);
                }
            };

            let recorder = request_summary.map(|(method, path)| ResponseMetricsRecorder {
                method,
                path,
                status_code: res.status().as_u16(),
                start_time,
            });

            Ok(res.map_body(|_, body| MetricsResponseBody { body, recorder }))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{App, HttpResponse, http::StatusCode, test, web};
    use bytes::Bytes;

    #[actix_web::test]
    async fn middleware_records_status_classes_and_rendered_output() {
        let _metrics_guard = MetricsMiddleware::test_lock().await;
        MetricsMiddleware::reset_for_tests();
        let app = test::init_service(
            App::new()
                .wrap(MetricsMiddleware)
                .route("/ok", web::get().to(HttpResponse::Ok))
                .route("/missing", web::get().to(HttpResponse::NotFound))
                .route(
                    "/boom",
                    web::get().to(|| async {
                        Err::<HttpResponse, _>(actix_web::error::ErrorBadRequest("bad"))
                    }),
                ),
        )
        .await;

        let ok_req = test::TestRequest::get().uri("/ok").to_request();
        let ok_resp = test::call_service(&app, ok_req).await;
        assert_eq!(ok_resp.status(), StatusCode::OK);
        drop(test::read_body(ok_resp).await);

        let missing_req = test::TestRequest::get().uri("/missing").to_request();
        let missing_resp = test::call_service(&app, missing_req).await;
        assert_eq!(missing_resp.status(), StatusCode::NOT_FOUND);
        drop(test::read_body(missing_resp).await);

        let boom_req = test::TestRequest::get().uri("/boom").to_request();
        let boom_resp = test::call_service(&app, boom_req).await;
        assert_eq!(boom_resp.status(), StatusCode::BAD_REQUEST);
        drop(test::read_body(boom_resp).await);

        let snapshot = http_metrics_snapshot();
        assert_eq!(snapshot.requests_total, 3);
        assert_eq!(snapshot.errors_total, 2);
        assert_eq!(snapshot.status_2xx_total, 1);
        assert_eq!(snapshot.status_4xx_total, 2);
        assert_eq!(snapshot.latency_ms_count, 3);

        let rendered = MetricsMiddleware::render_prometheus();
        assert!(rendered.contains("gateway_http_requests_total 3"));
        assert!(rendered.contains("gateway_http_request_errors_total 2"));
        assert!(rendered.contains("gateway_http_responses_total{class=\"2xx\"} 1"));
        assert!(rendered.contains("gateway_http_responses_total{class=\"4xx\"} 2"));
    }

    #[actix_web::test]
    async fn middleware_records_streaming_response_after_body_completion() {
        let _metrics_guard = MetricsMiddleware::test_lock().await;
        MetricsMiddleware::reset_for_tests();
        let app = test::init_service(App::new().wrap(MetricsMiddleware).route(
            "/stream",
            web::get().to(|| async {
                let stream = futures::stream::once(async {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Ok::<_, actix_web::Error>(Bytes::from_static(b"chunk"))
                });
                HttpResponse::Ok().streaming(stream)
            }),
        ))
        .await;

        let req = test::TestRequest::get().uri("/stream").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(http_metrics_snapshot().requests_total, 0);

        let body = test::read_body(resp).await;
        assert_eq!(body, Bytes::from_static(b"chunk"));

        let snapshot = http_metrics_snapshot();
        assert_eq!(snapshot.requests_total, 1);
        assert_eq!(snapshot.status_2xx_total, 1);
        assert_eq!(snapshot.latency_ms_count, 1);
        assert!(snapshot.latency_micros_sum >= 10_000);
    }

    #[actix_web::test]
    async fn middleware_does_not_record_metrics_scrapes() {
        let _metrics_guard = MetricsMiddleware::test_lock().await;
        MetricsMiddleware::reset_for_tests();
        let app = test::init_service(App::new().wrap(MetricsMiddleware).route(
            "/metrics",
            web::get().to(|| async { HttpResponse::Ok().body("metrics") }),
        ))
        .await;

        let req = test::TestRequest::get().uri("/metrics").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let body = test::read_body(resp).await;
        assert_eq!(body, Bytes::from_static(b"metrics"));

        assert_eq!(http_metrics_snapshot().requests_total, 0);
    }
}
