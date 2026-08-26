//! Shared TLS material validation and rustls configuration construction.

use crate::config::models::server::TlsConfig;
use crate::utils::error::gateway_error::{GatewayError, Result};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};

/// Build the same strict rustls configuration used by validation and startup.
pub(crate) fn load_rustls_config(tls: &TlsConfig) -> Result<rustls::ServerConfig> {
    build_rustls_config(tls).map_err(GatewayError::validation)
}

/// Validate TLS material without retaining the constructed listener configuration.
pub(crate) fn validate_rustls_config(tls: &TlsConfig) -> std::result::Result<(), String> {
    build_rustls_config(tls).map(drop)
}

fn build_rustls_config(tls: &TlsConfig) -> std::result::Result<rustls::ServerConfig, String> {
    if tls.http2 {
        return Err("tls.http2 is unsupported; set it to false to use HTTP/1 TLS".into());
    }
    if tls.ca_file.is_some() {
        return Err(
            "tls.ca_file is unsupported until client certificate auth is implemented".into(),
        );
    }
    if tls.require_client_cert {
        return Err("tls.require_client_cert is not implemented yet; set it to false".into());
    }

    let certs = load_certs(&tls.cert_file)?;
    let key = load_key(&tls.key_file)?;
    let provider = rustls::crypto::ring::default_provider();
    let mut config = rustls::ServerConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()
        .map_err(|error| format!("cannot select safe TLS protocol versions: {error}"))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|error| format!("invalid TLS certificate/key pair: {error}"))?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

pub(crate) fn load_certs(path: &str) -> std::result::Result<Vec<CertificateDer<'static>>, String> {
    let contents = read_pem(path, "cert", &["CERTIFICATE"])?;
    let certs = CertificateDer::pem_slice_iter(&contents)
        .enumerate()
        .map(|(index, cert)| {
            let cert =
                cert.map_err(|error| format!("invalid TLS certificates in {path}: {error}"))?;
            rustls::server::ParsedCertificate::try_from(&cert)
                .map_err(|error| format!("invalid TLS certificate {index} in {path}: {error}"))?;
            Ok(cert)
        })
        .collect::<std::result::Result<Vec<_>, String>>()?;
    if certs.is_empty() {
        return Err(format!("no TLS certificates found in {path}"));
    }
    Ok(certs)
}

fn load_key(path: &str) -> std::result::Result<PrivateKeyDer<'static>, String> {
    let contents = read_pem(
        path,
        "key",
        &[
            "PRIVATE KEY",
            "RSA PRIVATE KEY",
            "EC PRIVATE KEY",
            "EC PARAMETERS",
        ],
    )?;
    let mut keys = PrivateKeyDer::pem_slice_iter(&contents)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| format!("invalid TLS key in {path}: {error}"))?
        .into_iter();
    let key = keys
        .next()
        .ok_or_else(|| format!("no unencrypted private key found in {path}"))?;
    if keys.next().is_some() {
        return Err(format!(
            "multiple private keys found in {path}; expected exactly one"
        ));
    }
    Ok(key)
}

fn read_pem(
    path: &str,
    kind: &str,
    allowed_labels: &[&str],
) -> std::result::Result<Vec<u8>, String> {
    let contents = std::fs::read(path)
        .map_err(|error| format!("cannot open TLS {kind} file {path}: {error}"))?;
    let text = std::str::from_utf8(&contents)
        .map_err(|error| format!("TLS {kind} file {path} is not UTF-8 PEM: {error}"))?;
    for line in text.lines().map(str::trim) {
        if let Some(label) = line
            .strip_prefix("-----BEGIN ")
            .and_then(|line| line.strip_suffix("-----"))
            && !allowed_labels.contains(&label)
        {
            return Err(format!(
                "unsupported PEM block '{label}' in TLS {kind} file {path}"
            ));
        }
    }
    Ok(contents)
}
