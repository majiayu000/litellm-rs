use super::*;
use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const TEST_RSA_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----\nMIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQCrR7vVkeK5GZpi\nERe4p/w/3cUHo7vaASXPOv3WB7rwnajDIg+VWVAdlnfOWz29uOydId30WNGJuphD\ndBlyNn+0aNEAGHXfFNX5FaPiCeGTX2DfW27QONWO3qTKohsgpKiAZd4boWgv5/k7\nzyvEC/aeKtqBFuMMWD9xkmUTGuQFIx9QE5OnqF6vG4gjy+3K/fGOsEXuyF6JsFay\ny+371B4sc+Xwg9l5NtwXmeLXblA9lf9eZoHbDBrkp0W/nJzseKBNjIKIXApI3vDc\nlALJkMk31PTJ2v+8sB4uNnznpW600Oi6zhKqkAuDVrVQVGpKCgPeSAxhikuZAHIx\nEdVDYsuvAgMBAAECggEAPvstKA6xWlP+T1IusVFf8ZIYKcN8x2CFqSptfV6xUFoA\n3OPw6/9/9KlIG6K0VMejhfIWngts3WK2K5OM6dD9a3bhZ1IXQbT1K1bYQL1Wa6z2\nP5ts53cGnDblTLeIFxxE85XBstJKr9bycBoxYzDs+eMTHsWuLnNivN3SedB5CSPz\nl74uLafcE599qBckruPaAPKanhpNmI7PzDxKS5tu7Tj9hSzwUW4NtOEc/pr9L7N6\nalBsF9S2QMCKZOcl4X42sLVYKVpwHKvxjxSkJKJsDOs83r7r9Ph2A4S4YxgIXZFF\nma9O0ZIR6lVY7AkY+JjZdPCjmCcDmXZkRydGLMhSiQKBgQDRy01YhvsWwzfdO1C0\nS6jDI+AGT9V+RNEcNTCFcr/uzeHhZTroiokHEsFIb2AP6GBuKIy2XxqmtDaeOSDf\nd3XWelXUDnfkxCxV1WkOwNKIiF1HuDgRr9IDlkKXLFMS7pG6zY9dE40h3VPwJ1U2\nHZXPTLEU2376mEnsoog/PYwKZwKBgQDRAOxY7VXO4eRt870Vr8ocTT42Zzc08m1m\nBgL6RORrBYdZgBP5fvUkr/WGmk2Obn7m/8f0U8C8T0KAPIveQRmrJCJrX5kZqsqZ\nw3sUp5gy6g+vC4JXapMVfUlNc7fANC2baJXGpYH7DNLJ0dGHpHAlkjiETtsw9WON\nkOv5MER3eQKBgQC2htYNbqrofAKPpXqq0qTK2tyfQTgzOrZgf1pu0I5yu4eJ7eQZ\ny+Y6VDP7zILcdEXpsbfzN71dSq+2a2fRZQMODrO74ranP5J/P0S/RD4n8dSOgJWv\ntbPX0RSwqCzC7PO3ff78cPU6gHD2IZJ+mbDsggITboEEkBjJHAPEWc0MgwKBgQCZ\n14QhMRGoZr4t8OuNuweaLYFNqkwIvSmpn2MxtOQtorQuPQh27eykRKEFoy7TWKIw\nhrY4Mi38bpsUqXyK7IBoaQCs6IFZU04uQKWoXnS5hXBl+KLIlboOZ1o9mJ/46m9n\npWQaBFnY4WeHBtqkbXXfMfJH8YOGVhohajtIAS9kgQKBgCEA+TJKeDzghjsJOvxi\nIVEFvNJTWy1GPrbCGX2VYbKYHndrUNAt27euAV0P/OQ75OZt1jD5AhWI0UOoc+aH\nHxzUTotGbkDPVr+DJjtclKKcjDxjhua2R2QTZCzJdifirhtt+scmkTMkC9m9lCdz\n8o0y8rETRWzsNVewp71sFfdr\n-----END PRIVATE KEY-----";

#[test]
fn official_endpoints_are_mode_specific_and_region_is_validated() {
    let config = OciConfig {
        region: "us-chicago-1".to_string(),
        compartment_id: None,
        project_id: Some("ocid1.generativeaiproject.oc1.us-chicago-1.test".to_string()),
        auth: OciAuth::ApiKey {
            token: "key".to_string(),
        },
        api_mode: OciApiMode::OpenAiCompatible,
        base_url: None,
        endpoint_access: ProviderEndpointAccess::PublicOnly,
        timeout: 60,
        max_retries: 2,
        models: Vec::new(),
    };
    assert_eq!(
        config.api_base().expect("valid compatible region"),
        "https://inference.generativeai.us-chicago-1.oci.oraclecloud.com/openai/v1"
    );
    assert_eq!(
        OciConfig {
            api_mode: OciApiMode::Native,
            ..config.clone()
        }
        .api_base()
        .expect("valid native region"),
        "https://inference.generativeai.us-chicago-1.oci.oraclecloud.com/20231130"
    );
    assert!(
        OciConfig {
            region: "us/evil".to_string(),
            ..config
        }
        .api_base()
        .is_err()
    );
}

#[tokio::test]
async fn compatible_mode_requires_project_identity() {
    let error = OciConfig {
        region: "us-chicago-1".to_string(),
        compartment_id: None,
        project_id: None,
        auth: OciAuth::ApiKey {
            token: "key".to_string(),
        },
        api_mode: OciApiMode::OpenAiCompatible,
        base_url: None,
        endpoint_access: ProviderEndpointAccess::PublicOnly,
        timeout: 60,
        max_retries: 2,
        models: Vec::new(),
    }
    .build()
    .await
    .expect_err("/openai/v1 requires a project OCID");

    assert!(matches!(error, ProviderError::Configuration { .. }));
    assert!(error.to_string().contains("project"));
}

#[tokio::test]
async fn compatible_mode_emits_reserved_openai_project_header() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("test listener should bind");
    let address = listener.local_addr().expect("listener should have address");
    let redirect_target = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("redirect target should bind");
    let redirect_address = redirect_target
        .local_addr()
        .expect("redirect target should have address");
    let mut redirect_probe = tokio::spawn(async move { redirect_target.accept().await.is_ok() });
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request should arrive");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 2048];
        loop {
            let count = socket.read(&mut buffer).await.expect("request should read");
            if count == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..count]);
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        let headers = String::from_utf8_lossy(&request).to_ascii_lowercase();
        assert!(
            headers.contains(
                "\r\nopenai-project: ocid1.generativeaiproject.oc1.us-chicago-1.test\r\n"
            )
        );
        assert!(headers.contains("\r\nauthorization: bearer api-key\r\n"));
        socket
            .write_all(
                format!(
                    "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{redirect_address}/capture\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .await
            .expect("response should write");
    });
    let config: OciConfig = serde_json::from_value(serde_json::json!({
        "region":"us-chicago-1",
        "compartment_id":null,
        "project_id":"ocid1.generativeaiproject.oc1.us-chicago-1.test",
        "auth":{"type":"api_key","token":"api-key"},
        "api_mode":"open_ai_compatible",
        "base_url":format!("http://{address}/openai/v1"),
        "endpoint_access":"private_network",
        "timeout":60,
        "max_retries":2,
        "models":[]
    }))
    .expect("project-aware config should parse");
    let provider = config.build().await.expect("provider should build");
    let _ = provider
        .chat_completion(
            ChatRequest {
                model: "oci/test-model".to_string(),
                ..Default::default()
            },
            RequestContext::default(),
        )
        .await
        .expect_err("test server returns a non-followed redirect");
    server.await.expect("test server should finish");
    if tokio::time::timeout(std::time::Duration::from_millis(100), &mut redirect_probe)
        .await
        .is_ok()
    {
        panic!("credentialed enterprise request followed a redirect");
    }
    redirect_probe.abort();
}

#[test]
fn custom_endpoint_rejects_userinfo_query_and_fragment() {
    let config = OciConfig {
        region: "us-chicago-1".to_string(),
        compartment_id: None,
        project_id: Some("ocid1.generativeaiproject.oc1.us-chicago-1.test".to_string()),
        auth: OciAuth::ApiKey {
            token: "key".to_string(),
        },
        api_mode: OciApiMode::OpenAiCompatible,
        base_url: None,
        endpoint_access: ProviderEndpointAccess::PublicOnly,
        timeout: 60,
        max_retries: 2,
        models: Vec::new(),
    };
    for endpoint in [
        "https://user:password@oci.example.com/openai/v1",
        "https://oci.example.com/openai/v1?tenant=other",
        "https://oci.example.com/openai/v1#fragment",
    ] {
        assert!(
            OciConfig {
                base_url: Some(endpoint.to_string()),
                ..config.clone()
            }
            .api_base()
            .is_err(),
            "custom endpoint must reject {endpoint}"
        );
    }
}

#[tokio::test]
async fn auth_and_mode_combinations_fail_closed() {
    let config = OciConfig {
        region: "us-chicago-1".to_string(),
        compartment_id: Some("ocid1.compartment.oc1..test".to_string()),
        project_id: None,
        auth: OciAuth::ApiKey {
            token: "key".to_string(),
        },
        api_mode: OciApiMode::Native,
        base_url: None,
        endpoint_access: ProviderEndpointAccess::PublicOnly,
        timeout: 60,
        max_retries: 2,
        models: Vec::new(),
    };
    assert!(config.build().await.is_err());
}

#[test]
fn native_constructor_rejects_invalid_iam_credentials_before_publish() {
    let config = OciConfig {
        region: "us-chicago-1".to_string(),
        compartment_id: Some("ocid1.compartment.oc1..test".to_string()),
        project_id: None,
        auth: OciAuth::Iam {
            tenancy_ocid: "ocid1.tenancy.oc1..test".to_string(),
            user_ocid: "ocid1.user.oc1..test".to_string(),
            fingerprint: "aa:bb:cc".to_string(),
            private_key_pem: "not-an-rsa-private-key".to_string(),
        },
        api_mode: OciApiMode::Native,
        base_url: None,
        endpoint_access: ProviderEndpointAccess::PublicOnly,
        timeout: 60,
        max_retries: 2,
        models: Vec::new(),
    };

    let error = OciNativeProvider::new(config)
        .expect_err("invalid signing key must fail during runtime construction");
    assert!(error.to_string().contains("private_key_pem"));
}

#[test]
fn native_constructor_rejects_empty_iam_identity_before_publish() {
    let config = OciConfig {
        region: "us-chicago-1".to_string(),
        compartment_id: Some("ocid1.compartment.oc1..test".to_string()),
        project_id: None,
        auth: OciAuth::Iam {
            tenancy_ocid: String::new(),
            user_ocid: String::new(),
            fingerprint: String::new(),
            private_key_pem: "not-reached".to_string(),
        },
        api_mode: OciApiMode::Native,
        base_url: None,
        endpoint_access: ProviderEndpointAccess::PublicOnly,
        timeout: 60,
        max_retries: 2,
        models: Vec::new(),
    };

    let error = OciNativeProvider::new(config)
        .expect_err("empty IAM identity must fail during runtime construction");
    assert!(error.to_string().contains("tenancy_ocid"));
}

#[test]
fn native_constructor_rejects_malformed_iam_signature_parameters() {
    let error = OciNativeProvider::new(OciConfig {
        region: "us-chicago-1".to_string(),
        compartment_id: Some("ocid1.compartment.oc1..test".to_string()),
        project_id: None,
        auth: OciAuth::Iam {
            tenancy_ocid: "ocid1.tenancy.oc1..test\n".to_string(),
            user_ocid: "ocid1.user.oc1..test".to_string(),
            fingerprint: "aa:bb:cc".to_string(),
            private_key_pem: TEST_RSA_PRIVATE_KEY.to_string(),
        },
        api_mode: OciApiMode::Native,
        base_url: None,
        endpoint_access: ProviderEndpointAccess::PublicOnly,
        timeout: 60,
        max_retries: 2,
        models: Vec::new(),
    })
    .expect_err("malformed keyId component must fail before publish");

    assert!(matches!(error, ProviderError::Configuration { .. }));
    assert!(error.to_string().contains("tenancy_ocid"));
}

#[test]
fn native_signature_brackets_ipv6_host_with_port() {
    let auth = OciAuth::Iam {
        tenancy_ocid: "ocid1.tenancy.oc1..test".to_string(),
        user_ocid: "ocid1.user.oc1..test".to_string(),
        fingerprint: "aa:bb:cc".to_string(),
        private_key_pem: TEST_RSA_PRIVATE_KEY.to_string(),
    };
    let key = oci_iam_signing_key(&auth).expect("test key should parse");
    let headers = oci_iam_headers(
        &auth,
        key.as_ref(),
        "POST",
        "https://[2001:db8::1]:8443/20231130/actions/embedText",
        "{}",
        chrono::Utc::now(),
    )
    .expect("request should sign");

    assert_eq!(
        headers.get("host").map(String::as_str),
        Some("[2001:db8::1]:8443")
    );
}

#[test]
fn official_rerank_contract_uses_input_document_ranks_and_internal_id() {
    let config = OciConfig {
        region: "us-chicago-1".to_string(),
        compartment_id: Some("ocid1.compartment.oc1..test".to_string()),
        project_id: None,
        auth: OciAuth::ApiKey {
            token: "unused".to_string(),
        },
        api_mode: OciApiMode::Native,
        base_url: None,
        endpoint_access: ProviderEndpointAccess::PublicOnly,
        timeout: 60,
        max_retries: 2,
        models: Vec::new(),
    };
    let request = RerankRequest {
        model: "oci/cohere.rerank-v3-5".to_string(),
        query: "query text".to_string(),
        documents: vec![
            RerankDocument::from("first"),
            RerankDocument::from("second"),
        ],
        top_n: Some(1),
        return_documents: Some(false),
        ..Default::default()
    };
    let body = oci_rerank_body(&config, "cohere.rerank-v3-5", &request);
    assert_eq!(body["input"], "query text");
    assert!(body.get("query").is_none());
    assert_eq!(body["isEcho"], false);

    let parsed = parse_oci_rerank_response(
        "cohere.rerank-v3-5".to_string(),
        serde_json::json!({
            "documentRanks": [{"index": 1, "relevanceScore": 0.93}],
            "modelId": "cohere.rerank-v3-5",
            "modelVersion": "1.0"
        }),
        &request.documents,
        false,
    )
    .expect("official response shape should parse");
    assert!(!parsed.id.is_empty());
    assert_eq!(parsed.results[0].index, 1);
    assert!(parsed.results[0].document.is_none());
}

#[test]
fn official_embedding_usage_is_preserved() {
    let response = parse_oci_embedding_response(
        "cohere.embed-v4.0".to_string(),
        serde_json::json!({
            "embeddings": [[0.25, 0.75]],
            "usage": {"promptTokens": 4, "totalTokens": 4}
        }),
    )
    .expect("official embedding response should parse");

    let usage = response.usage.expect("usage should be retained");
    assert_eq!(usage.prompt_tokens, 4);
    assert_eq!(usage.total_tokens, 4);
}

fn assert_oci_rerank_response_parsing(error: GatewayError, expected_message: &str) {
    match error {
        GatewayError::Provider(ProviderError::ResponseParsing { provider, message }) => {
            assert_eq!(provider, "oci");
            assert!(
                message.contains(expected_message),
                "unexpected parsing error: {message}"
            );
        }
        other => panic!("expected OCI response parsing error, got {other:?}"),
    }
}

#[test]
fn malformed_rerank_success_payloads_are_provider_response_errors() {
    let documents = vec![RerankDocument::from("first")];
    for (response, expected_message) in [
        (
            serde_json::json!({}),
            "rerank response missing documentRanks",
        ),
        (
            serde_json::json!({
                "documentRanks": [{"relevanceScore": 0.5}]
            }),
            "rerank result missing index",
        ),
        (
            serde_json::json!({
                "documentRanks": [{"index": 0}]
            }),
            "rerank result missing relevanceScore",
        ),
    ] {
        let error =
            parse_oci_rerank_response("cohere.rerank-v3-5".to_string(), response, &documents, true)
                .expect_err("malformed upstream success payload must fail");
        assert_oci_rerank_response_parsing(error, expected_message);
    }
}

#[test]
fn out_of_range_rerank_index_is_a_provider_response_error_without_echo() {
    let documents = vec![RerankDocument::from("first")];
    let error = parse_oci_rerank_response(
        "cohere.rerank-v3-5".to_string(),
        serde_json::json!({
            "documentRanks": [{"index": 1, "relevanceScore": 0.5}]
        }),
        &documents,
        false,
    )
    .expect_err("out-of-range upstream index must fail even when documents are not echoed");

    assert_oci_rerank_response_parsing(error, "rerank result index is out of range");
}
