use super::*;

#[test]
fn official_endpoints_are_mode_specific_and_region_is_validated() {
    let config = OciConfig {
        region: "us-chicago-1".to_string(),
        compartment_id: None,
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

#[test]
fn custom_endpoint_rejects_userinfo_query_and_fragment() {
    let config = OciConfig {
        region: "us-chicago-1".to_string(),
        compartment_id: None,
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
fn official_rerank_contract_uses_input_document_ranks_and_internal_id() {
    let config = OciConfig {
        region: "us-chicago-1".to_string(),
        compartment_id: Some("ocid1.compartment.oc1..test".to_string()),
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
