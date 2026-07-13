use super::*;

async fn assert_public_only_rerank_rejects_loopback(mut provider: ProviderConfig, request: Value) {
    provider.endpoint_access = ProviderEndpointAccess::PublicOnly;
    let state = build_test_app_state(vec![provider]).await;
    let app = test::init_service(
        App::new()
            .app_data(web::Data::new(state))
            .configure(litellm_rs::server::routes::ai::configure_routes),
    )
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/rerank")
            .set_json(request)
            .to_request(),
    )
    .await;

    assert!(!response.status().is_success());
    let body: Value = test::read_body_json(response).await;
    assert!(
        body["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("SSRF protection"))
    );
}

#[tokio::test]
async fn public_only_cohere_rerank_rejects_loopback_before_connect() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let address = listener.local_addr().expect("listener should have address");
    let provider = cohere_rerank_provider(&format!("http://{address}/v1"));

    assert_public_only_rerank_rejects_loopback(provider, rerank_body()).await;

    assert!(
        tokio::time::timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err(),
        "public-only Cohere rerank route must not connect to loopback listener"
    );
}

#[tokio::test]
async fn public_only_jina_rerank_rejects_loopback_before_connect() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let address = listener.local_addr().expect("listener should have address");
    let provider = jina_rerank_provider_with_models(
        &format!("http://{address}/v1"),
        vec!["jina-reranker-v3".to_string()],
    );
    let mut request = rerank_body();
    request["model"] = json!("jina-reranker-v3");

    assert_public_only_rerank_rejects_loopback(provider, request).await;

    assert!(
        tokio::time::timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err(),
        "public-only Jina rerank route must not connect to loopback listener"
    );
}
