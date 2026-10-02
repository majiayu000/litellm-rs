//! Downstream API proof: this integration crate cannot access crate-private wiring.
use futures::{StreamExt, future::BoxFuture, stream::BoxStream};
use litellm_rs::core::{
    providers::{ExternalProvider, Provider, ProviderError, ProviderType},
    router::{Deployment, UnifiedRouter},
    types::{
        chat::ChatRequest,
        context::RequestContext,
        embedding::EmbeddingRequest,
        model::{ModelInfo, ProviderCapability},
        responses::{ChatChunk, ChatResponse},
    },
};
use serde_json::json;
use std::sync::{Arc, Mutex};

type RecordedCalls = Arc<Mutex<Vec<(String, String)>>>;

#[derive(Debug)]
struct DownstreamProvider {
    models: Vec<ModelInfo>,
    calls: RecordedCalls,
    fail: bool,
}

impl ExternalProvider for DownstreamProvider {
    fn name(&self) -> &str {
        "downstream"
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
        context: RequestContext,
    ) -> BoxFuture<'_, Result<ChatResponse, ProviderError>> {
        Box::pin(async move {
            self.calls
                .lock()
                .unwrap()
                .push((request.model.clone(), context.request_id));
            if self.fail {
                return Err(ProviderError::api_error(
                    "downstream",
                    403,
                    "workspace denied",
                ));
            }
            Ok(serde_json::from_value(json!({"id":"external-response", "object":"chat.completion", "created":1,"model":request.model,"choices":[{"index":0,"message":{"role":"assistant","content":"external answer"},"finish_reason":"stop"}],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}})).unwrap())
        })
    }
    fn chat_completion_stream(
        &self,
        request: ChatRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<BoxStream<'static, Result<ChatChunk, ProviderError>>, ProviderError>>
    {
        Box::pin(async move {
            let chunk: ChatChunk = serde_json::from_value(json!({"id":"external-stream","object":"chat.completion.chunk","created":1,"model":request.model,"choices":[{"index":0,"delta":{"content":"first"}}]})).unwrap();
            let events = vec![
                Ok(chunk),
                Err(ProviderError::rate_limit("downstream", Some(7))),
            ];
            Ok(Box::pin(futures::stream::iter(events)) as BoxStream<'static, _>)
        })
    }
}

fn fixture(fail: bool) -> (UnifiedRouter, RecordedCalls) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let provider = Provider::External(Arc::new(DownstreamProvider {
        models: vec![
            ModelInfo {
                id: "chat-model".into(),
                capabilities: vec![
                    ProviderCapability::ChatCompletion,
                    ProviderCapability::ChatCompletionStream,
                ],
                ..Default::default()
            },
            ModelInfo {
                id: "sync-model".into(),
                capabilities: vec![ProviderCapability::ChatCompletion],
                ..Default::default()
            },
        ],
        calls: Arc::clone(&calls),
        fail,
    }));
    let router = UnifiedRouter::default();
    router.add_deployment(Deployment::new(
        "external".into(),
        provider.clone(),
        "chat-model".into(),
        "public-name".into(),
    ));
    router.add_deployment(Deployment::new(
        "sync".into(),
        provider,
        "sync-model".into(),
        "sync-only".into(),
    ));
    (router, calls)
}

#[tokio::test]
async fn external_provider_executes_through_existing_router() {
    let (router, calls) = fixture(false);
    let result = router
        .execute_with_selected_deployment_capability_retry(
            "public-name",
            &ProviderCapability::ChatCompletion,
            |deployment| async move {
                let context = RequestContext {
                    request_id: "external-request".into(),
                    ..Default::default()
                };
                let response = deployment
                    .provider
                    .chat_completion(
                        ChatRequest::new(&deployment.model).add_user_message("hello"),
                        context,
                    )
                    .await?;
                let tokens = response.usage.as_ref().unwrap().total_tokens;
                Ok((response, u64::from(tokens)))
            },
        )
        .await
        .unwrap();
    assert_eq!(result.0.first_content(), Some("external answer"));
    assert_eq!(result.1, "external");
    assert_eq!(
        calls.lock().unwrap().as_slice(),
        &[("chat-model".into(), "external-request".into())]
    );
    let provider = &router.get_deployment("external").unwrap().provider;
    assert_eq!(provider.name(), "downstream");
    assert_eq!(
        provider.provider_type(),
        ProviderType::Custom("downstream".into())
    );
    assert!(provider.supports_model("chat-model"));
    assert!(!provider.supports_model("unlisted"));
}

#[tokio::test]
async fn external_stream_preserves_chunks_and_terminal_error() {
    let (router, _) = fixture(false);
    let lease = router
        .select_deployment_lease_for_capability(
            "public-name",
            &ProviderCapability::ChatCompletionStream,
        )
        .unwrap();
    let mut stream = lease
        .deployment()
        .provider
        .chat_completion_stream(ChatRequest::new("chat-model"), RequestContext::default())
        .await
        .unwrap();
    assert_eq!(
        stream.next().await.unwrap().unwrap().choices[0]
            .delta
            .content
            .as_deref(),
        Some("first")
    );
    assert!(matches!(
        stream.next().await.unwrap(),
        Err(ProviderError::RateLimit {
            retry_after: Some(7),
            ..
        })
    ));
    assert!(stream.next().await.is_none());
    assert!(
        router
            .select_deployment_lease_for_capability(
                "sync-only",
                &ProviderCapability::ChatCompletionStream
            )
            .is_err()
    );
    assert!(
        router
            .select_deployment_lease_for_capability("public-name", &ProviderCapability::Embeddings)
            .is_err()
    );
}

#[tokio::test]
async fn external_provider_errors_keep_status_and_are_not_retried() {
    let (router, calls) = fixture(true);
    let (error, attempts) = router
        .execute_with_selected_deployment_capability_retry(
            "public-name",
            &ProviderCapability::ChatCompletion,
            |deployment| async move {
                deployment
                    .provider
                    .chat_completion(
                        ChatRequest::new(&deployment.model),
                        RequestContext::default(),
                    )
                    .await
                    .map(|response| (response, 0))
            },
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error, ProviderError::ApiError { provider: "downstream", status:403, ref message } if message == "workspace denied")
    );
    assert_eq!(attempts, 1);
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn external_optional_methods_and_unknown_prices_are_not_free_successes() {
    let (router, _) = fixture(false);
    let provider = &router.get_deployment("external").unwrap().provider;
    let request: EmbeddingRequest =
        serde_json::from_value(json!({"model":"chat-model","input":"hello"})).unwrap();
    assert!(matches!(
        provider
            .create_embeddings(request, RequestContext::default())
            .await,
        Err(ProviderError::NotSupported { .. })
    ));
    assert!(matches!(
        provider.calculate_cost("chat-model", 1, 1).await,
        Err(ProviderError::NotSupported { .. })
    ));
}
