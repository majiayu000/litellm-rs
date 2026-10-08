//! Public SDK consumers must be able to finish complete and streamed tool turns.
use futures::{StreamExt, future::BoxFuture, stream::BoxStream};
use litellm_rs::{
    core::{
        providers::{ExternalProvider, Provider, ProviderError},
        router::{Deployment, RuntimeBinding, UnifiedRouter},
        types::{
            chat::ChatRequest,
            context::RequestContext,
            model::{ModelInfo, ProviderCapability},
            responses::{ChatChunk as CoreChunk, ChatResponse as CoreResponse},
        },
    },
    sdk::{
        LLMClient,
        errors::SDKError,
        types::{Content, Message, Role, SdkChatRequest, ToolChoice},
    },
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, atomic::Ordering},
};

#[derive(Debug)]
struct ToolProvider {
    models: Vec<ModelInfo>,
    requests: Arc<Mutex<Vec<ChatRequest>>>,
    stream_error: bool,
}

fn tool_call(index: u32, id: &str, name: &str, arguments: &str) -> Value {
    json!({"index":index,"id":id,"type":"function",
        "function":{"name":name,"arguments":arguments}})
}

fn chunk(choices: Value, usage: Value) -> CoreChunk {
    serde_json::from_value(json!({
        "id":"tool-stream","object":"chat.completion.chunk","created":1,
        "model":"private-model","choices":choices,"usage":usage
    }))
    .unwrap()
}

impl ExternalProvider for ToolProvider {
    fn name(&self) -> &str {
        "sdk-tools-test"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
        ]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn chat_completion(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<CoreResponse, ProviderError>> {
        Box::pin(async move {
            assert_eq!(request.model, "private-model");
            let is_result = request.messages.last().unwrap().tool_call_id.is_some();
            self.requests.lock().unwrap().push(request);
            let (message, reason) = if is_result {
                (json!({"role":"assistant","content":"18 C, sunny"}), "stop")
            } else {
                (
                    json!({"role":"assistant","content":null,"tool_calls":[
                        tool_call(0, "call-weather", "weather", "{\"city\":\"Paris\"}")
                    ]}),
                    "tool_calls",
                )
            };
            Ok(serde_json::from_value(json!({
                "id":"tool-chat","object":"chat.completion","created":1,
                "model":"private-model","choices":[{"index":0,"message":message,"finish_reason":reason}],
                "usage":{"prompt_tokens":4,"completion_tokens":8,"total_tokens":12}
            })).unwrap())
        })
    }

    fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<BoxStream<'static, Result<CoreChunk, ProviderError>>, ProviderError>>
    {
        Box::pin(async move {
            assert_eq!(request.model, "private-model");
            assert!(request.stream);
            assert_eq!(
                request.stream_options.as_ref().unwrap().include_usage,
                Some(true)
            );
            self.requests.lock().unwrap().push(request);
            let mut events = vec![
                Ok(chunk(
                    json!([{"index":0,"delta":{"role":"assistant","tool_calls":[
                        tool_call(0, "call-weather", "weather", "{\"city\":"),
                        tool_call(1, "call-units", "units", "{\"unit\":")
                    ]}}]),
                    Value::Null,
                )),
                Ok(chunk(
                    json!([{"index":0,"delta":{"tool_calls":[
                        {"index":1,"function":{"arguments":"\"C\"}"}},
                        {"index":0,"function":{"arguments":"\"Paris\"}"}}
                    ]}}]),
                    Value::Null,
                )),
            ];
            if self.stream_error {
                events.push(Err(ProviderError::api_error(
                    "sdk-tools-test",
                    403,
                    "denied",
                )));
            } else {
                events.push(Ok(chunk(
                    json!([{"index":0,"delta":{},"finish_reason":"tool_calls"}]),
                    Value::Null,
                )));
                events.push(Ok(chunk(
                    json!([]),
                    json!({
                        "prompt_tokens":4,"completion_tokens":8,"total_tokens":12,
                        "prompt_tokens_details":{"cached_tokens":2},
                        "completion_tokens_details":{"reasoning_tokens":3}
                    }),
                )));
            }
            Ok(Box::pin(futures::stream::iter(events)) as BoxStream<'static, _>)
        })
    }
}

fn fixture(stream_error: bool) -> (LLMClient, Arc<ToolProvider>, Arc<UnifiedRouter>) {
    let provider = Arc::new(ToolProvider {
        models: vec![ModelInfo {
            id: "private-model".into(),
            capabilities: vec![
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
            ],
            ..Default::default()
        }],
        requests: Arc::new(Mutex::new(Vec::new())),
        stream_error,
    });
    let router = Arc::new(UnifiedRouter::default());
    router.add_deployment(Deployment::new(
        "tools".into(),
        Provider::External(provider.clone()),
        "private-model".into(),
        "tools".into(),
    ));
    let client = LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "tools").unwrap();
    (client, provider, router)
}

fn request() -> SdkChatRequest {
    SdkChatRequest {
        model: "tools".into(),
        messages: vec![Message {
            role: Role::User, content: Some(Content::Text("Weather in Paris?".into())),
            name: None, tool_calls: None, tool_call_id: None,
        }],
        options: litellm_rs::sdk::types::ChatOptions {
            tools: Some(serde_json::from_value(json!([
                {"type":"function","function":{"name":"weather","parameters":{"type":"object","properties":{"city":{"type":"string"}}}}},
                {"type":"function","function":{"name":"units","parameters":{"type":"object","properties":{"unit":{"type":"string"}}}}}
            ])).unwrap()),
            tool_choice: Some(ToolChoice::Required),
            ..Default::default()
        },
    }
}

#[tokio::test]
async fn sdk_can_receive_and_answer_a_complete_tool_call() {
    let (client, provider, _) = fixture(false);
    let mut req = request();
    let response = client.chat_with_options(req.clone()).await.unwrap();
    assert_eq!(
        response.choices[0].finish_reason.as_deref(),
        Some("tool_calls")
    );
    let assistant = response.choices.into_iter().next().unwrap().message;
    let call = &assistant.tool_calls.as_ref().unwrap()[0];
    assert_eq!(
        call.function.arguments.as_deref(),
        Some("{\"city\":\"Paris\"}")
    );
    let result = Message::tool_result(&call.id, "18 C, sunny");
    req.messages.extend([assistant, result]);
    let response = client.chat_with_options(req).await.unwrap();
    assert!(
        matches!(&response.choices[0].message.content, Some(Content::Text(text)) if text == "18 C, sunny")
    );
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].tools.as_ref().unwrap().len(), 2);
    assert_eq!(
        requests[1].messages[1].tool_calls.as_ref().unwrap()[0].id,
        "call-weather"
    );
    let result = &requests[1].messages[2];
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({
            "role":"tool", "content":"18 C, sunny", "tool_call_id":"call-weather"
        })
    );
}

#[tokio::test]
async fn sdk_stream_preserves_interleaved_tool_deltas_and_usage_only_tail() {
    let (client, provider, router) = fixture(false);
    let mut stream = client.chat_stream_with_options(request()).await.unwrap();
    let mut arguments = BTreeMap::<u32, String>::new();
    let mut ids = BTreeMap::new();
    let mut names = BTreeMap::new();
    let mut usage = None;
    let mut finished = false;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.unwrap();
        if chunk.usage.is_some() {
            assert!(chunk.choices.is_empty());
            usage = chunk.usage;
        }
        for choice in chunk.choices {
            finished |= choice.finish_reason.as_deref() == Some("tool_calls");
            for delta in choice.delta.tool_calls.into_iter().flatten() {
                if let Some(id) = delta.id {
                    ids.insert(delta.index, id);
                }
                if let Some(function) = delta.function {
                    if let Some(name) = function.name {
                        names.insert(delta.index, name);
                    }
                    if let Some(part) = function.arguments {
                        arguments.entry(delta.index).or_default().push_str(&part);
                    }
                }
            }
        }
    }
    drop(stream);
    assert!(finished);
    assert_eq!(ids[&0], "call-weather");
    assert_eq!(ids[&1], "call-units");
    assert_eq!(names[&0], "weather");
    assert_eq!(names[&1], "units");
    assert_eq!(arguments[&0], "{\"city\":\"Paris\"}");
    assert_eq!(arguments[&1], "{\"unit\":\"C\"}");
    let usage = usage.unwrap();
    assert_eq!(usage.total_tokens, 12);
    assert_eq!(usage.prompt_tokens_details.unwrap().cached_tokens, Some(2));
    assert_eq!(
        usage.completion_tokens_details.unwrap().reasoning_tokens,
        Some(3)
    );
    assert_eq!(
        router
            .get_deployment("tools")
            .unwrap()
            .state
            .active_requests
            .load(Ordering::Relaxed),
        0
    );
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests[0].tools.as_ref().unwrap().len(), 2);
    assert_eq!(
        serde_json::to_value(&requests[0].tool_choice).unwrap(),
        json!("required")
    );
}

#[tokio::test]
async fn sdk_stream_preserves_errors_after_tool_deltas_and_releases_the_lease() {
    let (client, _, router) = fixture(true);
    let mut stream = client.chat_stream_with_options(request()).await.unwrap();
    assert!(
        stream.next().await.unwrap().unwrap().choices[0]
            .delta
            .tool_calls
            .is_some()
    );
    assert!(stream.next().await.unwrap().is_ok());
    assert!(matches!(
        stream.next().await.unwrap(),
        Err(SDKError::AuthError(_))
    ));
    assert!(stream.next().await.is_none());
    drop(stream);
    assert_eq!(
        router
            .get_deployment("tools")
            .unwrap()
            .state
            .active_requests
            .load(Ordering::Relaxed),
        0
    );
}

#[tokio::test]
async fn dropping_a_tool_stream_releases_its_in_flight_admission() {
    let (client, _, router) = fixture(false);
    let mut stream = client.chat_stream_with_options(request()).await.unwrap();
    assert!(stream.next().await.unwrap().is_ok());
    let deployment = router.get_deployment("tools").unwrap();
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 1);
    drop(stream);
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
}
