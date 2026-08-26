use super::http::HttpServer;
use super::http_listener::validated_listener_settings;
use super::tls::{build_tls_server, load_listener_tls};
use crate::config::models::server::{ServerConfig, TlsConfig};
use actix_web::web;
use rcgen::generate_simple_self_signed;
use rustls::pki_types::{CertificateDer, pem::PemObject};
use std::io::{Read as _, Write as _};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

struct TlsMaterial {
    _directory: TempDir,
    config: TlsConfig,
    trust_anchor: CertificateDer<'static>,
}

impl TlsMaterial {
    fn new() -> Self {
        let directory = TempDir::new().expect("temporary TLS directory");
        let certified =
            generate_simple_self_signed(["localhost".to_owned()]).expect("self-signed certificate");
        let cert_file = directory.path().join("cert.pem");
        let key_file = directory.path().join("key.pem");
        std::fs::write(&cert_file, certified.cert.pem()).expect("write certificate");
        std::fs::write(&key_file, certified.signing_key.serialize_pem()).expect("write key");
        let trust_anchor = CertificateDer::pem_file_iter(&cert_file)
            .expect("read certificate PEM")
            .next()
            .expect("certificate PEM entry")
            .expect("parse certificate PEM");
        Self {
            _directory: directory,
            config: TlsConfig {
                cert_file: cert_file.to_string_lossy().into_owned(),
                key_file: key_file.to_string_lossy().into_owned(),
                ca_file: None,
                require_client_cert: false,
                http2: false,
            },
            trust_anchor,
        }
    }
}

struct RunningTlsServer {
    address: std::net::SocketAddr,
    handle: actix_web::dev::ServerHandle,
    task: Option<tokio::task::JoinHandle<std::io::Result<()>>>,
}

impl RunningTlsServer {
    async fn start(server_config: ServerConfig) -> Self {
        let listener_tls = load_listener_tls(&server_config)
            .expect("TLS configuration should load")
            .expect("TLS should be enabled");
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
        let (server, address) = build_tls_server(
            web::Data::new(gateway.state().clone()),
            &settings,
            "127.0.0.1:0",
            listener_tls,
        )
        .expect("production TLS builder should bind");
        let handle = server.handle();
        let task = tokio::spawn(server);
        Self {
            address,
            handle,
            task: Some(task),
        }
    }

    async fn stop(mut self) {
        self.handle.stop(true).await;
        self.task
            .take()
            .expect("TLS server task")
            .await
            .expect("TLS server task should join")
            .expect("TLS server should stop cleanly");
    }
}

impl Drop for RunningTlsServer {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn client_config(trust_anchor: CertificateDer<'static>) -> rustls::ClientConfig {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(trust_anchor)
        .expect("trust self-signed certificate");
    let provider = rustls::crypto::ring::default_provider();
    let mut config = rustls::ClientConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()
        .expect("safe client protocol versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    config
}

#[actix_web::test]
async fn production_tls_builder_applies_first_request_head_timeout() {
    let material = TlsMaterial::new();
    let running = RunningTlsServer::start(ServerConfig {
        workers: Some(1),
        timeout: 1,
        tls: Some(material.config.clone()),
        ..ServerConfig::default()
    })
    .await;
    let address = running.address;
    let client = client_config(material.trust_anchor.clone());

    let response = tokio::task::spawn_blocking(move || {
        let connection = rustls::ClientConnection::new(
            Arc::new(client),
            "localhost".try_into().expect("server name"),
        )
        .expect("TLS client connection");
        let socket = std::net::TcpStream::connect(address).expect("connect TLS listener");
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");
        let mut stream = rustls::StreamOwned::new(connection, socket);
        stream
            .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n")
            .expect("write partial request head");
        let mut response = String::new();
        stream
            .read_to_string(&mut response)
            .expect("read timeout response");
        response
    })
    .await
    .expect("TLS client task should join");

    assert!(response.contains(" 408 "));
    running.stop().await;
}

#[actix_web::test]
async fn production_tls_builder_enforces_server_wide_connection_cap() {
    let material = TlsMaterial::new();
    let running = RunningTlsServer::start(ServerConfig {
        workers: Some(4),
        max_connections: Some(2),
        timeout: 10,
        tls: Some(material.config.clone()),
        ..ServerConfig::default()
    })
    .await;

    let first = tokio::net::TcpStream::connect(running.address)
        .await
        .expect("first stalled connection");
    let second = tokio::net::TcpStream::connect(running.address)
        .await
        .expect("second stalled connection");
    tokio::time::sleep(Duration::from_millis(250)).await;

    let client = client_config(material.trust_anchor);
    let mut connection = rustls::ClientConnection::new(
        Arc::new(client),
        "localhost".try_into().expect("server name"),
    )
    .expect("TLS client connection");
    let mut client_hello = Vec::new();
    connection
        .write_tls(&mut client_hello)
        .expect("serialize TLS client hello");
    let mut queued = tokio::net::TcpStream::connect(running.address)
        .await
        .expect("queued connection should reach backlog");
    queued
        .write_all(&client_hello)
        .await
        .expect("write queued client hello");
    let mut response = [0_u8; 4096];
    assert!(
        tokio::time::timeout(Duration::from_millis(500), queued.read(&mut response))
            .await
            .is_err(),
        "third connection must not be accepted while the total cap is occupied"
    );

    drop(first);
    drop(second);
    let bytes = tokio::time::timeout(Duration::from_secs(5), queued.read(&mut response))
        .await
        .expect("queued TLS connection should resume")
        .expect("queued TLS response should be readable");
    assert!(
        bytes > 0,
        "server should emit a TLS handshake after capacity frees"
    );
    running.stop().await;
}
