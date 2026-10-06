//! Exercise the public SDK facade as a downstream crate with isolated runtimes.
use futures::future::BoxFuture;
use litellm_rs::{
    core::{
        providers::{ExternalProvider, Provider, ProviderError},
        router::{Deployment, RuntimeBinding, UnifiedRouter},
        types::{
            chat::ChatRequest,
            context::RequestContext,
            embedding::EmbeddingRequest,
            model::{ModelInfo, ProviderCapability},
            responses::{ChatResponse, EmbeddingResponse},
        },
    },
    sdk::{LLMClient, errors::SDKError},
};
use serde_json::json;
use std::sync::{Arc, Mutex, atomic::Ordering};

type Calls = Arc<Mutex<Vec<EmbeddingRequest>>>;

#[derive(Debug)]
struct EmbeddingProvider {
    models: Vec<ModelInfo>,
    calls: Calls,
    marker: f32,
    fail: bool,
}

impl ExternalProvider for EmbeddingProvider {
    fn name(&self) -> &str {
        "sdk-embedding-test"
    }

    fn capabilities(&self) -> &'static [ProviderCapability] {
        &[ProviderCapability::Embeddings]
    }

    fn models(&self) -> &[ModelInfo] {
        &self.models
    }

    fn chat_completion(
        &self,
        _request: ChatRequest,
        _context: RequestContext,
    ) -> BoxFuture<'_, Result<ChatResponse, ProviderError>> {
        Box::pin(async { panic!("embedding must not dispatch to chat") })
    }

    fn embeddings(
        &self,
        request: EmbeddingRequest,
        context: RequestContext,
    ) -> BoxFuture<'_, Result<EmbeddingResponse, ProviderError>> {
        Box::pin(async move {
            assert!(!context.request_id.is_empty());
            let count = request.input.to_vec().len();
            let model = request.model.clone();
            self.calls.lock().unwrap().push(request);
            if self.fail {
                return Err(ProviderError::api_error(
                    "sdk-embedding-test",
                    403,
                    "embedding access denied",
                ));
            }
            // Upstreams may return entries out of order; preserve input order
            // in the SDK batch result by respecting each entry's index.
            let data: Vec<_> = (0..count)
                .rev()
                .map(|index| {
                    json!({"object":"embedding", "index":index,
                        "embedding":[self.marker, index as f32]})
                })
                .collect();
            Ok(serde_json::from_value(json!({
                "object":"list", "model":model, "data":data,
                "usage":{"prompt_tokens":3,"completion_tokens":0,"total_tokens":3}
            }))
            .unwrap())
        })
    }
}

fn client(marker: f32, fail: bool) -> (LLMClient, Calls, Arc<UnifiedRouter>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let provider = Provider::External(Arc::new(EmbeddingProvider {
        models: vec![ModelInfo {
            id: "vendor/embed-v1".into(),
            capabilities: vec![ProviderCapability::Embeddings],
            ..Default::default()
        }],
        calls: Arc::clone(&calls),
        marker,
        fail,
    }));
    let router = Arc::new(UnifiedRouter::default());
    router.add_deployment(Deployment::new(
        "embeddings".into(),
        provider,
        "vendor/embed-v1".into(),
        "my-embeddings".into(),
    ));
    router
        .add_model_alias("openai/text-embedding-3-small", "my-embeddings")
        .unwrap();
    let client =
        LLMClient::from_runtime(RuntimeBinding::new(router.clone()), "my-embeddings").unwrap();
    (client, calls, router)
}

#[tokio::test]
async fn sdk_embeddings_use_the_bound_runtime_for_default_and_explicit_models() {
    let (first, first_calls, router) = client(1.0, false);
    let (second, second_calls, _) = client(2.0, false);
    // Neither SDK has a legacy provider config or a global runtime installed.
    assert!(first.config().providers.is_empty());
    assert_eq!(first.embedding("one", None).await.unwrap(), [1.0, 0.0]);
    assert_eq!(
        second
            .embedding("two", Some("openai/text-embedding-3-small"))
            .await
            .unwrap(),
        [2.0, 0.0]
    );
    let texts = vec!["first".to_string(), "second".to_string()];
    assert_eq!(
        first
            .batch_embedding(&texts, Some("my-embeddings"))
            .await
            .unwrap(),
        vec![vec![1.0, 0.0], vec![1.0, 1.0]]
    );
    let calls = first_calls.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert!(
        calls
            .iter()
            .all(|request| request.model == "vendor/embed-v1")
    );
    assert_eq!(calls[0].input.to_vec(), ["one"]);
    assert_eq!(calls[1].input.to_vec(), texts);
    assert_eq!(second_calls.lock().unwrap().len(), 1);
    let deployment = router.get_deployment("embeddings").unwrap();
    assert_eq!(deployment.state.success_requests.load(Ordering::Relaxed), 2);
    assert_eq!(deployment.state.active_requests.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn sdk_embedding_unknown_models_do_not_fall_back_to_the_default() {
    let (client, calls, _) = client(1.0, false);
    assert!(matches!(
        client.embedding("one", Some("unknown/model")).await,
        Err(SDKError::ModelNotFound(_))
    ));
    assert!(calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn sdk_embedding_preserves_provider_errors_without_retrying_authorization_failures() {
    let (client, calls, _) = client(1.0, true);
    assert!(matches!(
        client.embedding("one", None).await,
        Err(SDKError::AuthError(_))
    ));
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn sdk_embedding_requires_embedding_capability_before_dispatch() {
    let (client, calls, router) = client(1.0, false);
    let provider = router
        .get_deployment("embeddings")
        .unwrap()
        .provider
        .clone();
    router.add_deployment(Deployment::new(
        "unsupported".into(),
        provider,
        "unadvertised-model".into(),
        "unsupported".into(),
    ));
    assert!(matches!(
        client.embedding("one", Some("unsupported")).await,
        Err(SDKError::NotSupported(_))
    ));
    assert!(calls.lock().unwrap().is_empty());
}
