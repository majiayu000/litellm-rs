use super::*;

#[tokio::test]
async fn public_only_fine_tuning_route_rejects_loopback_before_connect() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let address = listener.local_addr().expect("listener should have address");
    let mut provider = fine_tuning_provider(&format!("http://{address}/v1"));
    provider.endpoint_access = ProviderEndpointAccess::PublicOnly;
    let state = build_test_state(vec![provider]).await;
    let app = test::init_service(App::new().app_data(web::Data::new(state)).configure(|cfg| {
        litellm_rs::server::routes::ai::configure_routes(
            cfg,
            litellm_rs::config::models::default_max_body_size(),
        )
    }))
    .await;

    let response = test::call_service(
        &app,
        test::TestRequest::post()
            .uri("/v1/fine_tuning/jobs")
            .set_json(json!({
                "model": "gpt-4o-mini",
                "training_file": "file-train"
            }))
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
    assert!(
        tokio::time::timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err(),
        "public-only fine-tuning route must not connect to loopback listener"
    );
}
