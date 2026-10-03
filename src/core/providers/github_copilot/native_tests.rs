use super::*;
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, web};
use serde_json::{Value, json};
use std::sync::Mutex;

type Seen = Arc<Mutex<Vec<(String, Value, String, String, String)>>>;

#[derive(Clone)]
struct Upstream {
    seen: Seen,
    endpoints: Value,
    status: u16,
}

async fn handle(req: HttpRequest, bytes: web::Bytes, state: web::Data<Upstream>) -> HttpResponse {
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    let header = |name| {
        req.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string()
    };
    state.seen.lock().unwrap().push((
        req.path().into(),
        body.clone(),
        header("authorization"),
        header("x-initiator"),
        header("copilot-vision-request"),
    ));
    if req.path() == "/models" {
        if state.status != 200 {
            return HttpResponse::build(
                actix_web::http::StatusCode::from_u16(state.status).unwrap(),
            )
            .insert_header(("retry-after", "7"))
            .json(json!({"error":{"message":"unavailable"}}));
        }
        return HttpResponse::Ok()
            .json(json!({"data":[{"id":"account-model", "supported_endpoints":state.endpoints}]}));
    }
    if let Some(status) = body.get("test_status").and_then(Value::as_u64) {
        return HttpResponse::build(actix_web::http::StatusCode::from_u16(status as u16).unwrap())
            .insert_header(("retry-after", "7"))
            .json(json!({"error":{"message":"native failure"}}));
    }
    if body.get("stream") != Some(&Value::Bool(true)) {
        return HttpResponse::Ok().json(json!({"id":"copilot-native", "output":[], "usage":{"input_tokens":3,"output_tokens":2},"extension":true}));
    }
    HttpResponse::Ok().insert_header(("content-type", "text/event-stream"))
        .body("event: response.completed\ndata: {\"type\":\"response.completed\",\"opaque\":true}\n\n")
}

async fn fixture(
    endpoints: Value,
    status: u16,
) -> (
    GitHubCopilotProvider,
    Upstream,
    actix_web::dev::ServerHandle,
) {
    let state = Upstream {
        seen: Arc::default(),
        endpoints,
        status,
    };
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let data = state.clone();
    let server = HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(data.clone()))
            .default_service(web::to(handle))
    })
    .workers(1)
    .listen(listener)
    .unwrap()
    .run();
    let handle = server.handle();
    tokio::spawn(server);
    let provider = GitHubCopilotProvider::new(GitHubCopilotConfig {
        api_base: Some(format!("http://{address}")),
        ..Default::default()
    })
    .await
    .unwrap();
    *provider.cached_api_key.write().await = Some("fake-local-copilot-token".into());
    (provider, state, handle)
}

#[tokio::test]
async fn native_responses_use_account_endpoint_evidence_and_preserve_body_headers_sse() {
    let (provider, state, server) = fixture(json!(["/responses"]), 200).await;
    let body = json!({"model":"account-model","stream":true,"input":[{"role":"user","content":[{"type":"input_image","image_url":"data:image/png;base64,AA=="}]},{"type":"function_call_output","call_id":"call_1","output":"opaque"}],"tools":[{"type":"function","name":"lookup","parameters":{}}],"reasoning":{"effort":"low"},"future_field":true});
    let response = provider.native_response(body.clone()).await.unwrap();
    assert!(response.text().await.unwrap().contains("\"opaque\":true"));
    let seen = state.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0].0, "/models");
    assert_eq!(seen[1].0, "/responses");
    assert_eq!(seen[1].1, body);
    assert_eq!(seen[1].2, "Bearer fake-local-copilot-token");
    assert_eq!(seen[1].3, "agent");
    assert_eq!(seen[1].4, "true");
    server.stop(false).await;
}

#[tokio::test]
async fn response_only_models_never_reach_chat_completions() {
    let (provider, state, server) = fixture(json!(["/responses"]), 200).await;
    let request = ChatRequest::new("account-model");
    assert!(
        provider
            .chat_completion(request.clone(), RequestContext::new())
            .await
            .is_err()
    );
    assert!(
        provider
            .chat_completion_stream(request, RequestContext::new())
            .await
            .is_err()
    );
    assert!(
        state
            .seen
            .lock()
            .unwrap()
            .iter()
            .all(|entry| entry.0 == "/models")
    );
    server.stop(false).await;
}

#[tokio::test]
async fn missing_endpoint_evidence_and_unknown_models_fail_before_inference() {
    for endpoints in [Value::Null, json!([]), json!(["/chat/completions"])] {
        let (provider, state, server) = fixture(endpoints, 200).await;
        assert!(matches!(
            provider
                .native_response(json!({"model":"account-model","input":"Hi"}))
                .await,
            Err(ProviderError::NotSupported { .. })
        ));
        assert!(matches!(
            provider
                .native_response(json!({"model":"gpt-made-up","input":"Hi"}))
                .await,
            Err(ProviderError::ModelNotFound { .. })
        ));
        assert!(
            state
                .seen
                .lock()
                .unwrap()
                .iter()
                .all(|entry| entry.0 == "/models")
        );
        server.stop(false).await;
    }
}

#[tokio::test]
async fn discovery_errors_preserve_classification_and_never_fall_back() {
    for status in [401, 429, 503] {
        let (provider, state, server) = fixture(json!(["/responses"]), status).await;
        let error = provider
            .native_response(json!({"model":"account-model","input":"Hi"}))
            .await
            .unwrap_err();
        match status {
            401 => assert!(matches!(error, ProviderError::Authentication { .. })),
            429 => assert!(matches!(error, ProviderError::RateLimit { .. })),
            _ => assert!(matches!(error, ProviderError::ApiError { status: 503, .. })),
        }
        assert_eq!(state.seen.lock().unwrap().len(), 1);
        server.stop(false).await;
    }
}

#[tokio::test]
async fn native_json_and_upstream_errors_are_preserved() {
    let (provider, state, server) = fixture(json!(["/responses"]), 200).await;
    let response = provider
        .native_response(json!({"model":"account-model","input":"Hi"}))
        .await
        .unwrap();
    let value: Value = response.json().await.unwrap();
    assert_eq!(value["usage"]["input_tokens"], 3);
    assert_eq!(value["extension"], true);
    for status in [429, 503, 401] {
        let error = provider
            .native_response(json!({"model":"account-model","input":"Hi","test_status":status}))
            .await
            .unwrap_err();
        match status {
            401 => assert!(matches!(error, ProviderError::Authentication { .. })),
            429 => assert!(matches!(error, ProviderError::RateLimit { .. })),
            _ => assert!(matches!(error, ProviderError::ApiError { status: 503, .. })),
        }
    }
    assert_eq!(state.seen.lock().unwrap().len(), 8);
    server.stop(false).await;
}
