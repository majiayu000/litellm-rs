//! TLS configuration and listener support.

use crate::config::models::server::{ServerConfig, TlsConfig};
use crate::server::{HttpServer, state::AppState};
use crate::utils::error::gateway_error::{GatewayError, Result};
use actix_http::{HttpService, Protocol};
use actix_server::Server;
use actix_service::{IntoServiceFactory, ServiceFactoryExt as _, map_config};
use actix_tls::accept::{
    TlsError,
    rustls_0_23::{Acceptor as RustlsAcceptor, TlsStream},
};
use actix_web::{HttpServer as ActixHttpServer, dev::AppConfig, web};
use std::{
    convert::Infallible,
    io,
    net::{SocketAddr, ToSocketAddrs},
};

/// Validated TLS state cached before gateway dependencies are initialized.
pub(crate) struct ListenerTls {
    config: rustls::ServerConfig,
    http2: bool,
}

pub(crate) fn load_listener_tls(server: &ServerConfig) -> Result<Option<ListenerTls>> {
    server
        .tls
        .as_ref()
        .map(|tls| {
            Ok(ListenerTls {
                config: load_rustls_config(tls)?,
                http2: tls.http2,
            })
        })
        .transpose()
}

/// Bind the configured HTTP or HTTPS listener.
pub(crate) fn bind_server(
    bind_addr: &str,
    server: &ServerConfig,
    state: web::Data<AppState>,
    tls: Option<ListenerTls>,
) -> Result<Server> {
    let map_bind_error = |error| HttpServer::format_bind_error(error, bind_addr, server.port);
    let Some(tls) = tls else {
        return ActixHttpServer::new(move || HttpServer::create_app(state.clone()))
            .bind(bind_addr)
            .map(ActixHttpServer::run)
            .map_err(map_bind_error);
    };

    if tls.http2 {
        bind_http1_and2(bind_addr, state, tls.config).map_err(map_bind_error)
    } else {
        bind_http1_only(bind_addr, state, tls.config).map_err(map_bind_error)
    }
}

/// Build a rustls configuration without relying on a process-global provider.
pub(crate) fn load_rustls_config(tls: &TlsConfig) -> Result<rustls::ServerConfig> {
    crate::config::tls::build_rustls_config(tls).map_err(GatewayError::validation)
}

fn bind_http1_and2(
    bind_addr: &str,
    state: web::Data<AppState>,
    config: rustls::ServerConfig,
) -> std::io::Result<Server> {
    let mut last_error = None;
    for address in bind_addr.to_socket_addrs()? {
        let state = state.clone();
        match ActixHttpServer::new(move || HttpServer::create_app(state.clone()))
            .bind_rustls_0_23(address, config.clone())
        {
            Ok(builder) => return Ok(builder.run()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::other("Could not bind to address")))
}

fn bind_http1_only(
    bind_addr: &str,
    state: web::Data<AppState>,
    config: rustls::ServerConfig,
) -> std::io::Result<Server> {
    let mut last_error = None;
    for address in bind_addr.to_socket_addrs()? {
        let app_config = secure_app_config(address);
        let state = state.clone();
        let config = config.clone();
        let builder =
            Server::build()
                .backlog(1024)
                .bind("actix-web-https-http1", address, move || {
                    let factory = HttpServer::create_app(state.clone())
                        .into_factory()
                        .map_err(|error: actix_web::Error| error.error_response());
                    let app_config = app_config.clone();
                    let http = HttpService::build()
                        .client_disconnect_timeout(std::time::Duration::from_secs(1))
                        .finish(map_config(factory, move |_| app_config.clone()));

                    http1_tls_transport(config.clone())
                        .map_err(TlsError::into_service_error)
                        .and_then(http.map_err(TlsError::Service))
                });
        match builder {
            Ok(builder) => return Ok(builder.run()),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::other("Could not bind to address")))
}

fn http1_tls_transport(
    config: rustls::ServerConfig,
) -> impl actix_service::ServiceFactory<
    actix_web::rt::net::TcpStream,
    Config = (),
    Response = (
        TlsStream<actix_web::rt::net::TcpStream>,
        Protocol,
        Option<SocketAddr>,
    ),
    Error = TlsError<io::Error, Infallible>,
    InitError = (),
> + Clone {
    RustlsAcceptor::new(config).map(|io: TlsStream<actix_web::rt::net::TcpStream>| {
        let peer_addr = io.get_ref().0.peer_addr().ok();
        (io, Protocol::Http1, peer_addr)
    })
}

// Actix has no public secure AppConfig constructor. Keep its semver-exempt helper isolated; the
// exact actix-web version is pinned in Cargo.toml and the loopback test covers this listener path.
fn secure_app_config(address: SocketAddr) -> AppConfig {
    AppConfig::__priv_test_new(true, address.to_string(), address)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::tls::validate_rustls_config;
    use actix_service::fn_service;
    use rcgen::generate_simple_self_signed;
    use std::{
        fs,
        io::{Read as _, Write as _},
        net::TcpListener,
        sync::Arc,
        time::Duration,
    };
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};

    fn material() -> (TempDir, TlsConfig) {
        let directory = TempDir::new().expect("temporary directory");
        let certified =
            generate_simple_self_signed(["localhost".to_owned()]).expect("self-signed certificate");
        let cert_file = directory.path().join("cert.pem");
        let key_file = directory.path().join("key.pem");
        fs::write(&cert_file, certified.cert.pem()).expect("write certificate");
        fs::write(&key_file, certified.signing_key.serialize_pem()).expect("write key");
        let tls = TlsConfig {
            cert_file: cert_file.to_string_lossy().into_owned(),
            key_file: key_file.to_string_lossy().into_owned(),
            ca_file: None,
            require_client_cert: false,
            http2: false,
        };
        (directory, tls)
    }

    #[test]
    fn custom_listener_marks_app_config_secure() {
        let address: SocketAddr = "127.0.0.1:8443".parse().expect("socket address");
        let config = secure_app_config(address);
        assert!(config.secure());
        assert_eq!(config.host(), address.to_string());
        assert_eq!(config.local_addr(), address);
    }

    #[test]
    fn valid_material_builds_without_global_provider() {
        let (_directory, tls) = material();
        tls.validate()
            .expect("validate-config accepts valid TLS material");
        let config = load_rustls_config(&tls).expect("valid TLS material");
        assert_eq!(config.alpn_protocols, vec![b"http/1.1".to_vec()]);
    }

    #[test]
    fn http2_controls_configured_alpn() {
        let (_directory, mut tls) = material();
        tls.http2 = true;
        let config = load_rustls_config(&tls).expect("valid TLS material");
        assert_eq!(
            config.alpn_protocols,
            vec![b"h2".to_vec(), b"http/1.1".to_vec()]
        );
    }

    #[test]
    fn rejects_empty_and_malformed_pem() {
        let (_directory, tls) = material();
        fs::write(&tls.cert_file, []).expect("empty certificate");
        assert!(
            validate_rustls_config(&tls)
                .unwrap_err()
                .contains("no TLS certificates")
        );
        fs::write(
            &tls.cert_file,
            "-----BEGIN CERTIFICATE-----\nbad\n-----END CERTIFICATE-----",
        )
        .expect("malformed certificate");
        assert!(validate_rustls_config(&tls).is_err());
    }

    #[test]
    fn rejects_non_certificate_block_in_cert_file() {
        let (_directory, tls) = material();
        let key = fs::read_to_string(&tls.key_file).expect("read key");
        fs::write(&tls.cert_file, key).expect("write key into cert file");
        assert!(
            tls.validate()
                .unwrap_err()
                .contains("unsupported PEM block")
        );
    }

    #[test]
    fn rejects_invalid_der_later_in_certificate_chain() {
        let (_directory, tls) = material();
        let cert = fs::read_to_string(&tls.cert_file).expect("read cert");
        fs::write(
            &tls.cert_file,
            format!("{cert}\n-----BEGIN CERTIFICATE-----\nAQID\n-----END CERTIFICATE-----\n"),
        )
        .expect("write invalid intermediate");
        assert!(
            tls.validate()
                .unwrap_err()
                .contains("invalid TLS certificate 1")
        );
    }

    #[test]
    fn rejects_mismatched_certificate_and_key() {
        let (_directory, tls) = material();
        let other = generate_simple_self_signed(["localhost".to_owned()])
            .expect("second self-signed certificate");
        fs::write(&tls.key_file, other.signing_key.serialize_pem()).expect("write mismatched key");
        assert!(
            validate_rustls_config(&tls)
                .unwrap_err()
                .contains("certificate/key pair")
        );
    }

    #[test]
    fn rejects_unsupported_client_cert_auth_before_startup() {
        let (_directory, mut tls) = material();
        tls.require_client_cert = true;
        assert!(
            validate_rustls_config(&tls)
                .unwrap_err()
                .contains("require_client_cert")
        );
    }

    #[test]
    fn rejects_ca_file_while_client_auth_is_unsupported() {
        let (_directory, mut tls) = material();
        tls.ca_file = Some(tls.cert_file.clone());
        assert!(tls.validate().unwrap_err().contains("tls.ca_file"));
    }

    #[test]
    fn rejects_empty_malformed_multiple_and_trailing_bad_keys() {
        let (_directory, tls) = material();
        let key = fs::read_to_string(&tls.key_file).expect("read key");

        fs::write(&tls.key_file, []).expect("empty key");
        assert!(
            tls.validate()
                .unwrap_err()
                .contains("no unencrypted private key")
        );

        fs::write(
            &tls.key_file,
            "-----BEGIN PRIVATE KEY-----\nbad\n-----END PRIVATE KEY-----",
        )
        .expect("malformed key");
        assert!(tls.validate().is_err());

        fs::write(&tls.key_file, format!("{key}\n{key}")).expect("multiple keys");
        assert!(
            tls.validate()
                .unwrap_err()
                .contains("multiple private keys")
        );

        fs::write(
            &tls.key_file,
            format!("{key}\n-----BEGIN PRIVATE KEY-----\nbad"),
        )
        .expect("trailing malformed key");
        assert!(tls.validate().unwrap_err().contains("invalid TLS key"));
    }

    #[test]
    fn rejects_encrypted_only_and_mixed_key_files() {
        let (_directory, tls) = material();
        let key = fs::read_to_string(&tls.key_file).expect("read key");
        let encrypted =
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\nbad\n-----END ENCRYPTED PRIVATE KEY-----";

        fs::write(&tls.key_file, encrypted).expect("encrypted key");
        assert!(
            tls.validate()
                .unwrap_err()
                .contains("ENCRYPTED PRIVATE KEY")
        );

        fs::write(&tls.key_file, format!("{encrypted}\n{key}")).expect("mixed keys");
        assert!(
            tls.validate()
                .unwrap_err()
                .contains("ENCRYPTED PRIVATE KEY")
        );
    }

    #[actix_web::test]
    async fn http1_only_transport_negotiates_http1_and_stops_cleanly() {
        let (_directory, tls) = material();
        let trust_anchor = crate::config::tls::load_certs(&tls.cert_file)
            .expect("certificate")
            .remove(0);
        let config = load_rustls_config(&tls).expect("server config");
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let address = listener.local_addr().expect("listener address");
        let server = Server::build()
            .listen("tls-http1-test", listener, move || {
                http1_tls_transport(config.clone()).and_then(fn_service(
                    |(mut stream, protocol, _): (
                        TlsStream<actix_web::rt::net::TcpStream>,
                        Protocol,
                        Option<SocketAddr>,
                    )| async move {
                        if protocol != Protocol::Http1 {
                            return Err(TlsError::Tls(io::Error::other(
                                "listener selected a protocol other than HTTP/1",
                            )));
                        }
                        let mut request = [0_u8; 1024];
                        stream.read(&mut request).await.map_err(TlsError::Tls)?;
                        stream
                            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                            .await
                            .map_err(TlsError::Tls)?;
                        stream.shutdown().await.map_err(TlsError::Tls)?;
                        Ok::<_, TlsError<io::Error, Infallible>>(())
                    },
                ))
            })
            .expect("register listener")
            .workers(1)
            .run();
        let handle = server.handle();
        let task = tokio::spawn(server);

        let client_task = tokio::task::spawn_blocking(move || {
            let mut roots = rustls::RootCertStore::empty();
            roots.add(trust_anchor).expect("trust self-signed cert");
            let provider = rustls::crypto::ring::default_provider();
            let mut client = rustls::ClientConfig::builder_with_provider(provider.into())
                .with_safe_default_protocol_versions()
                .expect("safe client protocol versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
            client.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
            let connection = rustls::ClientConnection::new(
                Arc::new(client),
                "localhost".try_into().expect("server name"),
            )
            .expect("client connection");
            let socket = std::net::TcpStream::connect(address).expect("connect loopback");
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("read timeout");
            socket
                .set_write_timeout(Some(Duration::from_secs(5)))
                .expect("write timeout");
            let mut stream = rustls::StreamOwned::new(connection, socket);
            stream
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .expect("write request");
            let mut response = String::new();
            stream.read_to_string(&mut response).expect("read response");
            (stream.conn.alpn_protocol().map(<[u8]>::to_vec), response)
        });
        let client_result = tokio::time::timeout(Duration::from_secs(5), client_task).await;

        handle.stop(true).await;
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("server stopped before timeout")
            .expect("server task joined")
            .expect("server exited cleanly");
        let (negotiated, response) = client_result
            .expect("client completed before timeout")
            .expect("client task joined");
        assert_eq!(negotiated.as_deref(), Some(b"http/1.1".as_slice()));
        assert!(response.ends_with("\r\n\r\nok"));
    }
}
