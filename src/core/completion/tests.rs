//! Completion module tests

use super::*;
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::Provider;
use crate::core::providers::base::BaseConfig;
use crate::core::providers::openai::{OpenAIConfig, OpenAIProvider};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::router::{Deployment, RuntimeBinding, UnifiedRouter};
use crate::core::types::message::MessageRole;
use crate::utils::error::gateway_error::GatewayError;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[test]
fn test_message_creation() {
    let msg = user_message("Hello, world!");
    assert_eq!(msg.role, MessageRole::User);
    if let Some(MessageContent::Text(content)) = msg.content {
        assert_eq!(content, "Hello, world!");
    } else {
        panic!("Expected text content");
    }
}

#[test]
fn test_completion_options_default() {
    let options = CompletionOptions::default();
    assert!(!options.stream);
    assert_eq!(options.extra_params.len(), 0);
}

#[test]
fn test_system_message_creation() {
    let msg = system_message("You are a helpful assistant.");
    assert_eq!(msg.role, MessageRole::System);
    if let Some(MessageContent::Text(content)) = msg.content {
        assert_eq!(content, "You are a helpful assistant.");
    } else {
        panic!("Expected text content");
    }
}

#[test]
fn test_assistant_message_creation() {
    let msg = assistant_message("I can help you with that.");
    assert_eq!(msg.role, MessageRole::Assistant);
    if let Some(MessageContent::Text(content)) = msg.content {
        assert_eq!(content, "I can help you with that.");
    } else {
        panic!("Expected text content");
    }
}

#[tokio::test]
async fn explicit_completion_facade_executes_selected_runtime_deployment() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock provider should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let upstream = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("request should arrive");
        let mut request = vec![0_u8; 8192];
        let bytes = socket
            .read(&mut request)
            .await
            .expect("request should read");
        let request = String::from_utf8_lossy(&request[..bytes]);
        assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
        assert!(request.contains(r#""model":"canonical-model""#));

        let body = r#"{"id":"chatcmpl-runtime","object":"chat.completion","created":1,"model":"canonical-model","choices":[{"index":0,"message":{"role":"assistant","content":"runtime"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("response should write");
    });

    let provider = OpenAIProvider::new(OpenAIConfig {
        base: BaseConfig {
            api_key: Some("sk-runtime-test".to_string()),
            api_base: Some(format!("http://{address}/v1")),
            endpoint_access: ProviderEndpointAccess::PrivateNetwork,
            ..Default::default()
        },
        ..Default::default()
    })
    .await
    .expect("provider should build");
    let runtime = Arc::new(UnifiedRouter::default());
    runtime.add_deployment(Deployment::new(
        "canonical-deployment".to_string(),
        Provider::OpenAI(provider),
        "canonical-model".to_string(),
        "facade-model".to_string(),
    ));
    runtime
        .add_model_alias("google/facade-alias", "facade-model")
        .expect("prefix-looking alias should publish");
    let facade = DefaultRouter::from_runtime(RuntimeBinding::new(runtime));

    let response = facade
        .complete(
            "google/facade-alias",
            vec![user_message("hello")],
            CompletionOptions::default(),
        )
        .await
        .expect("canonical runtime completion should succeed");

    assert_eq!(response.id, "chatcmpl-runtime");
    assert_eq!(response.model, "canonical-model");
    upstream.await.expect("mock provider should finish");
}

#[tokio::test]
async fn terminal_provider_error_preserves_type_and_redacts_gateway_copy() {
    let provider = OpenAIProvider::new(OpenAIConfig {
        base: BaseConfig {
            api_key: Some("sk-unused-test-key".to_string()),
            ..Default::default()
        },
        ..Default::default()
    })
    .await
    .expect("provider should build");
    let runtime = Arc::new(UnifiedRouter::default());
    runtime.add_deployment(Deployment::new(
        "authentication-deployment".to_string(),
        Provider::OpenAI(provider),
        "canonical-model".to_string(),
        "auth-model".to_string(),
    ));
    let handle = RuntimeBinding::new(runtime).bind();
    let error = handle
        .execute_with_selected_deployment_typed("auth-model", |_| async {
            Err::<((), u64), _>(ProviderError::authentication(
                "openai",
                "authorization: Bearer sk-terminal-secret",
            ))
        })
        .await
        .expect_err("typed runtime boundary should preserve authentication failure");
    let error = GatewayError::from(error);

    match error {
        GatewayError::Provider(ref error @ ProviderError::Authentication { ref message, .. }) => {
            assert_eq!(
                error.canonical_code(),
                crate::utils::error::ErrorCode::Authentication
            );
            assert!(!message.contains("sk-terminal-secret"));
            assert!(message.contains("[REDACTED]"));
        }
        other => panic!("expected typed authentication error, got {other:?}"),
    }
}

#[tokio::test]
async fn unary_completion_fails_closed_instead_of_using_request_provider_overrides() {
    let facade =
        DefaultRouter::from_runtime(RuntimeBinding::new(Arc::new(UnifiedRouter::default())));
    let error = facade
        .complete(
            "missing-model",
            vec![user_message("hello")],
            CompletionOptions {
                api_key: Some("sk-must-not-build-a-provider".to_string()),
                ..Default::default()
            },
        )
        .await
        .expect_err("request-level provider construction must fail closed");

    assert!(matches!(
        error,
        GatewayError::Provider(ProviderError::InvalidRequest { .. })
    ));
}

#[test]
fn unary_completion_source_has_no_legacy_execution_fallback() {
    let source = include_str!("default_router/router_impl.rs");
    let start = source
        .find("pub(super) async fn complete_with_runtime_handle")
        .expect("canonical unary helper should exist");
    let end = source[start..]
        .find("fn bedrock_execution_model_id")
        .map(|offset| start + offset)
        .expect("canonical unary helper should have a stable boundary");
    let unary = &source[start..end];

    for forbidden in [
        "ProviderRegistry",
        "std::env",
        "try_dynamic_provider_creation",
        "OpenAIProvider::new",
        "select_static_provider",
        "unsupported_explicit_completion_selector",
        "router_error_to_provider_error",
    ] {
        assert!(
            !unary.contains(forbidden),
            "unary completion must not contain legacy fallback: {forbidden}"
        );
    }
    assert!(unary.contains("execute_with_selected_deployment_typed"));

    let facade = include_str!("default_router/mod.rs");
    let start = facade
        .find("pub async fn completion(")
        .expect("completion free function should exist");
    let end = facade[start..]
        .find("pub async fn acompletion(")
        .map(|offset| start + offset)
        .expect("completion free function should have a stable boundary");
    let facade_unary = &facade[start..end];
    assert!(facade_unary.contains("default_runtime()"));
    assert!(!facade_unary.contains("get_global_router"));
}
