//! TLS listener support.
//!
//! When the `server.tls` section is present the gateway serves HTTPS via
//! rustls instead of silently ignoring the configuration. Loading fails
//! closed on any unreadable file, unparsable content, or unsupported option.

use crate::config::models::server::TlsConfig;
use crate::utils::error::gateway_error::{GatewayError, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

/// Build a rustls [`ServerConfig`] from the configured PEM files.
pub(crate) fn load_rustls_config(tls: &TlsConfig) -> Result<rustls::ServerConfig> {
    if tls.require_client_cert {
        return Err(GatewayError::not_implemented(
            "tls.require_client_cert is not implemented yet; set it to false",
        ));
    }

    let certs = load_certs(&tls.cert_file)?;
    let key = load_key(&tls.key_file)?;

    let mut config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| GatewayError::validation(format!("invalid TLS certificate/key pair: {e}")))?;

    config.alpn_protocols = if tls.http2 {
        vec![b"h2".to_vec(), b"http/1.1".to_vec()]
    } else {
        vec![b"http/1.1".to_vec()]
    };
    Ok(config)
}

fn load_certs(path: &str) -> Result<Vec<CertificateDer<'static>>> {
    let file = std::fs::File::open(path)
        .map_err(|e| GatewayError::validation(format!("cannot open TLS cert file {path}: {e}")))?;
    let mut reader = std::io::BufReader::new(file);
    rustls_pemfile::certs(&mut reader)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| GatewayError::validation(format!("invalid TLS certificates in {path}: {e}")))
}

fn load_key(path: &str) -> Result<PrivateKeyDer<'static>> {
    let file = std::fs::File::open(path)
        .map_err(|e| GatewayError::validation(format!("cannot open TLS key file {path}: {e}")))?;
    let mut reader = std::io::BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .map_err(|e| GatewayError::validation(format!("invalid TLS key in {path}: {e}")))?
        .ok_or_else(|| GatewayError::validation(format!("no private key found in {path}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tls_config(require_client_cert: bool) -> TlsConfig {
        TlsConfig {
            cert_file: "/nonexistent/cert.pem".into(),
            key_file: "/nonexistent/key.pem".into(),
            ca_file: None,
            require_client_cert,
            http2: false,
        }
    }

    #[test]
    fn rejects_unimplemented_client_cert_auth() {
        let err = load_rustls_config(&tls_config(true))
            .unwrap_err()
            .to_string();
        assert!(err.contains("require_client_cert"));
    }

    #[test]
    fn missing_cert_file_fails_closed() {
        assert!(load_rustls_config(&tls_config(false)).is_err());
    }
}
