use super::http::HttpServer;
use super::http_listener::{build_actix_server, validated_listener_settings};
use super::middleware::MetricsMiddleware;
use crate::config::models::server::ServerConfig;
use actix_web::{
    http::{StatusCode, header},
    test as actix_test, web,
};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

struct RunningServer {
    address: std::net::SocketAddr,
    handle: actix_web::dev::ServerHandle,
    task: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
}

impl RunningServer {
    async fn start(server_config: ServerConfig) -> Self {
        let mut config = super::valid_test_config();
        config.gateway.server = server_config;
        config.gateway.auth.enable_jwt = false;
        config.gateway.auth.enable_api_key = false;
        config.gateway.auth.allow_anonymous = true;
        config.gateway.storage.database.enabled = false;
        config.gateway.storage.redis.enabled = false;
        config.gateway.pricing.source = None;
        config.gateway.monitoring.metrics.enabled = false;

        let gateway = HttpServer::new(&config)
            .await
            .expect("production server state should initialize");
        let settings = validated_listener_settings(gateway.config())
            .expect("listener settings should validate");
        let (server, address) = build_actix_server(
            web::Data::new(gateway.state().clone()),
            &settings,
            "127.0.0.1:0",
        )
        .expect("production Actix builder should bind");
        let handle = server.handle();
        let task = tokio::spawn(server);
        wait_until_ready(address).await;
        Self {
            address,
            handle,
            task: Some(task),
        }
    }

    async fn stop(mut self) {
        self.handle.stop(true).await;
        let result = self
            .task
            .take()
            .expect("server task should be present")
            .await
            .expect("server task should join");
        result.expect("server should stop cleanly");
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

async fn wait_until_ready(address: std::net::SocketAddr) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(mut stream) = tokio::net::TcpStream::connect(address).await
            && stream
                .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .await
                .is_ok()
        {
            let mut response = Vec::new();
            if let Ok(Ok(_)) =
                tokio::time::timeout(Duration::from_secs(1), stream.read_to_end(&mut response))
                    .await
                && String::from_utf8_lossy(&response).contains(" 200 ")
            {
                return;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "production-path server did not become ready at {address}"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

async fn open_keep_alive(address: std::net::SocketAddr) -> tokio::net::TcpStream {
    let mut stream = tokio::net::TcpStream::connect(address)
        .await
        .expect("keep-alive client should connect");
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .expect("keep-alive client should write a request");
    let mut response = vec![0_u8; 2048];
    let bytes = tokio::time::timeout(Duration::from_secs(2), stream.read(&mut response))
        .await
        .expect("health request should complete")
        .expect("health response should be readable");
    assert!(String::from_utf8_lossy(&response[..bytes]).contains(" 200 "));
    stream
}

#[test]
fn listener_settings_apply_workers_timeout_and_total_cap() {
    let uncapped = validated_listener_settings(&ServerConfig {
        workers: Some(4),
        max_connections: None,
        timeout: 17,
        ..ServerConfig::default()
    })
    .expect("uncapped settings should validate");
    assert_eq!(uncapped.configured_workers, 4);
    assert_eq!(uncapped.effective_workers, 4);
    assert_eq!(uncapped.first_request_head_timeout, Duration::from_secs(17));
    assert_eq!(uncapped.max_connections_per_worker, None);

    let capped = validated_listener_settings(&ServerConfig {
        workers: Some(4),
        max_connections: Some(10),
        ..ServerConfig::default()
    })
    .expect("capped settings should validate");
    let per_worker = capped
        .max_connections_per_worker
        .expect("cap should produce a per-worker limit");
    assert_eq!(capped.effective_workers, 4);
    assert_eq!(per_worker * capped.effective_workers, 8);
    assert!(per_worker * capped.effective_workers <= 10);
}

#[tokio::test]
async fn production_builder_applies_first_request_head_timeout() {
    let running = RunningServer::start(ServerConfig {
        workers: Some(1),
        timeout: 1,
        ..ServerConfig::default()
    })
    .await;
    let mut stream = tokio::net::TcpStream::connect(running.address)
        .await
        .expect("test client should connect");
    stream
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n")
        .await
        .expect("partial request head should be written");
    let mut response = vec![0_u8; 1024];
    let bytes = tokio::time::timeout(Duration::from_secs(3), stream.read(&mut response))
        .await
        .expect("request-head timeout should fire")
        .expect("timeout response should be readable");
    assert!(String::from_utf8_lossy(&response[..bytes]).contains(" 408 "));
    running.stop().await;
}

#[tokio::test]
async fn production_builder_enforces_server_wide_connection_cap() {
    let running = RunningServer::start(ServerConfig {
        workers: Some(2),
        max_connections: Some(4),
        timeout: 10,
        ..ServerConfig::default()
    })
    .await;
    let mut occupied = Vec::new();
    for _ in 0..4 {
        occupied.push(open_keep_alive(running.address).await);
    }
    tokio::time::sleep(Duration::from_millis(250)).await;

    let mut queued = tokio::net::TcpStream::connect(running.address)
        .await
        .expect("queued client should reach the backlog");
    queued
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .await
        .expect("queued request should be written");
    let mut response = vec![0_u8; 2048];
    assert!(
        tokio::time::timeout(Duration::from_secs(1), queued.read(&mut response))
            .await
            .is_err()
    );

    drop(occupied);
    let bytes = tokio::time::timeout(Duration::from_secs(3), queued.read(&mut response))
        .await
        .expect("queued request should resume")
        .expect("queued response should be readable");
    assert!(String::from_utf8_lossy(&response[..bytes]).contains(" 200 "));
    running.stop().await;
}

fn body_limit_config() -> crate::config::Config {
    let mut config = super::valid_test_config();
    config.gateway.auth.enable_jwt = false;
    config.gateway.auth.enable_api_key = false;
    config.gateway.auth.allow_anonymous = true;
    config.gateway.storage.database.enabled = false;
    config.gateway.storage.redis.enabled = false;
    config.gateway.pricing.source = None;
    config.gateway.server.max_body_size = 1024;
    config.gateway.monitoring.metrics.enabled = false;
    config
}

#[tokio::test]
async fn app_factory_enforces_configured_json_body_limits() {
    let _metrics_guard = MetricsMiddleware::test_lock().await;
    let config = body_limit_config();
    let server = HttpServer::new(&config)
        .await
        .expect("body-limit server should initialize");
    let app = actix_test::init_service(HttpServer::create_app(web::Data::new(
        server.state().clone(),
    )))
    .await;

    let oversized = format!(
        r#"{{"model":"test-model","messages":[{{"role":"user","content":"{}"}}]}}"#,
        "x".repeat(4096)
    );
    let request = actix_test::TestRequest::post()
        .uri("/v1/chat/completions")
        .insert_header((header::CONTENT_TYPE, "application/json"))
        .set_payload(oversized)
        .to_request();
    let response = actix_test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = actix_test::read_body(response).await;
    assert!(String::from_utf8_lossy(&body).contains("Invalid JSON request body"));

    let request = actix_test::TestRequest::post()
        .uri("/auth/login")
        .set_json(serde_json::json!({
            "username": "body-limit-user",
            "password": "x".repeat(4096),
        }))
        .to_request();
    let response = actix_test::call_service(&app, request).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
