//! TLS configuration and listener support.

use crate::config::models::server::ServerConfig;
use crate::server::http_listener::ListenerSettings;
use crate::server::{HttpServer, state::AppState};
use crate::utils::error::gateway_error::Result;
use crate::utils::tls::load_rustls_config;
use actix_http::{HttpService, Protocol};
use actix_server::Server;
use actix_service::{IntoServiceFactory, ServiceFactoryExt as _, fn_service, map_config};
use actix_tls::accept::{
    TlsError,
    rustls_0_23::{Acceptor as RustlsAcceptor, TlsStream},
};
use actix_web::{dev::AppConfig, web};
use socket2::{Domain, Protocol as SocketProtocol, Socket, Type};
use std::{
    convert::Infallible,
    io,
    net::{SocketAddr, TcpListener, ToSocketAddrs},
    time::Duration,
};

const TLS_BACKLOG: u32 = 1024;
const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(3);

/// Validated TLS state cached before gateway dependencies are initialized.
pub(super) struct ListenerTls {
    config: rustls::ServerConfig,
}

pub(super) fn load_listener_tls(server: &ServerConfig) -> Result<Option<ListenerTls>> {
    server
        .tls
        .as_ref()
        .map(|tls| load_rustls_config(tls).map(|config| ListenerTls { config }))
        .transpose()
}

/// Bind one HTTPS listener with the same worker, capacity, and request-head
/// settings used by the plain HTTP listener.
pub(super) fn build_tls_server(
    state: web::Data<AppState>,
    settings: &ListenerSettings,
    addresses: impl ToSocketAddrs,
    tls: ListenerTls,
) -> io::Result<(Server, SocketAddr)> {
    bind_http1_only(addresses, state, tls.config, settings)
}

fn bind_http1_only(
    addresses: impl ToSocketAddrs,
    state: web::Data<AppState>,
    config: rustls::ServerConfig,
    settings: &ListenerSettings,
) -> std::io::Result<(Server, SocketAddr)> {
    let (listener, address) = bind_first_resolved_listener(addresses, TLS_BACKLOG)?;
    let app_config = secure_app_config(address);
    let request_timeout = settings.first_request_head_timeout;
    let mut builder = Server::build()
        .workers(settings.effective_workers)
        .backlog(TLS_BACKLOG);
    if let Some(per_worker) = settings.max_connections_per_worker {
        builder = builder.max_concurrent_connections(per_worker);
    }
    let builder = builder.listen("actix-web-https-http1", listener, move || {
        let factory = HttpServer::create_app(state.clone())
            .into_factory()
            .map_err(|error: actix_web::Error| error.error_response());
        let app_config = app_config.clone();
        let http = HttpService::build()
            .client_request_timeout(request_timeout)
            .client_disconnect_timeout(std::time::Duration::from_secs(1))
            .finish(map_config(factory, move |_| app_config.clone()));

        http1_tls_transport(config.clone())
            .map_err(TlsError::into_service_error)
            .and_then(http.map_err(TlsError::Service))
    })?;
    Ok((builder.run(), address))
}

fn bind_first_resolved_listener(
    addrs: impl ToSocketAddrs,
    backlog: u32,
) -> io::Result<(TcpListener, SocketAddr)> {
    let mut last_error = None;
    for address in addrs.to_socket_addrs()? {
        match create_tcp_listener(address, backlog) {
            Ok(listener) => {
                let selected_address = listener.local_addr()?;
                return Ok((listener, selected_address));
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::other("Could not bind to address")))
}

fn create_tcp_listener(address: SocketAddr, backlog: u32) -> io::Result<TcpListener> {
    let socket = Socket::new(
        Domain::for_address(address),
        Type::STREAM,
        Some(SocketProtocol::TCP),
    )?;
    #[cfg(not(windows))]
    socket.set_reuse_address(true)?;
    socket.bind(&address.into())?;
    socket.listen(backlog.min(i32::MAX as u32) as i32)?;
    Ok(socket.into())
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
    http1_tls_acceptor(config).and_then(fn_service(
        |io: TlsStream<actix_web::rt::net::TcpStream>| async move {
            io.get_ref().0.set_nodelay(true).map_err(TlsError::Tls)?;
            let peer_addr = io.get_ref().0.peer_addr().ok();
            Ok((io, Protocol::Http1, peer_addr))
        },
    ))
}

fn http1_tls_acceptor(config: rustls::ServerConfig) -> RustlsAcceptor {
    // Actix's acceptor owns the standard per-worker TLS handshake guard. Its counter is shared
    // across every acceptor service created on that worker, so all resolved listeners contribute
    // to the same 256-handshake default instead of each listener receiving a separate allowance.
    let mut acceptor = RustlsAcceptor::new(config);
    acceptor.set_handshake_timeout(TLS_HANDSHAKE_TIMEOUT);
    acceptor
}

// Actix has no public secure AppConfig constructor. Keep its semver-exempt helper isolated; the
// exact actix-web version is pinned in Cargo.toml and the loopback test covers this listener path.
fn secure_app_config(address: SocketAddr) -> AppConfig {
    AppConfig::__priv_test_new(true, address.to_string(), address)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::models::server::TlsConfig;
    use crate::utils::tls::{load_certs, validate_rustls_config};
    use actix_service::{Service as _, ServiceFactory as _, fn_service};
    use rcgen::generate_simple_self_signed;
    use std::{
        fs,
        future::poll_fn,
        io::{Read as _, Write as _},
        net::TcpListener,
        sync::Arc,
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
    fn binds_first_available_resolved_address_and_stops() {
        let occupied = create_tcp_listener(
            "127.0.0.1:0".parse().expect("loopback address"),
            TLS_BACKLOG,
        )
        .expect("occupy loopback address");
        let unavailable = occupied.local_addr().expect("occupied address");
        let available = reserve_available_address();
        let unused = reserve_available_address();
        let candidates = [unavailable, available, unused];

        let (listener, selected) = bind_first_resolved_listener(&candidates[..], TLS_BACKLOG)
            .expect("available address should still bind");
        assert_eq!(selected, available);
        let unused_listener = TcpListener::bind(unused)
            .expect("later candidate must remain available after the first successful bind");
        drop(listener);
        drop(unused_listener);
    }

    fn reserve_available_address() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").expect("reserve candidate address");
        listener.local_addr().expect("candidate address")
    }

    #[test]
    fn custom_listener_marks_app_config_secure() {
        let address: SocketAddr = "127.0.0.1:8443".parse().expect("socket address");
        let config = secure_app_config(address);
        assert!(config.secure());
        assert_eq!(config.host(), address.to_string());
        assert_eq!(config.local_addr(), address);
    }

    async fn tcp_pair() -> (actix_web::rt::net::TcpStream, actix_web::rt::net::TcpStream) {
        let listener = actix_web::rt::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind async loopback listener");
        let address = listener.local_addr().expect("async listener address");
        let client = actix_web::rt::net::TcpStream::connect(address);
        let server = listener.accept();
        let (client, server) = tokio::join!(client, server);
        let (server, _) = server.expect("accept loopback connection");
        (server, client.expect("connect loopback client"))
    }

    #[actix_web::test]
    async fn http1_acceptor_times_out_stalled_handshake() {
        let (_directory, tls) = material();
        let config = load_rustls_config(&tls).expect("server config");
        let transport = http1_tls_transport(config)
            .new_service(())
            .await
            .expect("initialize HTTP/1 TLS transport");
        let (server, _stalled_client) = tcp_pair().await;
        let started = tokio::time::Instant::now();

        let result = tokio::time::timeout(
            TLS_HANDSHAKE_TIMEOUT + Duration::from_secs(1),
            transport.call(server),
        )
        .await
        .expect("Actix handshake timeout should complete the transport future");

        assert!(matches!(result, Err(TlsError::Timeout)));
        assert!(started.elapsed() >= TLS_HANDSHAKE_TIMEOUT);
    }

    #[actix_web::test]
    async fn http1_acceptors_share_actix_handshake_limit() {
        const ACTIX_DEFAULT_TLS_HANDSHAKE_LIMIT: usize = 256;

        let (_directory, tls) = material();
        let config = load_rustls_config(&tls).expect("server config");
        let first = http1_tls_transport(config.clone())
            .new_service(())
            .await
            .expect("initialize first transport");
        let second = http1_tls_transport(config)
            .new_service(())
            .await
            .expect("initialize second transport");
        let mut stalled = Vec::with_capacity(ACTIX_DEFAULT_TLS_HANDSHAKE_LIMIT);

        for _ in 0..ACTIX_DEFAULT_TLS_HANDSHAKE_LIMIT {
            poll_fn(|context| first.poll_ready(context))
                .await
                .expect("first transport ready below the Actix limit");
            let (server, client) = tcp_pair().await;
            stalled.push((first.call(server), client));
        }

        assert!(
            tokio::time::timeout(
                Duration::from_millis(50),
                poll_fn(|context| second.poll_ready(context)),
            )
            .await
            .is_err(),
            "a second listener factory must observe the shared saturated handshake counter"
        );

        drop(stalled.pop());
        tokio::time::timeout(
            Duration::from_secs(1),
            poll_fn(|context| second.poll_ready(context)),
        )
        .await
        .expect("dropping a stalled handshake should release shared capacity")
        .expect("second transport becomes ready after capacity is released");
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
    fn rejects_http2_before_loading_runtime_material() {
        let (_directory, mut tls) = material();
        tls.http2 = true;
        assert!(
            load_rustls_config(&tls)
                .unwrap_err()
                .to_string()
                .contains("tls.http2")
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
            validate_rustls_config(&tls)
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
            validate_rustls_config(&tls)
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
            validate_rustls_config(&tls)
                .unwrap_err()
                .contains("no unencrypted private key")
        );

        fs::write(
            &tls.key_file,
            "-----BEGIN PRIVATE KEY-----\nbad\n-----END PRIVATE KEY-----",
        )
        .expect("malformed key");
        assert!(validate_rustls_config(&tls).is_err());

        fs::write(&tls.key_file, format!("{key}\n{key}")).expect("multiple keys");
        assert!(
            validate_rustls_config(&tls)
                .unwrap_err()
                .contains("multiple private keys")
        );

        fs::write(
            &tls.key_file,
            format!("{key}\n-----BEGIN PRIVATE KEY-----\nbad"),
        )
        .expect("trailing malformed key");
        assert!(
            validate_rustls_config(&tls)
                .unwrap_err()
                .contains("invalid TLS key")
        );
    }

    #[test]
    fn rejects_encrypted_only_and_mixed_key_files() {
        let (_directory, tls) = material();
        let key = fs::read_to_string(&tls.key_file).expect("read key");
        let encrypted =
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\nbad\n-----END ENCRYPTED PRIVATE KEY-----";

        fs::write(&tls.key_file, encrypted).expect("encrypted key");
        assert!(
            validate_rustls_config(&tls)
                .unwrap_err()
                .contains("ENCRYPTED PRIVATE KEY")
        );

        fs::write(&tls.key_file, format!("{encrypted}\n{key}")).expect("mixed keys");
        assert!(
            validate_rustls_config(&tls)
                .unwrap_err()
                .contains("ENCRYPTED PRIVATE KEY")
        );
    }

    #[actix_web::test]
    async fn http1_only_transport_negotiates_http1_and_stops_cleanly() {
        let (_directory, tls) = material();
        let trust_anchor = load_certs(&tls.cert_file).expect("certificate").remove(0);
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
                        if !stream.get_ref().0.nodelay().map_err(TlsError::Tls)? {
                            return Err(TlsError::Tls(io::Error::other(
                                "accepted TLS connection did not enable TCP_NODELAY",
                            )));
                        }
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
