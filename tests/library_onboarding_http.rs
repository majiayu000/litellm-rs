//! Public-API regressions for existing local endpoint and streaming contracts.
use futures::StreamExt;
use litellm_rs::{
    config::models::provider::ProviderConfig,
    core::{
        completion::{CompletionOptions, DefaultRouter, Router as CompletionRouter},
        net::ProviderEndpointAccess,
        providers::anthropic::{AnthropicConfig, AnthropicProvider},
        router::{RouterConfig, RuntimeBinding, UnifiedRouter, UnifiedRoutingStrategy},
        traits::provider::llm_provider::trait_definition::LLMProvider,
        types::{chat::ChatRequest, context::RequestContext},
    },
    user_message,
};
use serde_json::{Value, json};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

async fn serve_once(
    body: String,
    content_type: &'static str,
) -> (String, JoinHandle<(String, Value)>) {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let url = format!("http://localhost:{}", listener.local_addr().unwrap().port());
    let task = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(10), async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let header_end = loop {
                let mut bytes = [0; 4096];
                let n = socket.read(&mut bytes).await.unwrap();
                assert!(n > 0, "request ended before headers");
                request.extend_from_slice(&bytes[..n]);
                if let Some(i) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    break i + 4;
                }
            };
            let headers = String::from_utf8(request[..header_end].to_vec()).unwrap();
            let length: usize = headers.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length").then(|| value.trim().parse().unwrap())
            }).unwrap();
            while request.len() < header_end + length {
                let mut bytes = [0; 4096];
                let n = socket.read(&mut bytes).await.unwrap();
                assert!(n > 0, "request ended before body");
                request.extend_from_slice(&bytes[..n]);
            }
            let parsed = serde_json::from_slice(&request[header_end..header_end + length]).unwrap();
            let response = format!("HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            // Fragment in the middle of SSE field names and JSON as well as at event boundaries.
            for bytes in response.as_bytes().chunks(13) {
                socket.write_all(bytes).await.unwrap();
            }
            (headers, parsed)
        }).await.expect("local HTTP exchange timed out")
    });
    (url, task)
}

#[tokio::test]
async fn anthropic_http_stream_retains_first_frame_input_and_cache_usage() {
    let events = [
        json!({"type":"message_start","message":{"id":"msg_local","role":"assistant","usage":{"input_tokens":10,"cache_creation_input_tokens":4,"cache_read_input_tokens":6,"output_tokens":0}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hello"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":3}}),
        json!({"type":"message_stop"}),
    ];
    let body = events
        .iter()
        .map(|event| {
            format!(
                "event: {}\ndata: {event}\n\n",
                event["type"].as_str().unwrap()
            )
        })
        .collect();
    let (url, server) = serve_once(body, "text/event-stream").await;
    let provider = AnthropicProvider::new(AnthropicConfig {
        api_key: Some("local-test-key".into()),
        base_url: url,
        endpoint_access: ProviderEndpointAccess::PrivateNetwork,
        max_retries: 0,
        ..Default::default()
    })
    .unwrap();
    let chunks: Vec<_> = provider
        .chat_completion_stream(
            ChatRequest::new("claude-sonnet-4-6").add_user_message("hello"),
            RequestContext::default(),
        )
        .await
        .unwrap()
        .collect()
        .await;
    let chunks: Vec<_> = chunks.into_iter().collect::<Result<_, _>>().unwrap();
    let usage = chunks
        .iter()
        .rev()
        .find_map(|chunk| chunk.usage.as_ref())
        .expect("terminal usage");
    assert_eq!(
        (
            usage.prompt_tokens,
            usage.completion_tokens,
            usage.total_tokens
        ),
        (20, 3, 23)
    );
    let details = usage.prompt_tokens_details.as_ref().unwrap();
    assert_eq!(details.cache_creation_tokens, Some(4));
    assert_eq!(details.cache_read_tokens, Some(6));
    assert!(chunks.iter().any(|chunk| {
        chunk
            .choices
            .iter()
            .any(|choice| choice.delta.content.as_deref() == Some("hello"))
    }));
    let (headers, request) = server.await.unwrap();
    assert!(headers.starts_with("POST /v1/messages "), "{headers}");
    assert_eq!(request["stream"], true);
}

#[tokio::test]
async fn yaml_alias_and_priority_dispatch_to_local_vllm_primary() {
    let response = json!({"id":"local","object":"chat.completion","created":1,"model":"local-model","choices":[{"index":0,"message":{"role":"assistant","content":"primary"},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":1,"total_tokens":3}});
    let (url, server) = serve_once(response.to_string(), "application/json").await;
    let fallback = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let providers: Vec<ProviderConfig> = serde_yml::from_str(&format!(
        r#"
- name: fallback
  provider_type: vllm
  api_key: ""
  base_url: http://localhost:{}/v1
  endpoint_access: private_network
  models: [local-model]
  priority: 10
- name: primary
  provider_type: vllm
  api_key: ""
  base_url: {url}/v1
  endpoint_access: private_network
  models: [local-model]
  priority: 0
"#,
        fallback.local_addr().unwrap().port()
    ))
    .unwrap();
    let aliases: HashMap<String, String> =
        serde_yml::from_str("stable-chat: production-chat\nproduction-chat: local-model\n")
            .unwrap();
    let router = UnifiedRouter::from_gateway_config_with_aliases(
        &providers,
        Some(RouterConfig {
            routing_strategy: UnifiedRoutingStrategy::PriorityBased,
            ..Default::default()
        }),
        &aliases,
    )
    .await
    .unwrap();
    let response = DefaultRouter::from_runtime(RuntimeBinding::new(Arc::new(router)))
        .complete(
            "stable-chat",
            vec![user_message("hello")],
            CompletionOptions::default(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.choices[0]
            .message
            .content
            .as_ref()
            .unwrap()
            .to_string(),
        "primary"
    );
    let (headers, request) = server.await.unwrap();
    assert!(headers.starts_with("POST /v1/chat/completions "));
    assert_eq!(request["model"], "local-model");
    assert!(
        tokio::time::timeout(Duration::from_millis(50), fallback.accept())
            .await
            .is_err()
    );
}
