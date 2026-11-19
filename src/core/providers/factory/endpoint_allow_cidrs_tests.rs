//! #1503: `endpoint_allow_cidrs` reaches the provider built by the factory.

use super::create_provider;
use crate::config::models::provider::ProviderConfig;
use crate::core::net::EndpointAllowCidrs;
use crate::core::providers::{Provider, ProviderType};
use crate::core::types::chat::ChatRequest;
use crate::core::types::context::RequestContext;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn cidrs(values: &[&str]) -> EndpointAllowCidrs {
    EndpointAllowCidrs::parse(values).expect("test CIDRs should parse")
}

fn provider(provider_type: &str, base_url: &str, allow: &[&str]) -> ProviderConfig {
    ProviderConfig {
        name: format!("{provider_type}-allow-test"),
        provider_type: provider_type.to_string(),
        api_key: "sk-allow-test".to_string(),
        base_url: Some(base_url.to_string()),
        endpoint_allow_cidrs: cidrs(allow),
        ..Default::default()
    }
}

#[tokio::test]
async fn fake_ip_literal_endpoint_needs_the_allow_list_on_every_factory_path() {
    for provider_type in ["openai_compatible", "deepseek"] {
        let base_url = "https://198.18.0.7/v1";
        let error = create_provider(provider(provider_type, base_url, &[]))
            .await
            .expect_err("fake-IP endpoint must be rejected by default")
            .to_string();
        assert!(
            error.contains("198.18.0.7") && error.contains("endpoint_allow_cidrs"),
            "{provider_type}: {error}"
        );

        let created = create_provider(provider(provider_type, base_url, &["198.18.0.0/15"]))
            .await
            .unwrap_or_else(|error| panic!("{provider_type} opt-in must build: {error}"));
        assert!(
            matches!(&created, Provider::OpenAILike(p)
                if p.config().base.endpoint_allow_cidrs == cidrs(&["198.18.0.0/15"])),
            "{provider_type} must carry the allow list into its runtime config"
        );
    }

    // Direct factory JSON path.
    let direct = Provider::from_config_async(
        ProviderType::OpenAICompatible,
        serde_json::json!({"api_key": "sk-allow-test", "base_url": "https://198.18.0.7/v1",
            "endpoint_allow_cidrs": ["198.18.0.0/15"]}),
    )
    .await
    .expect("direct factory opt-in must build");
    assert!(matches!(&direct, Provider::OpenAILike(p)
        if !p.config().base.endpoint_allow_cidrs.is_empty()));
    let invalid = Provider::from_config_async(
        ProviderType::OpenAICompatible,
        serde_json::json!({"api_key": "sk-allow-test", "base_url": "https://api.example.com/v1",
            "endpoint_allow_cidrs": ["198.18.0.1/15"]}),
    )
    .await
    .expect_err("invalid CIDR must be a configuration error");
    assert!(
        invalid.to_string().contains("endpoint_allow_cidrs"),
        "{invalid}"
    );
}

#[tokio::test]
async fn metadata_and_unsupported_runtimes_fail_closed() {
    let error = create_provider(provider(
        "openai_compatible",
        "http://169.254.169.254/v1",
        &["168.0.0.0/7"],
    ))
    .await
    .expect_err("metadata endpoint must stay blocked")
    .to_string();
    assert!(error.contains("always blocked"), "{error}");

    let error = create_provider(provider(
        "anthropic",
        "https://api.anthropic.com",
        &["198.18.0.0/15"],
    ))
    .await
    .expect_err("runtimes that cannot honour the allow list must reject it")
    .to_string();
    assert!(
        error.contains("endpoint_allow_cidrs is not supported"),
        "{error}"
    );
    let direct = Provider::from_config_async(
        ProviderType::Anthropic,
        serde_json::json!({"api_key": "sk-ant-x", "endpoint_allow_cidrs": ["198.18.0.0/15"]}),
    )
    .await
    .expect_err("direct factory must reject an unsupported allow list");
    assert!(
        direct.to_string().contains("endpoint_allow_cidrs"),
        "{direct}"
    );

    let mut nested = provider("openai_compatible", "https://api.example.com/v1", &[]);
    nested.settings.insert(
        "endpoint_allow_cidrs".into(),
        serde_json::json!(["198.18.0.0/15"]),
    );
    let error = create_provider(nested)
        .await
        .expect_err("nested setting must be rejected");
    assert!(error.to_string().contains("top-level"), "{error}");
}

/// End to end: a factory-built provider whose endpoint sits in an otherwise
/// rejected range completes a real chat request only with the opt-in.
#[tokio::test]
async fn opted_in_provider_completes_a_real_request() {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("listener should bind");
    let base_url = format!("http://{}/v1", listener.local_addr().unwrap());
    let body = r#"{"id":"c1","object":"chat.completion","created":1,"model":"m","choices":[{"index":0,"message":{"role":"assistant","content":"through"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#;
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("provider should connect");
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        while !String::from_utf8_lossy(&request).contains("\"messages\"") {
            let read = socket
                .read(&mut buffer)
                .await
                .expect("request should be readable");
            assert!(read > 0, "request ended early");
            request.extend_from_slice(&buffer[..read]);
        }
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("response should write");
        String::from_utf8_lossy(&request).into_owned()
    });

    // public_only rejects loopback; the allow list re-admits exactly this range.
    let error = create_provider(provider("openai_compatible", &base_url, &[]))
        .await
        .expect_err("non-opted provider must be rejected");
    assert!(
        error.to_string().contains("endpoint_allow_cidrs"),
        "{error}"
    );

    let created = create_provider(provider("openai_compatible", &base_url, &["127.0.0.0/8"]))
        .await
        .expect("opted-in provider must build");
    let response = created
        .chat_completion(
            ChatRequest::new("m").add_user_message("hello"),
            RequestContext::default(),
        )
        .await
        .expect("opted-in provider must reach the endpoint");
    let request = server.await.expect("server task should finish");
    assert!(
        request.starts_with("POST /v1/chat/completions "),
        "{request}"
    );
    assert_eq!(
        response.choices[0]
            .message
            .content
            .as_ref()
            .map(ToString::to_string),
        Some("through".to_string())
    );
}
