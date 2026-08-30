//! AWS SigV4 Authentication for Bedrock
//!
//! Implementation of AWS Signature Version 4 signing process
//! for authenticating requests to AWS Bedrock services.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

/// AWS SigV4 signer for Bedrock requests
#[derive(Clone)]
pub struct SigV4Signer {
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
    region: String,
    service: String,
}

impl std::fmt::Debug for SigV4Signer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SigV4Signer")
            .field("access_key", &"[REDACTED]")
            .field("secret_key", &"[REDACTED]")
            .field(
                "session_token",
                &self.session_token.as_ref().map(|_| "[REDACTED]"),
            )
            .field("region", &self.region)
            .field("service", &self.service)
            .finish()
    }
}

impl SigV4Signer {
    /// Create a new SigV4 signer
    pub fn new(
        access_key: String,
        secret_key: String,
        session_token: Option<String>,
        region: String,
    ) -> Self {
        Self::new_for_service(access_key, secret_key, session_token, region, "bedrock")
    }

    /// Create a signer for another AWS runtime using the same SigV4 contract.
    pub fn new_for_service(
        access_key: String,
        secret_key: String,
        session_token: Option<String>,
        region: String,
        service: impl Into<String>,
    ) -> Self {
        Self {
            access_key,
            secret_key,
            session_token,
            region,
            service: service.into(),
        }
    }

    /// Sign an HTTP request with AWS SigV4
    pub fn sign_request(
        &self,
        method: &str,
        url: &str,
        headers: &HashMap<String, String>,
        body: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<HashMap<String, String>, String> {
        // Parse URL
        let parsed_url = url::Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;

        let mut host = match parsed_url.host().ok_or("Missing host in URL")? {
            url::Host::Domain(domain) => domain.to_string(),
            url::Host::Ipv4(address) => address.to_string(),
            url::Host::Ipv6(address) => format!("[{address}]"),
        };
        if let Some(port) = parsed_url.port() {
            host.push_str(&format!(":{port}"));
        }

        let path = canonical_uri(parsed_url.path());
        let query = canonical_query(parsed_url.query());

        // Format timestamp
        let amz_date = timestamp.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = timestamp.format("%Y%m%d").to_string();

        // Create canonical headers
        let mut canonical_headers = headers.clone();
        canonical_headers.insert("host".to_string(), host);
        canonical_headers.insert("x-amz-date".to_string(), amz_date.clone());

        if let Some(ref token) = self.session_token {
            canonical_headers.insert("x-amz-security-token".to_string(), token.clone());
        }

        // Sort headers by key (case-insensitive)
        let mut sorted_headers: Vec<_> = canonical_headers.iter().collect();
        sorted_headers.sort_by_key(|header| header.0.to_lowercase());

        // Build canonical headers string
        let canonical_headers_str = sorted_headers
            .iter()
            .map(|(k, v)| format!("{}:{}", k.to_lowercase(), v.trim()))
            .collect::<Vec<_>>()
            .join("\n");

        // Build signed headers string
        let signed_headers = sorted_headers
            .iter()
            .map(|(k, _)| k.to_lowercase())
            .collect::<Vec<_>>()
            .join(";");

        // Create canonical request
        let payload_hash = hex::encode(Sha256::digest(body.as_bytes()));
        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n\n{}\n{}",
            method.to_uppercase(),
            path,
            query,
            canonical_headers_str,
            signed_headers,
            payload_hash
        );

        // Create string to sign
        let algorithm = "AWS4-HMAC-SHA256";
        let credential_scope = format!(
            "{}/{}/{}/aws4_request",
            date_stamp, self.region, self.service
        );
        let canonical_request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));

        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm, amz_date, credential_scope, canonical_request_hash
        );

        // Calculate signature
        let signature = self.calculate_signature(&string_to_sign, &date_stamp)?;

        // Create authorization header
        let authorization = format!(
            "{} Credential={}/{}, SignedHeaders={}, Signature={}",
            algorithm, self.access_key, credential_scope, signed_headers, signature
        );

        // Build final headers
        let mut final_headers = canonical_headers;
        final_headers.insert("Authorization".to_string(), authorization);

        Ok(final_headers)
    }

    /// Calculate AWS SigV4 signature
    fn calculate_signature(
        &self,
        string_to_sign: &str,
        date_stamp: &str,
    ) -> Result<String, String> {
        let k_date = self.hmac_sha256(
            format!("AWS4{}", self.secret_key).as_bytes(),
            date_stamp.as_bytes(),
        )?;

        let k_region = self.hmac_sha256(&k_date, self.region.as_bytes())?;
        let k_service = self.hmac_sha256(&k_region, self.service.as_bytes())?;
        let k_signing = self.hmac_sha256(&k_service, b"aws4_request")?;

        let signature = self.hmac_sha256(&k_signing, string_to_sign.as_bytes())?;
        Ok(hex::encode(signature))
    }

    /// HMAC-SHA256 helper function
    fn hmac_sha256(&self, key: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
        let mut mac =
            HmacSha256::new_from_slice(key).map_err(|e| format!("HMAC key error: {}", e))?;
        mac.update(data);
        Ok(mac.finalize().into_bytes().to_vec())
    }
}

fn canonical_uri(path: &str) -> String {
    aws_uri_encode(path.as_bytes(), true)
}

fn canonical_query(query: Option<&str>) -> String {
    let Some(query) = query else {
        return String::new();
    };
    let mut pairs = query
        .split('&')
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (
                aws_uri_encode(&percent_decode(key), false),
                aws_uri_encode(&percent_decode(value), false),
            )
        })
        .collect::<Vec<_>>();
    pairs.sort_unstable();
    pairs
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn percent_decode(value: &str) -> Vec<u8> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
        {
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    decoded
}

fn hex_digit(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn aws_uri_encode(bytes: &[u8], preserve_slash: bool) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut encoded = String::with_capacity(bytes.len());
    for &byte in bytes {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b'/' if preserve_slash => encoded.push('/'),
            _ => {
                encoded.push('%');
                encoded.push(HEX[(byte >> 4) as usize] as char);
                encoded.push(HEX[(byte & 0x0F) as usize] as char);
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    #[test]
    fn test_sigv4_signer_creation() {
        let signer = SigV4Signer::new(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            None,
            "us-east-1".to_string(),
        );

        assert_eq!(signer.access_key, "AKIATEST");
        assert_eq!(signer.region, "us-east-1");
        assert_eq!(signer.service, "bedrock");
    }

    #[test]
    fn signer_debug_redacts_all_credentials() {
        let signer = SigV4Signer::new(
            "debug-access-key".to_string(),
            "debug-secret-key".to_string(),
            Some("debug-session-token".to_string()),
            "us-east-1".to_string(),
        );

        let debug = format!("{signer:?}");
        assert!(!debug.contains("debug-access-key"));
        assert!(!debug.contains("debug-secret-key"));
        assert!(!debug.contains("debug-session-token"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn service_specific_signer_uses_sagemaker_scope() {
        let signer = SigV4Signer::new_for_service(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            None,
            "us-east-1".to_string(),
            "sagemaker",
        );
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let signed = signer
            .sign_request(
                "POST",
                "https://runtime.sagemaker.us-east-1.amazonaws.com/endpoints/demo/invocations",
                &HashMap::new(),
                "{}",
                timestamp,
            )
            .expect("SageMaker request should sign");
        assert!(signed["Authorization"].contains("/us-east-1/sagemaker/aws4_request"));
    }

    #[test]
    fn canonical_query_sorts_and_encodes_equivalent_pairs() {
        let signer = SigV4Signer::new_for_service(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            None,
            "us-east-1".to_string(),
            "sagemaker",
        );
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let first = signer
            .sign_request(
                "GET",
                "https://runtime.sagemaker.us-east-1.amazonaws.com/invoke?z=two%20words&a=%2Fmodel",
                &HashMap::new(),
                "",
                timestamp,
            )
            .expect("first query should sign");
        let second = signer
            .sign_request(
                "GET",
                "https://runtime.sagemaker.us-east-1.amazonaws.com/invoke?a=%2Fmodel&z=two%20words",
                &HashMap::new(),
                "",
                timestamp,
            )
            .expect("second query should sign");

        assert_eq!(
            canonical_query(Some("z=two%20words&a=%2Fmodel")),
            "a=%2Fmodel&z=two%20words"
        );
        assert_eq!(first["Authorization"], second["Authorization"]);
    }

    #[test]
    fn canonical_host_includes_non_default_port() {
        let signer = SigV4Signer::new_for_service(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            None,
            "us-east-1".to_string(),
            "sagemaker",
        );
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let signed = signer
            .sign_request(
                "POST",
                "https://example.com:8443/invoke",
                &HashMap::new(),
                "{}",
                timestamp,
            )
            .expect("custom endpoint should sign");

        assert_eq!(signed["host"], "example.com:8443");
    }

    #[test]
    fn test_hmac_sha256() {
        let signer = SigV4Signer::new(
            "test".to_string(),
            "test".to_string(),
            None,
            "us-east-1".to_string(),
        );

        let result = signer.hmac_sha256(b"key", b"message");
        assert!(result.is_ok());

        // Known HMAC-SHA256 result for key="key", message="message"
        let expected = "6e9ef29b75fffc5b7abae527d58fdadb2fe42e7219011976917343065f58ed4a";
        assert_eq!(hex::encode(result.unwrap()), expected);
    }

    #[test]
    fn canonical_uri_double_encodes_escaped_model_id_chars() {
        assert_eq!(
            canonical_uri("/model/anthropic.claude-3-5-haiku-20241022-v1%3A0/converse"),
            "/model/anthropic.claude-3-5-haiku-20241022-v1%253A0/converse"
        );
        assert_eq!(
            canonical_uri(
                "/model/arn%3Aaws%3Abedrock%3Aus-east-1%3A123456789012%3Ainference-profile%2Fus.anthropic.claude-3-5-sonnet-20241022-v2%3A0/converse"
            ),
            "/model/arn%253Aaws%253Abedrock%253Aus-east-1%253A123456789012%253Ainference-profile%252Fus.anthropic.claude-3-5-sonnet-20241022-v2%253A0/converse"
        );
    }

    #[test]
    fn test_sign_request() {
        let signer = SigV4Signer::new(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            None,
            "us-east-1".to_string(),
        );

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let headers = HashMap::new();

        let result = signer.sign_request(
            "POST",
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/test/invoke",
            &headers,
            "{}",
            timestamp,
        );

        assert!(result.is_ok());
        let signed_headers = result.unwrap();
        assert!(signed_headers.contains_key("Authorization"));
        assert!(signed_headers.contains_key("x-amz-date"));
    }
}
