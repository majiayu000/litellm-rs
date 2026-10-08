//! Tests for LLM client

#[cfg(test)]
use super::llm_client::LLMClient;
use super::types::{LoadBalancer, LoadBalancingStrategy, ProviderStats};
use crate::core::providers::base::sse::AnthropicUsageState;
use crate::sdk::config::{ConfigBuilder, ProviderType, SdkProviderConfig};
use crate::sdk::errors::SDKError;
use crate::sdk::types::{
    ChatOptions, Content, ContentPart, ImageUrl, Message, Role, SdkChatRequest,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

fn test_provider_config(id: &str, provider_type: ProviderType, model: &str) -> SdkProviderConfig {
    SdkProviderConfig {
        id: id.to_string(),
        provider_type,
        name: format!("{id} provider"),
        api_key: "test-key".to_string(),
        base_url: None,
        models: vec![model.to_string()],
        enabled: true,
        weight: 1.0,
        rate_limit_rpm: Some(1000),
        rate_limit_tpm: Some(10000),
        settings: HashMap::new(),
    }
}

#[tokio::test]
async fn legacy_openai_stream_requests_and_preserves_terminal_usage() {
    use futures::StreamExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for reported in [Some(5_u32), Some(0), None] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let (body_start, length) = loop {
                let mut chunk = [0; 4096];
                let count = socket.read(&mut chunk).await.unwrap();
                assert_ne!(count, 0, "complete request headers are required");
                bytes.extend_from_slice(&chunk[..count]);
                if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                    let headers = std::str::from_utf8(&bytes[..end]).unwrap();
                    assert!(headers.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .expect("JSON request has a content length");
                    break (end + 4, length);
                }
            };
            while bytes.len() < body_start + length {
                let mut chunk = [0; 4096];
                let count = socket.read(&mut chunk).await.unwrap();
                assert_ne!(count, 0, "complete request body is required");
                bytes.extend_from_slice(&chunk[..count]);
            }
            let request: serde_json::Value =
                serde_json::from_slice(&bytes[body_start..body_start + length]).unwrap();
            let content = serde_json::json!({
                "id":"legacy-stream", "model":"gpt-4o-mini", "choices":[{
                    "index":0, "delta":{"content":"ready"}, "finish_reason":null
                }]
            });
            let mut response_body = format!("data: {content}\n\n");
            // Model the provider contract: no usage event is sent unless it
            // was requested. An absent upstream count must still remain None.
            if request["stream_options"]["include_usage"] == true
                && let Some(tokens) = reported
            {
                let usage = serde_json::json!({
                    "id":"legacy-stream", "model":"gpt-4o-mini", "choices":[],
                    "usage":{"prompt_tokens":tokens,"completion_tokens":0,"total_tokens":tokens}
                });
                response_body.push_str(&format!("data: {usage}\n\n"));
            }
            response_body.push_str("data: [DONE]\n\n");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                response_body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            request
        });
        let mut provider = test_provider_config("legacy", ProviderType::OpenAI, "gpt-4o-mini");
        provider.base_url = Some(format!("http://{address}"));
        let client = LLMClient::new(ConfigBuilder::new().add_provider(provider).build()).unwrap();
        let mut stream = client.chat_stream(vec![]).await.unwrap();
        let first = stream.next().await.unwrap().unwrap();
        assert_eq!(first.choices[0].delta.content.as_deref(), Some("ready"));
        assert!(first.usage.is_none());
        if let Some(tokens) = reported {
            let terminal = stream.next().await.unwrap().unwrap();
            assert!(terminal.choices.is_empty());
            assert_eq!(terminal.usage.unwrap().total_tokens, tokens);
        }
        assert!(stream.next().await.is_none());
        let request = server.await.unwrap();
        assert_eq!(request["model"], "gpt-4o-mini");
        assert_eq!(request["stream"], true);
        assert_eq!(request["stream_options"]["include_usage"], true);
    }
}

#[tokio::test]
async fn legacy_anthropic_chat_and_stream_send_tool_history_over_http() {
    use futures::StreamExt;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for streaming in [false, true] {
        for tools in [false, true] {
            tokio::time::timeout(std::time::Duration::from_secs(10), async {
                let mut messages: Vec<Message> = serde_json::from_value(serde_json::json!([
                    {"role":"system", "content":"Be precise."},
                    {"role":"user", "content":"Weather?"},
                    {"role":"assistant", "content":"Checking."}
                ])).unwrap();
                let expected_messages = if tools {
                    messages[2].tool_calls = Some(serde_json::from_value(serde_json::json!([
                        {"id":"tool-a", "type":"function", "function":{
                            "name":"weather", "arguments":"{\"city\":\"Paris\"}"
                        }},
                        {"id":"tool-b", "type":"function", "function":{
                            "name":"weather", "arguments":"{\"city\":\"東京\"}"
                        }}
                    ])).unwrap());
                    messages.push(Message::tool_result("tool-a", "18 °C"));
                    messages.push(Message::tool_result("tool-b", "21 °C"));
                    serde_json::json!([
                        {"role":"user", "content":"Weather?"},
                        {"role":"assistant", "content":[
                            {"type":"text", "text":"Checking."},
                            {"type":"tool_use", "id":"tool-a", "name":"weather", "input":{"city":"Paris"}},
                            {"type":"tool_use", "id":"tool-b", "name":"weather", "input":{"city":"東京"}}
                        ]},
                        {"role":"user", "content":[{"type":"tool_result", "tool_use_id":"tool-a", "content":"18 °C"}]},
                        {"role":"user", "content":[{"type":"tool_result", "tool_use_id":"tool-b", "content":"21 °C"}]}
                    ])
                } else {
                    messages.push(serde_json::from_value(serde_json::json!({
                        "role":"user", "content":"Please continue."
                    })).unwrap());
                    serde_json::json!([
                        {"role":"user", "content":"Weather?"},
                        {"role":"assistant", "content":"Checking."},
                        {"role":"user", "content":"Please continue."}
                    ])
                };
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let address = listener.local_addr().unwrap();
                let server = tokio::spawn(async move {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut bytes = Vec::new();
                    let (body_start, length) = loop {
                        let mut chunk = [0; 4096];
                        let count = socket.read(&mut chunk).await.unwrap();
                        assert_ne!(count, 0, "complete request headers are required");
                        bytes.extend_from_slice(&chunk[..count]);
                        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                            let headers = std::str::from_utf8(&bytes[..end]).unwrap();
                            assert!(headers.starts_with("POST /v1/messages HTTP/1.1\r\n"));
                            let length = headers.lines().find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            }).expect("JSON request has a content length");
                            break (end + 4, length);
                        }
                    };
                    while bytes.len() < body_start + length {
                        let mut chunk = [0; 4096];
                        let count = socket.read(&mut chunk).await.unwrap();
                        assert_ne!(count, 0, "complete request body is required");
                        bytes.extend_from_slice(&chunk[..count]);
                    }
                    let request: serde_json::Value = serde_json::from_slice(
                        &bytes[body_start..body_start + length]
                    ).unwrap();
                    // Reply only after the actual legacy adapter sent the complete
                    // paired tool turn (or the unchanged ordinary text control).
                    assert_eq!(request["messages"], expected_messages);
                    assert_eq!(request["system"], "Be precise.");
                    assert_eq!(request["model"], "claude-sonnet-4-6");
                    assert_eq!(request["stream"].as_bool().unwrap_or(false), streaming);
                    let (content_type, response_body) = if streaming {
                        ("text/event-stream", concat!(
                            "event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ready\"}}\n\n",
                            "event: message_delta\ndata: {\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":1}}\n\n",
                            "event: message_stop\ndata: {}\n\n"
                        ).to_string())
                    } else {
                        ("application/json", serde_json::json!({
                            "id":"legacy-tool-reply", "content":[{"type":"text", "text":"ready"}],
                            "stop_reason":"end_turn", "usage":{"input_tokens":1,"output_tokens":1}
                        }).to_string())
                    };
                    socket.write_all(format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                        response_body.len()
                    ).as_bytes()).await.unwrap();
                });
                let mut provider = test_provider_config("legacy", ProviderType::Anthropic, "claude-sonnet-4-6");
                provider.base_url = Some(format!("http://{address}"));
                let client = LLMClient::new(ConfigBuilder::new().add_provider(provider).build()).unwrap();
                if streaming {
                    let mut stream = client.chat_stream(messages).await.unwrap();
                    let mut text = String::new();
                    while let Some(chunk) = stream.next().await {
                        for choice in chunk.unwrap().choices {
                            if let Some(content) = choice.delta.content {
                                text.push_str(&content);
                            }
                        }
                    }
                    assert_eq!(text, "ready");
                } else {
                    let response = client.chat(messages).await.unwrap();
                    assert!(matches!(&response.choices[0].message.content, Some(Content::Text(text)) if text == "ready"));
                }
                server.await.unwrap();
            }).await.expect("the local legacy Anthropic exchange must complete");
        }
    }
}

#[tokio::test]
async fn legacy_anthropic_tool_responses_round_trip_over_http() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    for with_text in [false, true] {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let mut assistant_content = Vec::new();
                if with_text {
                    assistant_content.push(serde_json::json!({"type":"text", "text":"Checking."}));
                }
                assistant_content.extend([
                    serde_json::json!({
                        "type":"tool_use", "id":"tool-a", "name":"weather", "input":{"city":"Paris"}
                    }),
                    serde_json::json!({
                        "type":"tool_use", "id":"tool-b", "name":"forecast", "input":{
                            "city":"東京", "options":{"days":[1,2], "metric":true}, "note":null
                        }
                    }),
                ]);
                for turn in 0..2 {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut bytes = Vec::new();
                    let (body_start, length) = loop {
                        let mut chunk = [0; 4096];
                        let count = socket.read(&mut chunk).await.unwrap();
                        assert_ne!(count, 0, "complete request headers are required");
                        bytes.extend_from_slice(&chunk[..count]);
                        if let Some(end) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                            let headers = std::str::from_utf8(&bytes[..end]).unwrap();
                            assert!(headers.starts_with("POST /v1/messages HTTP/1.1\r\n"));
                            let length = headers.lines().find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().unwrap())
                            }).expect("JSON request has a content length");
                            break (end + 4, length);
                        }
                    };
                    while bytes.len() < body_start + length {
                        let mut chunk = [0; 4096];
                        let count = socket.read(&mut chunk).await.unwrap();
                        assert_ne!(count, 0, "complete request body is required");
                        bytes.extend_from_slice(&chunk[..count]);
                    }
                    let request: serde_json::Value = serde_json::from_slice(
                        &bytes[body_start..body_start + length]
                    ).unwrap();
                    assert_eq!(request["system"], "Be precise.");
                    assert_eq!(request["model"], "claude-sonnet-4-6");
                    let response = if turn == 0 {
                        assert_eq!(request["messages"], serde_json::json!([
                            {"role":"user", "content":"Weather?"}
                        ]));
                        serde_json::json!({
                            "id":"tool-turn", "content":assistant_content,
                            "stop_reason":"tool_use", "usage":{"input_tokens":2,"output_tokens":3}
                        })
                    } else {
                        // The second request uses the actual SDK response message,
                        // so losing IDs, names, inputs, or text breaks this exchange.
                        assert_eq!(request["messages"], serde_json::json!([
                            {"role":"user", "content":"Weather?"},
                            {"role":"assistant", "content":assistant_content},
                            {"role":"user", "content":[{
                                "type":"tool_result", "tool_use_id":"tool-a", "content":"18 °C"
                            }]},
                            {"role":"user", "content":[{
                                "type":"tool_result", "tool_use_id":"tool-b", "content":"21 °C"
                            }]}
                        ]));
                        serde_json::json!({
                            "id":"final-turn", "content":[{"type":"text", "text":"Paris 18 °C; 東京 21 °C."}],
                            "stop_reason":"end_turn", "usage":{"input_tokens":4,"output_tokens":2}
                        })
                    }.to_string();
                    socket.write_all(format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",
                        response.len()
                    ).as_bytes()).await.unwrap();
                }
            });
            let mut provider = test_provider_config("legacy", ProviderType::Anthropic, "claude-sonnet-4-6");
            provider.base_url = Some(format!("http://{address}"));
            let client = LLMClient::new(ConfigBuilder::new().add_provider(provider).build()).unwrap();
            let mut messages: Vec<Message> = serde_json::from_value(serde_json::json!([
                {"role":"system", "content":"Be precise."},
                {"role":"user", "content":"Weather?"}
            ])).unwrap();
            let first = client.chat(messages.clone()).await.unwrap();
            assert_eq!(first.choices[0].finish_reason.as_deref(), Some("tool_calls"));
            assert_eq!(first.usage.total_tokens, 5);
            let assistant = &first.choices[0].message;
            assert_eq!(assistant.role, Role::Assistant);
            if with_text {
                assert!(matches!(&assistant.content, Some(Content::Text(text)) if text == "Checking."));
            } else {
                assert!(assistant.content.is_none());
            }
            let calls = assistant.tool_calls.as_ref().unwrap();
            assert_eq!(calls.len(), 2);
            for (call, id, name, input) in [
                (&calls[0], "tool-a", "weather", serde_json::json!({"city":"Paris"})),
                (&calls[1], "tool-b", "forecast", serde_json::json!({
                    "city":"東京", "options":{"days":[1,2], "metric":true}, "note":null
                })),
            ] {
                assert_eq!(call.id, id);
                assert_eq!(call.tool_type, "function");
                assert_eq!(call.function.name, name);
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(call.function.arguments.as_deref().unwrap()).unwrap(),
                    input
                );
            }
            messages.push(assistant.clone());
            messages.push(Message::tool_result(calls[0].id.clone(), "18 °C"));
            messages.push(Message::tool_result(calls[1].id.clone(), "21 °C"));
            let final_response = client.chat(messages).await.unwrap();
            assert_eq!(final_response.choices[0].finish_reason.as_deref(), Some("stop"));
            assert!(final_response.choices[0].message.tool_calls.is_none());
            assert!(matches!(
                &final_response.choices[0].message.content,
                Some(Content::Text(text)) if text == "Paris 18 °C; 東京 21 °C."
            ));
            assert_eq!(final_response.usage.total_tokens, 6);
            server.await.unwrap();
        }).await.expect("both local legacy Anthropic turns must complete");
    }
}

#[tokio::test]
async fn test_llm_client_creation() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "test",
            ProviderType::OpenAI,
            "gpt-3.5-turbo",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    assert_eq!(client.list_providers().len(), 1);
}

#[tokio::test]
async fn test_provider_selection() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "anthropic",
            ProviderType::Anthropic,
            "claude-3-sonnet-20240229",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();

    let request = SdkChatRequest {
        model: "claude-3-sonnet-20240229".to_string(),
        messages: vec![],
        options: ChatOptions::default(),
    };

    let provider = client.select_provider(&request).await.unwrap();
    assert_eq!(provider.id, "anthropic");
}

#[tokio::test]
async fn test_provider_selection_accepts_anthropic_haiku_preset_aliases() {
    let config = ConfigBuilder::new()
        .add_anthropic("anthropic", "test-key")
        .build();
    let client = LLMClient::new(config).unwrap();

    assert!(
        client
            .select_provider(&SdkChatRequest {
                model: "claude-3-5-haiku-20241022".to_string(),
                messages: vec![],
                options: ChatOptions::default(),
            })
            .await
            .is_err()
    );
    for model in ["claude-haiku-4-5", "claude-haiku-4-5-20251001"] {
        let provider = client
            .select_provider(&SdkChatRequest {
                model: model.to_string(),
                messages: vec![],
                options: ChatOptions::default(),
            })
            .await
            .unwrap();

        assert_eq!(provider.id, "anthropic");
    }
}

#[tokio::test]
async fn test_stream_provider_selection_prefers_default_provider() {
    let config = ConfigBuilder::new()
        .default_provider("openai")
        .add_provider(test_provider_config(
            "anthropic",
            ProviderType::Anthropic,
            "claude-3-sonnet-20240229",
        ))
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-4o-mini",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.select_provider_for_stream(&[]).await.unwrap();

    assert_eq!(provider.id, "openai");
}

#[tokio::test]
async fn test_sdk_load_balancer_uses_core_round_robin_strategy_for_model() {
    let providers = vec![
        test_provider_config("openai-a", ProviderType::OpenAI, "gpt-shared"),
        test_provider_config("openai-b", ProviderType::OpenAI, "gpt-shared"),
    ];
    let stats = Arc::new(RwLock::new(HashMap::<String, ProviderStats>::new()));
    let load_balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);

    let first = load_balancer
        .select_chat_provider(&providers, &stats, Some("gpt-shared"))
        .await
        .unwrap();
    let second = load_balancer
        .select_chat_provider(&providers, &stats, Some("gpt-shared"))
        .await
        .unwrap();
    let other_model = load_balancer
        .select_chat_provider(&providers, &stats, Some("missing"))
        .await
        .unwrap_err();

    assert_eq!(first.id, "openai-a");
    assert_eq!(second.id, "openai-b");
    assert!(matches!(other_model, SDKError::ModelNotFound(_)));
}

#[tokio::test]
async fn test_sdk_load_balancer_maps_health_to_core_priority_strategy() {
    let providers = vec![
        test_provider_config("slow-errors", ProviderType::OpenAI, "gpt-shared"),
        test_provider_config("healthy", ProviderType::OpenAI, "gpt-shared"),
    ];
    let stats = Arc::new(RwLock::new(HashMap::from([
        (
            "slow-errors".to_string(),
            ProviderStats {
                health_score: 0.25,
                ..ProviderStats::default()
            },
        ),
        (
            "healthy".to_string(),
            ProviderStats {
                health_score: 0.98,
                ..ProviderStats::default()
            },
        ),
    ])));
    let load_balancer = LoadBalancer::new(LoadBalancingStrategy::HealthBased);

    let selected = load_balancer
        .select_chat_provider(&providers, &stats, Some("gpt-shared"))
        .await
        .unwrap();

    assert_eq!(selected.id, "healthy");
}

#[tokio::test]
async fn test_sdk_load_balancer_maps_latency_to_core_latency_strategy() {
    let providers = vec![
        test_provider_config("slow", ProviderType::OpenAI, "gpt-shared"),
        test_provider_config("fast", ProviderType::OpenAI, "gpt-shared"),
    ];
    let stats = Arc::new(RwLock::new(HashMap::from([
        (
            "slow".to_string(),
            ProviderStats {
                avg_latency_ms: 250.0,
                ..ProviderStats::default()
            },
        ),
        (
            "fast".to_string(),
            ProviderStats {
                avg_latency_ms: 25.0,
                ..ProviderStats::default()
            },
        ),
    ])));
    let load_balancer = LoadBalancer::new(LoadBalancingStrategy::LeastLatency);

    let selected = load_balancer
        .select_chat_provider(&providers, &stats, Some("gpt-shared"))
        .await
        .unwrap();

    assert_eq!(selected.id, "fast");
}

#[tokio::test]
async fn test_sdk_chat_routing_skips_non_chat_provider_with_matching_model() {
    let providers = vec![
        test_provider_config("azure", ProviderType::Azure, "gpt-shared"),
        test_provider_config("openai", ProviderType::OpenAI, "gpt-shared"),
    ];
    let stats = Arc::new(RwLock::new(HashMap::<String, ProviderStats>::new()));
    let load_balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);

    let selected = load_balancer
        .select_chat_provider(&providers, &stats, Some("gpt-shared"))
        .await
        .unwrap();

    assert_eq!(selected.id, "openai");
}

#[tokio::test]
async fn test_sdk_chat_routing_rejects_google_as_unsupported() {
    let providers = vec![test_provider_config(
        "google",
        ProviderType::Google,
        "gemini-pro",
    )];
    let stats = Arc::new(RwLock::new(HashMap::<String, ProviderStats>::new()));
    let load_balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);

    let err = load_balancer
        .select_chat_provider(&providers, &stats, Some("gemini-pro"))
        .await
        .unwrap_err();

    assert!(
        matches!(err, SDKError::NotSupported(ref message) if message.contains("google")),
        "expected Google SDK chat to be explicitly unsupported, got {err:?}"
    );
}

#[tokio::test]
async fn test_sdk_stream_routing_skips_non_stream_provider() {
    let providers = vec![
        test_provider_config("google", ProviderType::Google, "gemini-pro"),
        test_provider_config("openai", ProviderType::OpenAI, "gpt-4o-mini"),
    ];
    let stats = Arc::new(RwLock::new(HashMap::<String, ProviderStats>::new()));
    let load_balancer = LoadBalancer::new(LoadBalancingStrategy::RoundRobin);

    let selected = load_balancer
        .select_stream_provider(&providers, &stats)
        .await
        .unwrap();

    assert_eq!(selected.id, "openai");
}

#[tokio::test]
async fn test_provider_selection_rejects_unsupported_default_provider() {
    let config = ConfigBuilder::new()
        .default_provider("google")
        .add_provider(test_provider_config(
            "google",
            ProviderType::Google,
            "gemini-pro",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let err = client
        .select_provider(&SdkChatRequest {
            model: String::new(),
            messages: Vec::new(),
            options: ChatOptions::default(),
        })
        .await
        .unwrap_err();

    assert!(
        matches!(err, SDKError::NotSupported(ref message) if message.contains("google")),
        "expected unsupported default provider error, got {err:?}"
    );
}

#[tokio::test]
async fn test_provider_selection_prefers_default_provider_when_model_unspecified() {
    let config = ConfigBuilder::new()
        .default_provider("openai")
        .add_provider(test_provider_config(
            "anthropic",
            ProviderType::Anthropic,
            "claude-3-sonnet-20240229",
        ))
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-4o-mini",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client
        .select_provider(&SdkChatRequest {
            model: String::new(),
            messages: Vec::new(),
            options: ChatOptions::default(),
        })
        .await
        .unwrap();

    assert_eq!(provider.id, "openai");
}

#[test]
fn test_provider_config_lookup() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-3.5-turbo",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(provider.id, "openai");
    assert_eq!(provider.models, vec!["gpt-3.5-turbo".to_string()]);
}

#[test]
fn test_provider_config_lookup_missing_provider() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-3.5-turbo",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let err = client.provider_config("missing").unwrap_err();

    assert!(matches!(err, SDKError::ProviderNotFound(id) if id == "missing"));
}

#[test]
fn test_provider_default_model_uses_first_configured_model() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-4o-mini",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(
        client.provider_default_model(provider, "fallback-model"),
        "gpt-4o-mini"
    );
}

#[test]
fn test_provider_default_model_falls_back_without_allocating_config_value() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            models: Vec::new(),
            ..test_provider_config("openai", ProviderType::OpenAI, "unused")
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(
        client.provider_default_model(provider, "fallback-model"),
        "fallback-model"
    );
}

#[test]
fn test_provider_base_url_uses_configured_value() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            base_url: Some("https://example.com/custom".to_string()),
            ..test_provider_config("openai", ProviderType::OpenAI, "gpt-4o-mini")
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(
        client.provider_base_url(provider, "https://fallback.example"),
        "https://example.com/custom"
    );
}

#[test]
fn test_provider_base_url_falls_back_to_default() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-4o-mini",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(
        client.provider_base_url(provider, "https://fallback.example"),
        "https://fallback.example"
    );
}

#[test]
fn test_provider_endpoint_uses_shared_url_joining() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "openai",
            ProviderType::OpenAI,
            "gpt-4o-mini",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("openai").unwrap();

    assert_eq!(
        client.provider_endpoint(provider, "https://api.openai.com/", "/v1/chat/completions"),
        "https://api.openai.com/v1/chat/completions"
    );
}

#[tokio::test]
async fn test_execute_chat_request_anthropic_plain_url_image_returns_invalid_request() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "anthropic",
            ProviderType::Anthropic,
            "claude-sonnet-4-5",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let request = SdkChatRequest {
        model: String::new(),
        messages: vec![Message {
            tool_call_id: None,
            role: Role::User,
            content: Some(Content::Multimodal(vec![ContentPart::Image {
                image_url: ImageUrl {
                    url: "https://example.com/photo.jpg".to_string(),
                    detail: None,
                },
            }])),
            name: None,
            tool_calls: None,
        }],
        options: ChatOptions::default(),
    };

    let err = client
        .execute_chat_request("anthropic", request)
        .await
        .unwrap_err();
    assert!(
        matches!(err, SDKError::InvalidRequest(_)),
        "expected InvalidRequest, got {err:?}"
    );
}

#[tokio::test]
async fn test_execute_chat_request_anthropic_malformed_data_uri_returns_invalid_request() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "anthropic",
            ProviderType::Anthropic,
            "claude-sonnet-4-5",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let request = SdkChatRequest {
        model: String::new(),
        messages: vec![Message {
            tool_call_id: None,
            role: Role::User,
            content: Some(Content::Multimodal(vec![ContentPart::Image {
                image_url: ImageUrl {
                    url: "data:image/png;base64,!!!invalid!!!".to_string(),
                    detail: None,
                },
            }])),
            name: None,
            tool_calls: None,
        }],
        options: ChatOptions::default(),
    };

    let err = client
        .execute_chat_request("anthropic", request)
        .await
        .unwrap_err();
    assert!(
        matches!(err, SDKError::InvalidRequest(_)),
        "expected InvalidRequest, got {err:?}"
    );
}

#[tokio::test]
async fn test_execute_chat_request_google_returns_not_supported() {
    let config = ConfigBuilder::new()
        .add_provider(test_provider_config(
            "google",
            ProviderType::Google,
            "gemini-pro",
        ))
        .build();

    let client = LLMClient::new(config).unwrap();
    let request = SdkChatRequest {
        model: "gemini-pro".to_string(),
        messages: vec![],
        options: ChatOptions::default(),
    };

    let err = client
        .execute_chat_request("google", request)
        .await
        .unwrap_err();
    assert!(
        matches!(err, SDKError::NotSupported(ref message) if message.contains("google")),
        "expected Google SDK chat to be NotSupported, got {err:?}"
    );
}

#[test]
fn test_anthropic_messages_endpoint_avoids_duplicate_v1() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            base_url: Some("https://api.anthropic.com/v1".to_string()),
            ..test_provider_config("anthropic", ProviderType::Anthropic, "claude-sonnet-4-6")
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let provider = client.provider_config("anthropic").unwrap();

    assert_eq!(
        client.anthropic_messages_endpoint(provider),
        "https://api.anthropic.com/v1/messages"
    );
}

// ==================== Streaming Tests ====================

#[tokio::test]
async fn test_stream_azure_unsupported() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            id: "azure-test".to_string(),
            provider_type: ProviderType::Azure,
            name: "Azure".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some("https://my-resource.openai.azure.com".to_string()),
            models: vec!["gpt-4".to_string()],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(1000),
            rate_limit_tpm: Some(10000),
            settings: HashMap::new(),
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let result = client.execute_stream_request("azure-test", vec![]).await;
    assert!(matches!(result, Err(SDKError::NotSupported(_))));
}

#[tokio::test]
async fn test_stream_unsupported_provider() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            id: "google-test".to_string(),
            provider_type: ProviderType::Google,
            name: "Google".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            models: vec!["gemini-pro".to_string()],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(1000),
            rate_limit_tpm: Some(10000),
            settings: HashMap::new(),
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let result = client.execute_stream_request("google-test", vec![]).await;
    assert!(matches!(result, Err(SDKError::NotSupported(_))));
}

#[tokio::test]
async fn test_stream_provider_not_found() {
    let config = ConfigBuilder::new()
        .add_provider(SdkProviderConfig {
            id: "openai-test".to_string(),
            provider_type: ProviderType::OpenAI,
            name: "OpenAI".to_string(),
            api_key: "test-key".to_string(),
            base_url: None,
            models: vec!["gpt-4".to_string()],
            enabled: true,
            weight: 1.0,
            rate_limit_rpm: Some(1000),
            rate_limit_tpm: Some(10000),
            settings: HashMap::new(),
        })
        .build();

    let client = LLMClient::new(config).unwrap();
    let result = client.execute_stream_request("nonexistent", vec![]).await;
    assert!(matches!(result, Err(SDKError::ProviderNotFound(_))));
}

#[test]
fn test_parse_openai_sse_line_content() {
    use super::completions::parse_openai_sse_line;

    let line = r#"data: {"id":"chatcmpl-abc","model":"gpt-4","choices":[{"index":0,"delta":{"role":null,"content":"Hello","tool_calls":null},"finish_reason":null}]}"#;
    let result = parse_openai_sse_line(line);
    assert!(result.is_some());
    let chunk = result.unwrap().unwrap();
    assert_eq!(chunk.id, "chatcmpl-abc");
    assert_eq!(chunk.model, "gpt-4");
    assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
}

#[test]
fn test_parse_openai_sse_line_done() {
    use super::completions::parse_openai_sse_line;

    let result = parse_openai_sse_line("data: [DONE]");
    assert!(result.is_none());
}

#[test]
fn test_parse_openai_sse_line_malformed() {
    use super::completions::parse_openai_sse_line;

    let result = parse_openai_sse_line("data: {not valid json}");
    assert!(result.is_some());
    assert!(matches!(result.unwrap(), Err(SDKError::ParseError(_))));
}

#[test]
fn test_parse_anthropic_sse_record_delta() {
    use super::completions::parse_anthropic_sse_record;

    let data =
        r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}"#;
    let result = parse_anthropic_sse_record(
        "content_block_delta",
        data,
        None,
        &mut AnthropicUsageState::default(),
    );
    assert!(result.is_some());
    let chunk = result.unwrap().unwrap();
    assert_eq!(chunk.choices[0].delta.content, Some("Hello".to_string()));
}

#[test]
fn test_parse_anthropic_sse_record_stop() {
    use super::completions::parse_anthropic_sse_record;

    let result = parse_anthropic_sse_record(
        "message_stop",
        r#"{"type":"message_stop"}"#,
        None,
        &mut AnthropicUsageState::default(),
    );
    assert!(result.is_none());
}

#[test]
fn test_parse_anthropic_sse_record_message_delta_end_turn_maps_to_stop() {
    use super::completions::parse_anthropic_sse_record;

    let mut usage_state = AnthropicUsageState::default();
    assert!(
        parse_anthropic_sse_record(
            "message_start",
            r#"{"type":"message_start","message":{"usage":{"input_tokens":10,"cache_creation_input_tokens":4,"cache_read_input_tokens":6,"output_tokens":0}}}"#,
            None,
            &mut usage_state,
        )
        .is_none()
    );
    let text = parse_anthropic_sse_record(
        "content_block_delta",
        r#"{"delta":{"type":"text_delta","text":"Hello"}}"#,
        None,
        &mut usage_state,
    )
    .unwrap()
    .unwrap();
    assert_eq!(text.choices[0].delta.content.as_deref(), Some("Hello"));
    assert!(text.usage.is_none());
    let intermediate = parse_anthropic_sse_record(
        "message_delta",
        r#"{"delta":{"stop_reason":null},"usage":{"output_tokens":1,"cache_creation_input_tokens":4,"cache_read_input_tokens":6}}"#,
        None,
        &mut usage_state,
    )
    .unwrap()
    .unwrap();
    assert!(intermediate.usage.is_none());
    let data = r#"{"type":"message_delta","delta":{"stop_reason":"end_turn","stop_sequence":null},"usage":{"output_tokens":3}}"#;
    let chunk = parse_anthropic_sse_record("message_delta", data, None, &mut usage_state)
        .unwrap()
        .unwrap();
    assert_eq!(chunk.choices[0].finish_reason, Some("stop".to_string()));
    let usage = chunk.usage.unwrap();
    assert_eq!(usage.prompt_tokens, 20);
    assert_eq!(usage.completion_tokens, 3);
    assert_eq!(usage.total_tokens, 23);
    let details = usage.prompt_tokens_details.unwrap();
    assert_eq!(details.cache_creation_tokens, Some(4));
    assert_eq!(details.cache_read_tokens, Some(6));
    assert_eq!(details.cached_tokens, Some(6));
}

#[test]
fn test_parse_anthropic_sse_record_message_delta_max_tokens_maps_to_length() {
    use super::completions::parse_anthropic_sse_record;

    let data = r#"{"type":"message_delta","delta":{"stop_reason":"max_tokens","stop_sequence":null},"usage":{"output_tokens":100}}"#;
    let chunk = parse_anthropic_sse_record(
        "message_delta",
        data,
        None,
        &mut AnthropicUsageState::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(chunk.choices[0].finish_reason, Some("length".to_string()));
    assert!(chunk.usage.is_none());
}

#[test]
fn test_parse_anthropic_sse_record_message_delta_tool_use_maps_to_tool_calls() {
    use super::completions::parse_anthropic_sse_record;

    let data = r#"{"type":"message_delta","delta":{"stop_reason":"tool_use","stop_sequence":null},"usage":{"output_tokens":5}}"#;
    let chunk = parse_anthropic_sse_record(
        "message_delta",
        data,
        None,
        &mut AnthropicUsageState::default(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        chunk.choices[0].finish_reason,
        Some("tool_calls".to_string())
    );
}

#[test]
fn legacy_openai_sse_preserves_partial_tool_arguments_and_usage() {
    let line = r#"data: {"id":"tools","model":"gpt-4","choices":[{"index":0,"delta":{"tool_calls":[{"index":1,"function":{"arguments":"Paris"}}]}}]}"#;
    let chunk = super::completions::parse_openai_sse_line(line)
        .unwrap()
        .unwrap();
    let delta = &chunk.choices[0].delta.tool_calls.as_ref().unwrap()[0];
    assert_eq!(delta.index, 1);
    assert!(delta.id.is_none());
    let function = delta.function.as_ref().unwrap();
    assert!(function.name.is_none());
    assert_eq!(function.arguments.as_deref(), Some("Paris"));

    let line = r#"data: {"id":"tools","model":"gpt-4","choices":[],"usage":{"prompt_tokens":4,"completion_tokens":8,"total_tokens":12}}"#;
    let chunk = super::completions::parse_openai_sse_line(line)
        .unwrap()
        .unwrap();
    assert!(chunk.choices.is_empty());
    assert_eq!(chunk.usage.unwrap().total_tokens, 12);
}

#[test]
fn legacy_anthropic_tool_delta_keeps_its_block_index_and_identity() {
    let chunk = super::completions::parse_anthropic_sse_record(
        "content_block_delta",
        r#"{"index":2,"delta":{"type":"input_json_delta","partial_json":"Paris"}}"#,
        Some(("call-weather", "weather")),
        &mut AnthropicUsageState::default(),
    )
    .unwrap()
    .unwrap();
    let delta = &chunk.choices[0].delta.tool_calls.as_ref().unwrap()[0];
    assert_eq!(delta.index, 2);
    assert_eq!(delta.id.as_deref(), Some("call-weather"));
    assert_eq!(
        delta.function.as_ref().unwrap().name.as_deref(),
        Some("weather")
    );
    assert_eq!(
        delta.function.as_ref().unwrap().arguments.as_deref(),
        Some("Paris")
    );
}

#[tokio::test]
async fn legacy_stream_with_options_fails_explicitly_before_transport() {
    let client = LLMClient::new(
        ConfigBuilder::new()
            .add_provider(test_provider_config(
                "openai",
                ProviderType::OpenAI,
                "gpt-4",
            ))
            .build(),
    )
    .unwrap();
    let result = client
        .chat_stream_with_options(SdkChatRequest {
            model: "gpt-4".into(),
            messages: Vec::new(),
            options: ChatOptions::default(),
        })
        .await;
    assert!(matches!(result, Err(SDKError::NotSupported(_))));
}

#[test]
fn test_parse_anthropic_sse_record_ignored_events() {
    use super::completions::parse_anthropic_sse_record;

    let result = parse_anthropic_sse_record(
        "message_start",
        r#"{"type":"message_start","message":{"id":"msg_1","model":"claude-3"}}"#,
        None,
        &mut AnthropicUsageState::default(),
    );
    assert!(result.is_none());

    let result =
        parse_anthropic_sse_record("ping", r#"{}"#, None, &mut AnthropicUsageState::default());
    assert!(result.is_none());
    let result = parse_anthropic_sse_record(
        "message_start",
        "{malformed",
        None,
        &mut AnthropicUsageState::default(),
    );
    assert!(result.is_none());
}

#[test]
fn legacy_anthropic_usage_stays_unknown_without_complete_trusted_counts() {
    use super::completions::parse_anthropic_sse_record;

    for (start, terminal) in [
        (
            r#"{"message":{}}"#,
            r#"{"delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":10}}"#,
        ),
        (
            r#"{"message":{"usage":{"input_tokens":4}}}"#,
            r#"{"delta":{"stop_reason":"end_turn"}}"#,
        ),
        (
            r#"{"message":{"usage":{"input_tokens":-1}}}"#,
            r#"{"delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":10}}"#,
        ),
        (
            r#"{"message":{"usage":{"input_tokens":4294967295}}}"#,
            r#"{"delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}"#,
        ),
        (
            r#"{"message":{"usage":{"cache_creation_input_tokens":4,"cache_read_input_tokens":6}}}"#,
            r#"{"delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":3}}"#,
        ),
        (
            r#"{"message":{"usage":{"input_tokens":10,"output_tokens":2}}}"#,
            r#"{"delta":{"stop_reason":"end_turn"},"usage":{"cache_read_input_tokens":6}}"#,
        ),
        (
            r#"{"message":{"usage":{"input_tokens":10,"cache_read_input_tokens":null}}}"#,
            r#"{"delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":3}}"#,
        ),
        (
            r#"{"message":{"usage":{"input_tokens":4294967294,"cache_creation_input_tokens":2}}}"#,
            r#"{"delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":0}}"#,
        ),
    ] {
        let mut usage_state = AnthropicUsageState::default();
        assert!(
            parse_anthropic_sse_record("message_start", start, None, &mut usage_state).is_none()
        );
        let chunk = parse_anthropic_sse_record("message_delta", terminal, None, &mut usage_state)
            .unwrap()
            .unwrap();
        assert_eq!(chunk.choices[0].finish_reason.as_deref(), Some("stop"));
        assert!(chunk.usage.is_none(), "start={start}, terminal={terminal}");
    }

    for (start, terminal, cache_reported) in [
        (
            r#"{"message":{"usage":{"input_tokens":0}}}"#,
            r#"{"delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":0}}"#,
            false,
        ),
        (
            r#"{"message":{"usage":{"input_tokens":10,"cache_creation_input_tokens":4,"cache_read_input_tokens":6}}}"#,
            r#"{"delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":0,"output_tokens":0,"cache_creation_input_tokens":0,"cache_read_input_tokens":0}}"#,
            true,
        ),
    ] {
        let mut usage_state = AnthropicUsageState::default();
        assert!(
            parse_anthropic_sse_record("message_start", start, None, &mut usage_state).is_none()
        );
        let usage = parse_anthropic_sse_record("message_delta", terminal, None, &mut usage_state)
            .unwrap()
            .unwrap()
            .usage
            .unwrap();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
        if cache_reported {
            let details = usage.prompt_tokens_details.unwrap();
            assert_eq!(details.cache_creation_tokens, Some(0));
            assert_eq!(details.cache_read_tokens, Some(0));
            assert_eq!(details.cached_tokens, Some(0));
        } else {
            assert!(usage.prompt_tokens_details.is_none());
        }
    }
}

#[test]
fn legacy_anthropic_usage_resets_cache_and_invalid_state_for_next_message() {
    use super::completions::parse_anthropic_sse_record;

    for invalid_intermediate in [false, true] {
        let mut usage_state = AnthropicUsageState::default();
        assert!(
            parse_anthropic_sse_record(
                "message_start",
                r#"{"message":{"usage":{"input_tokens":10,"cache_creation_input_tokens":4,"cache_read_input_tokens":6}}}"#,
                None,
                &mut usage_state,
            )
            .is_none()
        );
        let intermediate_usage = if invalid_intermediate {
            r#"{"usage":{"cache_read_input_tokens":-1}}"#
        } else {
            r#"{"usage":{"cache_read_input_tokens":6,"output_tokens":1}}"#
        };
        let intermediate =
            parse_anthropic_sse_record("message_delta", intermediate_usage, None, &mut usage_state)
                .unwrap()
                .unwrap();
        assert!(intermediate.usage.is_none());
        let terminal = parse_anthropic_sse_record(
            "message_delta",
            r#"{"delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":10,"cache_read_input_tokens":6,"output_tokens":3}}"#,
            None,
            &mut usage_state,
        )
        .unwrap()
        .unwrap();
        assert_eq!(terminal.choices[0].finish_reason.as_deref(), Some("stop"));
        if invalid_intermediate {
            assert!(
                terminal.usage.is_none(),
                "invalid usage must remain unknown"
            );
        } else {
            assert_eq!(terminal.usage.unwrap().total_tokens, 23);
        }

        assert!(
            parse_anthropic_sse_record(
                "message_start",
                r#"{"message":{"usage":{"input_tokens":4}}}"#,
                None,
                &mut usage_state,
            )
            .is_none()
        );
        let next = parse_anthropic_sse_record(
            "message_delta",
            r#"{"delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":1}}"#,
            None,
            &mut usage_state,
        )
        .unwrap()
        .unwrap()
        .usage
        .expect("new message resets previous invalid/cache state");
        assert_eq!(next.prompt_tokens, 4);
        assert_eq!(next.completion_tokens, 1);
        assert_eq!(next.total_tokens, 5);
        assert!(next.prompt_tokens_details.is_none());
    }
}
