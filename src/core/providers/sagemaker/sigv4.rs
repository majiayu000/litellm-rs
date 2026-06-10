//! AWS SigV4 Authentication for Sagemaker
//!
//! Implementation of AWS Signature Version 4 signing process
//! for authenticating requests to AWS Sagemaker services.

use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

type HmacSha256 = Hmac<Sha256>;

/// AWS SigV4 signer for Sagemaker requests
#[derive(Debug, Clone)]
pub struct SagemakerSigV4Signer {
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
    region: String,
}

impl SagemakerSigV4Signer {
    /// Create a new SigV4 signer for Sagemaker
    pub fn new(
        access_key: String,
        secret_key: String,
        session_token: Option<String>,
        region: String,
    ) -> Self {
        Self {
            access_key,
            secret_key,
            session_token,
            region,
        }
    }

    /// Sign an HTTP request with AWS SigV4 for Sagemaker
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

        let host = parsed_url.host_str().ok_or("Missing host in URL")?;
        let path = parsed_url.path();
        let query = parsed_url.query().unwrap_or("");

        // Format timestamp
        let amz_date = timestamp.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = timestamp.format("%Y%m%d").to_string();

        // Create canonical headers
        let mut canonical_headers = headers.clone();
        canonical_headers.insert("host".to_string(), host.to_string());
        canonical_headers.insert("x-amz-date".to_string(), amz_date.clone());

        // Add content-type if not present
        if !canonical_headers.contains_key("content-type") {
            canonical_headers.insert("content-type".to_string(), "application/json".to_string());
        }

        if let Some(ref token) = self.session_token {
            canonical_headers.insert("x-amz-security-token".to_string(), token.clone());
        }

        // Sort headers by key (case-insensitive)
        let mut sorted_headers: Vec<_> = canonical_headers.iter().collect();
        sorted_headers.sort_by(|(a_key, _), (b_key, _)| {
            let a_bytes = a_key.as_bytes().iter().map(|b| b.to_ascii_lowercase());
            let b_bytes = b_key.as_bytes().iter().map(|b| b.to_ascii_lowercase());
            a_bytes.cmp(b_bytes)
        });

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
        let service = "sagemaker";
        let credential_scope = format!("{}/{}/{}/aws4_request", date_stamp, self.region, service);
        let canonical_request_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));

        let string_to_sign = format!(
            "{}\n{}\n{}\n{}",
            algorithm, amz_date, credential_scope, canonical_request_hash
        );

        // Calculate signature
        let signature = self.calculate_signature(&string_to_sign, &date_stamp, service)?;

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
        service: &str,
    ) -> Result<String, String> {
        let k_date = self.hmac_sha256(
            format!("AWS4{}", self.secret_key).as_bytes(),
            date_stamp.as_bytes(),
        )?;

        let k_region = self.hmac_sha256(&k_date, self.region.as_bytes())?;
        let k_service = self.hmac_sha256(&k_region, service.as_bytes())?;
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

    /// Get the region
    pub fn region(&self) -> &str {
        &self.region
    }

    /// Check if using temporary credentials
    pub fn is_temporary_credentials(&self) -> bool {
        self.session_token.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_sagemaker_sigv4_signer_creation() {
        let signer = SagemakerSigV4Signer::new(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            None,
            "us-east-1".to_string(),
        );

        assert_eq!(signer.region(), "us-east-1");
        assert!(!signer.is_temporary_credentials());
    }

    #[test]
    fn test_sagemaker_sigv4_signer_with_session_token() {
        let signer = SagemakerSigV4Signer::new(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            Some("session-token".to_string()),
            "us-west-2".to_string(),
        );

        assert!(signer.is_temporary_credentials());
    }

    #[test]
    fn test_hmac_sha256() {
        let signer = SagemakerSigV4Signer::new(
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
    fn test_sign_request() {
        let signer = SagemakerSigV4Signer::new(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            None,
            "us-east-1".to_string(),
        );

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let headers = HashMap::new();

        let result = signer.sign_request(
            "POST",
            "https://runtime.sagemaker.us-east-1.amazonaws.com/endpoints/my-endpoint/invocations",
            &headers,
            r#"{"inputs": "Hello"}"#,
            timestamp,
        );

        assert!(result.is_ok());
        let signed_headers = result.unwrap();
        assert!(signed_headers.contains_key("Authorization"));
        assert!(signed_headers.contains_key("x-amz-date"));
        assert!(signed_headers.contains_key("host"));
        assert!(signed_headers.contains_key("content-type"));
    }

    #[test]
    fn test_sign_request_with_session_token() {
        let signer = SagemakerSigV4Signer::new(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            Some("session-token-123".to_string()),
            "us-west-2".to_string(),
        );

        let timestamp = Utc.with_ymd_and_hms(2024, 6, 15, 10, 30, 0).unwrap();
        let headers = HashMap::new();

        let result = signer.sign_request(
            "POST",
            "https://runtime.sagemaker.us-west-2.amazonaws.com/endpoints/test/invocations",
            &headers,
            "{}",
            timestamp,
        );

        assert!(result.is_ok());
        let signed_headers = result.unwrap();
        assert!(signed_headers.contains_key("x-amz-security-token"));
    }

    #[test]
    fn test_sign_request_preserves_existing_headers() {
        let signer = SagemakerSigV4Signer::new(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            None,
            "us-east-1".to_string(),
        );

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let mut headers = HashMap::new();
        headers.insert("content-type".to_string(), "application/json".to_string());
        headers.insert("x-custom-header".to_string(), "custom-value".to_string());

        let result = signer.sign_request(
            "POST",
            "https://runtime.sagemaker.us-east-1.amazonaws.com/endpoints/test/invocations",
            &headers,
            "{}",
            timestamp,
        );

        assert!(result.is_ok());
        let signed_headers = result.unwrap();
        assert_eq!(
            signed_headers.get("content-type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(
            signed_headers.get("x-custom-header"),
            Some(&"custom-value".to_string())
        );
    }

    #[test]
    fn test_sign_request_invalid_url() {
        let signer = SagemakerSigV4Signer::new(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            None,
            "us-east-1".to_string(),
        );

        let timestamp = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let headers = HashMap::new();

        let result = signer.sign_request("POST", "not-a-valid-url", &headers, "{}", timestamp);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid URL"));
    }

    #[test]
    fn test_signer_clone() {
        let signer = SagemakerSigV4Signer::new(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            Some("token".to_string()),
            "eu-west-1".to_string(),
        );

        let cloned = signer.clone();
        assert_eq!(cloned.region(), signer.region());
        assert_eq!(
            cloned.is_temporary_credentials(),
            signer.is_temporary_credentials()
        );
    }

    #[test]
    fn test_signer_debug() {
        let signer = SagemakerSigV4Signer::new(
            "AKIATEST".to_string(),
            "testsecret".to_string(),
            None,
            "us-east-1".to_string(),
        );

        let debug_str = format!("{:?}", signer);
        assert!(debug_str.contains("SagemakerSigV4Signer"));
        assert!(debug_str.contains("us-east-1"));
    }
}
