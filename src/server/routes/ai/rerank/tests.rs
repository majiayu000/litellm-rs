use super::*;

fn provider_config(name: &str, provider_type: &str, models: Vec<&str>) -> ProviderConfig {
    ProviderConfig {
        name: name.to_string(),
        provider_type: provider_type.to_string(),
        api_key: "test-key".to_string(),
        models: models.into_iter().map(ToString::to_string).collect(),
        ..ProviderConfig::default()
    }
}

#[test]
fn detects_provider_kind_from_type_or_name() {
    let by_type = provider_config("primary", "cohere_rerank", Vec::new());
    let by_name = provider_config("jina-reranker", "custom", Vec::new());
    let voyage = provider_config("voyage", "voyage", Vec::new());
    let unsupported = provider_config("custom", "custom", Vec::new());

    assert_eq!(
        rerank_provider_kind(&by_type),
        Some(RerankProviderKind::Cohere)
    );
    assert_eq!(
        rerank_provider_kind(&by_name),
        Some(RerankProviderKind::Jina)
    );
    assert_eq!(
        rerank_provider_kind(&voyage),
        Some(RerankProviderKind::Voyage)
    );
    assert_eq!(rerank_provider_kind(&unsupported), None);
}

#[test]
fn provider_model_filter_accepts_prefixed_and_unprefixed_models() {
    let provider = provider_config("cohere", "cohere", vec!["rerank-english-v3.0"]);

    assert!(rerank_provider_supports_model(
        &provider,
        RerankProviderKind::Cohere,
        "rerank-english-v3.0"
    ));
    assert!(rerank_provider_supports_model(
        &provider,
        RerankProviderKind::Cohere,
        "cohere/rerank-english-v3.0"
    ));
    assert!(!rerank_provider_supports_model(
        &provider,
        RerankProviderKind::Cohere,
        "jina/rerank-english-v3.0"
    ));
    assert!(!rerank_provider_supports_model(
        &provider,
        RerankProviderKind::Cohere,
        "rerank-multilingual-v3.0"
    ));
}

#[test]
fn provider_model_filter_allows_explicit_new_provider_models() {
    let cohere = provider_config("cohere", "cohere", vec!["rerank-v4.0-pro"]);
    let jina = provider_config("jina", "jina", vec!["jina-colbert-v2"]);

    assert!(rerank_provider_supports_model(
        &cohere,
        RerankProviderKind::Cohere,
        "rerank-v4.0-pro"
    ));
    assert!(rerank_provider_supports_model(
        &cohere,
        RerankProviderKind::Cohere,
        "cohere/rerank-v4.0-pro"
    ));
    assert!(rerank_provider_supports_model(
        &jina,
        RerankProviderKind::Jina,
        "jina-colbert-v2"
    ));
    assert!(rerank_provider_supports_model(
        &jina,
        RerankProviderKind::Jina,
        "jina/jina-colbert-v2"
    ));
}

#[test]
fn provider_model_filter_rejects_unconfigured_unknown_models() {
    let cohere = provider_config("cohere", "cohere", Vec::new());

    assert!(!rerank_provider_supports_model(
        &cohere,
        RerankProviderKind::Cohere,
        "rerank-v4.0-pro"
    ));
}

#[test]
fn selected_provider_uses_configured_endpoint_aliases() {
    let mut provider = provider_config("custom-voyage", "voyage", vec!["rerank-2.5"]);
    provider.settings.insert(
        "api_base".to_string(),
        serde_json::json!("https://private.example/v1"),
    );

    let selected = selected_rerank_provider_from_config(&provider, RerankProviderKind::Voyage)
        .expect("Voyage config should select");

    assert_eq!(
        selected.base_url.as_deref(),
        Some("https://private.example/v1")
    );
}

#[test]
fn selects_matching_enabled_provider() {
    let wrong = provider_config("wrong-cohere", "cohere", vec!["rerank-multilingual-v3.0"]);
    let selected = provider_config("right-cohere", "cohere", vec!["rerank-english-v3.0"]);
    let request = RerankRequest {
        model: "rerank-english-v3.0".to_string(),
        query: "hello".to_string(),
        documents: vec!["doc".into()],
        ..RerankRequest::default()
    };

    let selected = select_rerank_provider(&[wrong, selected], &request)
        .expect("matching provider should be selected");

    assert_eq!(selected.provider_name, "right-cohere");
    assert_eq!(selected.kind, RerankProviderKind::Cohere);
}

#[cfg(feature = "providers-extended")]
#[tokio::test]
async fn selected_provider_uses_deployment_id_for_native_cohere_duplicate_model() {
    let primary = provider_config("cohere-a", "cohere", vec!["rerank-english-v3.0"]);
    let fallback = provider_config("cohere-b", "cohere", vec!["rerank-english-v3.0"]);
    let cohere =
        match crate::core::providers::cohere::CohereProvider::with_api_key("test-key").await {
            Ok(provider) => provider,
            Err(error) => panic!("native cohere provider should build: {error}"),
        };
    let provider = Provider::Cohere(cohere);

    let selected = match selected_rerank_provider(
        &[primary, fallback],
        "cohere-b-rerank-english-v3.0",
        &provider,
        "rerank-english-v3.0",
        "rerank-english-v3.0",
    ) {
        Ok(selected) => selected,
        Err(error) => panic!("selected deployment id should identify the fallback config: {error}"),
    };

    assert_eq!(selected.provider_name, "cohere-b");
    assert_eq!(selected.kind, RerankProviderKind::Cohere);
}
