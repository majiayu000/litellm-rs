use super::*;

#[tokio::test]
async fn openai_like_api_base_trailing_slashes_preserve_request_paths()
-> Result<(), Box<dyn std::error::Error>> {
    // A rejected request still exposes the exact path sent on the wire, without
    // requiring a different successful response fixture for every endpoint.
    let body = r#"{"error":{"type":"authentication_error","message":"invalid key"}}"#;
    for prefix in ["", "/v1", "/proxy/openai/v1"] {
        for suffix in ["", "/", "///"] {
            for operation in ["chat", "stream", "embeddings", "images", "models"] {
                let (api_base, captured) =
                    openai_like_json_response_url("401 Unauthorized", body).await?;
                let provider = OpenAILikeProvider::new_openai_compatible(
                    private_openai_like_config(format!("{api_base}{prefix}{suffix}")),
                )
                .await?;
                let context = RequestContext::default();
                let endpoint = match operation {
                    "chat" => {
                        let error = provider
                            .chat_completion(openai_like_chat_stream_request(), context)
                            .await
                            .expect_err("upstream must reject the test key");
                        assert!(matches!(error, ProviderError::Authentication { .. }));
                        "chat/completions"
                    }
                    "stream" => {
                        let error = provider
                            .chat_completion_stream(openai_like_chat_stream_request(), context)
                            .await
                            .err()
                            .expect("upstream must reject the test key before streaming");
                        assert!(matches!(error, ProviderError::Authentication { .. }));
                        "chat/completions"
                    }
                    "embeddings" => {
                        let error = provider
                            .embeddings(
                                embedding_request(
                                    "text-embedding-3-small",
                                    EmbeddingInput::Text("hello".to_string()),
                                ),
                                context,
                            )
                            .await
                            .expect_err("upstream must reject the test key");
                        assert!(matches!(error, ProviderError::Authentication { .. }));
                        "embeddings"
                    }
                    "images" => {
                        let error = provider
                            .image_generation(image_generation_request("gpt-image-1-mini"), context)
                            .await
                            .expect_err("upstream must reject the test key");
                        assert!(matches!(error, ProviderError::Authentication { .. }));
                        "images/generations"
                    }
                    "models" => {
                        assert_eq!(provider.health_check().await, HealthStatus::Degraded);
                        "models"
                    }
                    _ => unreachable!(),
                };
                let captured = captured.lock().expect("captured request mutex");
                let captured = captured.as_ref().expect("request should reach upstream");
                assert_eq!(
                    captured.path,
                    format!("{prefix}/{endpoint}"),
                    "operation={operation}, prefix={prefix:?}, suffix={suffix:?}",
                );
            }
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_provider_creation_with_api_base() {
    let provider = OpenAILikeProvider::with_api_base(TEST_PUBLIC_API_BASE).await;
    assert!(provider.is_ok());

    let provider = provider.unwrap();
    assert_eq!(provider.name(), "openai_like");
}
