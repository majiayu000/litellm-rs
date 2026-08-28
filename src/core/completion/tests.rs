//! Completion module tests

use super::*;
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::Provider;
use crate::core::providers::base::BaseConfig;
use crate::core::providers::openai::{OpenAIConfig, OpenAIProvider};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::router::{Deployment, RuntimeBinding, UnifiedRouter};
use crate::core::types::message::MessageRole;
use crate::sdk::LLMClient;
use crate::sdk::errors::SDKError;
use crate::sdk::types::{Content as SdkContent, Message as SdkMessage, Role as SdkRole};
use crate::utils::error::gateway_error::GatewayError;
use futures::StreamExt;
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
    runtime.add_deployment(
        Deployment::new(
            "canonical-deployment".to_string(),
            Provider::OpenAI(provider),
            "canonical-model".to_string(),
            "facade-model".to_string(),
        )
        .with_model_identity(Some("gpt-4".to_string()), None),
    );
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
async fn explicit_completion_stream_executes_selected_runtime_deployment() {
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
        assert!(request.contains(r#""model":"canonical-stream-model""#));
        assert!(request.contains(r#""stream":true"#));

        let body = concat!(
            "data: {\"id\":\"chatcmpl-runtime-stream\",\"object\":\"chat.completion.chunk\",\"created\":1,\"model\":\"canonical-stream-model\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"runtime-stream\"},\"finish_reason\":null}]}\n\n",
            "data: [DONE]\n\n"
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
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
            api_key: Some("sk-runtime-stream-test".to_string()),
            api_base: Some(format!("http://{address}/v1")),
            endpoint_access: ProviderEndpointAccess::PrivateNetwork,
            ..Default::default()
        },
        ..Default::default()
    })
    .await
    .expect("provider should build");
    let runtime = Arc::new(UnifiedRouter::default());
    runtime.add_deployment(
        Deployment::new(
            "canonical-stream-deployment".to_string(),
            Provider::OpenAI(provider),
            "canonical-stream-model".to_string(),
            "stream-facade-model".to_string(),
        )
        .with_model_identity(Some("gpt-4".to_string()), None),
    );
    let deployment = runtime
        .get_deployment("canonical-stream-deployment")
        .expect("stream deployment should exist");
    let facade = DefaultRouter::from_runtime(RuntimeBinding::new(runtime));

    let mut stream = facade
        .complete_stream(
            "stream-facade-model",
            vec![user_message("hello")],
            CompletionOptions::default(),
        )
        .await
        .expect("canonical runtime stream should start");
    assert_eq!(
        deployment
            .state
            .active_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    let chunk = stream
        .next()
        .await
        .expect("stream should yield a chunk")
        .expect("chunk should succeed");

    assert_eq!(chunk.id, "chatcmpl-runtime-stream");
    assert_eq!(chunk.model, "canonical-stream-model");
    assert_eq!(
        chunk.choices[0].delta.content.as_deref(),
        Some("runtime-stream")
    );
    drop(stream);
    assert_eq!(
        deployment
            .state
            .active_requests
            .load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    upstream.await.expect("mock provider should finish");
}

#[tokio::test]
async fn completion_and_sdk_facades_share_runtime_selection_and_error_categories() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("mock provider should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let upstream = tokio::spawn(async move {
        for _ in 0..2 {
            let (mut socket, _) = listener.accept().await.expect("request should arrive");
            let mut request = vec![0_u8; 8192];
            let bytes = socket
                .read(&mut request)
                .await
                .expect("request should read");
            let request = String::from_utf8_lossy(&request[..bytes]);
            assert!(request.contains(r#""model":"shared-runtime-model""#));

            let body = r#"{"id":"chatcmpl-shared","object":"chat.completion","created":1,"model":"shared-runtime-model","choices":[{"index":0,"message":{"role":"assistant","content":"shared"},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":1,"total_tokens":2}}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("response should write");
        }
    });

    let provider = OpenAIProvider::new(OpenAIConfig {
        base: BaseConfig {
            api_key: Some("sk-shared-runtime-test".to_string()),
            api_base: Some(format!("http://{address}/v1")),
            endpoint_access: ProviderEndpointAccess::PrivateNetwork,
            ..Default::default()
        },
        ..Default::default()
    })
    .await
    .expect("provider should build");
    let runtime = Arc::new(UnifiedRouter::default());
    runtime.add_deployment(
        Deployment::new(
            "shared-runtime-deployment".to_string(),
            Provider::OpenAI(provider),
            "shared-runtime-model".to_string(),
            "public-runtime-model".to_string(),
        )
        .with_model_identity(Some("gpt-4".to_string()), None),
    );
    let binding = RuntimeBinding::new(runtime);
    let completion_facade = DefaultRouter::from_runtime(binding.clone());
    let sdk_facade = LLMClient::from_runtime(binding, "public-runtime-model")
        .expect("SDK runtime facade should build");

    let completion_response = completion_facade
        .complete(
            "public-runtime-model",
            vec![user_message("hello")],
            CompletionOptions::default(),
        )
        .await
        .expect("completion facade should succeed");
    let sdk_response = sdk_facade
        .chat(vec![SdkMessage {
            role: SdkRole::User,
            content: Some(SdkContent::Text("hello".to_string())),
            name: None,
            tool_calls: None,
        }])
        .await
        .expect("SDK facade should succeed");

    assert_eq!(completion_response.model, "shared-runtime-model");
    assert_eq!(sdk_response.model, "shared-runtime-model");
    upstream.await.expect("mock provider should finish");

    let empty_binding = RuntimeBinding::new(Arc::new(UnifiedRouter::default()));
    let completion_error = DefaultRouter::from_runtime(empty_binding.clone())
        .complete(
            "missing-model",
            vec![user_message("hello")],
            CompletionOptions::default(),
        )
        .await
        .expect_err("completion must fail for an unknown model");
    let sdk_error = LLMClient::from_runtime(empty_binding, "missing-model")
        .expect("SDK runtime facade should build")
        .chat(vec![SdkMessage {
            role: SdkRole::User,
            content: Some(SdkContent::Text("hello".to_string())),
            name: None,
            tool_calls: None,
        }])
        .await
        .expect_err("SDK must fail for an unknown model");

    assert!(matches!(
        completion_error,
        GatewayError::Provider(ProviderError::ModelNotFound { .. })
    ));
    assert!(matches!(sdk_error, SDKError::ModelNotFound(_)));
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
        .find("pub(super) async fn complete_stream_with_runtime_handle")
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

    let stream_start = end;
    let stream_end = source[stream_start..]
        .find("#[async_trait]")
        .map(|offset| stream_start + offset)
        .expect("canonical stream helper should have a stable boundary");
    let stream = &source[stream_start..stream_end];
    for forbidden in [
        "ProviderRegistry",
        "std::env",
        "try_dynamic_provider_stream_creation",
        "OpenAIProvider::new",
        "select_static_provider",
    ] {
        assert!(
            !stream.contains(forbidden),
            "streaming completion must not contain legacy fallback: {forbidden}"
        );
    }
    assert!(stream.contains("select_deployment_lease_typed"));
    assert!(stream.contains("let _lease = &lease"));

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

    let stream_start = facade
        .find("pub async fn completion_stream(")
        .expect("completion_stream free function should exist");
    let stream_end = facade[stream_start..]
        .find("fn convert_chat_chunk_to_completion_chunk")
        .map(|offset| stream_start + offset)
        .expect("completion_stream should have a stable boundary");
    let facade_stream = &facade[stream_start..stream_end];
    assert!(facade_stream.contains("default_runtime()"));
    assert!(!facade_stream.contains("get_global_router"));
}
