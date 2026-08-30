use super::*;

#[test]
fn bfl_credentialed_polling_is_wired_to_a_no_redirect_lifecycle() {
    let source = include_str!("../../src/core/providers/black_forest_labs/mod.rs");

    assert!(source.contains("GenerationLifecycle::new_no_redirect"));
}

#[tokio::test]
async fn bfl_poll_redirect_does_not_forward_api_key_to_another_origin() {
    let source_listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock BFL listener should bind");
    let source_address = source_listener
        .local_addr()
        .expect("source address should exist");
    let target_listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("redirect target should bind");
    let target_address = target_listener
        .local_addr()
        .expect("target address should exist");
    let source_server = tokio::spawn(async move {
        let (mut socket, _) = source_listener
            .accept()
            .await
            .expect("submit should arrive");
        let _submit = read_http_request(&mut socket).await;
        let body =
            format!(r#"{{"id":"task-1","polling_url":"http://{source_address}/poll/task-1"}}"#);
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("submit response should write");

        let (mut socket, _) = source_listener.accept().await.expect("poll should arrive");
        let poll = read_http_request(&mut socket).await;
        assert!(poll.to_ascii_lowercase().contains("x-key: bfl-secret"));
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/foreign\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        socket
            .write_all(response.as_bytes())
            .await
            .expect("redirect response should write");
    });
    let target_server = tokio::spawn(async move {
        tokio::time::timeout(
            std::time::Duration::from_millis(150),
            target_listener.accept(),
        )
        .await
    });

    let mut config = BflConfig::with_api_key("bfl-secret");
    config.base.api_base = Some(format!("http://{source_address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    config.poll_policy = PollPolicy::from_millis(1, 4, 200);
    let provider = BflProvider::new(config).expect("BFL provider should initialize");
    let error = provider
        .generate_native(
            BflImageRequest::new("flux-pro-1.1", "a glass city"),
            &CancellationToken::new(),
        )
        .await
        .expect_err("redirect response must not be followed");
    source_server.await.expect("source server should finish");
    let target_result = target_server.await.expect("target server should finish");

    assert!(matches!(error, ProviderError::Other { .. }));
    assert!(target_result.is_err(), "foreign origin received BFL x-key");
}

#[tokio::test]
async fn bfl_invalid_successful_submit_bodies_are_not_safe_to_resubmit() {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("mock BFL listener should bind");
    let address = listener.local_addr().expect("mock address should exist");
    let server = tokio::spawn(async move {
        for body in ["not-json", "{}", r#"{"polling_url":42}"#] {
            let (mut socket, _) = listener.accept().await.expect("submit should arrive");
            let _request = read_http_request(&mut socket).await;
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket
                .write_all(response.as_bytes())
                .await
                .expect("submit response should write");
        }
    });

    let mut config = BflConfig::with_api_key("bfl-secret");
    config.base.api_base = Some(format!("http://{address}"));
    config.base.endpoint_access = ProviderEndpointAccess::PrivateNetwork;
    let provider = BflProvider::new(config).expect("BFL provider should initialize");
    for _ in 0..3 {
        let error = provider
            .generate_native(
                BflImageRequest::new("flux-pro-1.1", "a glass city"),
                &CancellationToken::new(),
            )
            .await
            .expect_err("invalid accepted submit response should fail");
        assert!(matches!(error, ProviderError::Other { .. }));
        assert!(error.to_string().contains("already accepted"));
    }
    server.await.expect("mock server should finish");
}
