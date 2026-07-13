//! Tests for rerank module

#[cfg(test)]
use super::cache::RerankCache;
use super::providers::{CohereRerankProvider, JinaRerankProvider};
use super::service::{RerankProvider, RerankService};
use super::types::{RerankDocument, RerankRequest, RerankResponse, RerankResult};
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn test_rerank_document_creation() {
    let doc1 = RerankDocument::text("Hello world");
    assert_eq!(doc1.get_text(), "Hello world");
    assert!(doc1.get_id().is_none());

    let doc2 = RerankDocument::Structured {
        text: "Test document".to_string(),
        title: Some("Title".to_string()),
        id: Some("doc-1".to_string()),
        metadata: HashMap::new(),
    };
    assert_eq!(doc2.get_text(), "Test document");
    assert_eq!(doc2.get_id(), Some("doc-1"));
}

#[test]
fn test_rerank_document_from_string() {
    let doc: RerankDocument = "Test".into();
    assert_eq!(doc.get_text(), "Test");

    let doc2: RerankDocument = String::from("Test2").into();
    assert_eq!(doc2.get_text(), "Test2");
}

#[test]
fn test_rerank_request_default() {
    let request = RerankRequest::default();
    assert_eq!(request.model, "cohere/rerank-english-v3.0");
    assert!(request.query.is_empty());
    assert!(request.documents.is_empty());
    assert!(request.top_n.is_none());
    assert_eq!(request.return_documents, Some(true));
}

#[test]
fn test_rerank_service_extract_provider() {
    let service = RerankService::new();

    assert_eq!(
        service.extract_provider_name("cohere/rerank-english-v3.0"),
        "cohere"
    );
    assert_eq!(
        service.extract_provider_name("jina/jina-reranker-v2"),
        "jina"
    );
    assert_eq!(service.extract_provider_name("voyage/rerank-2"), "voyage");
    // No provider prefix - uses default
    assert_eq!(
        service.extract_provider_name("rerank-english-v3.0"),
        "cohere"
    );
}

#[test]
fn test_rerank_service_validation() {
    let service = RerankService::new();

    // Empty query
    let request = RerankRequest {
        query: "".to_string(),
        documents: vec![RerankDocument::text("doc")],
        ..Default::default()
    };
    assert!(service.validate_request(&request).is_err());

    // Empty documents
    let request = RerankRequest {
        query: "query".to_string(),
        documents: vec![],
        ..Default::default()
    };
    assert!(service.validate_request(&request).is_err());

    // top_n = 0
    let request = RerankRequest {
        query: "query".to_string(),
        documents: vec![RerankDocument::text("doc")],
        top_n: Some(0),
        ..Default::default()
    };
    assert!(service.validate_request(&request).is_err());

    // Valid request
    let request = RerankRequest {
        query: "query".to_string(),
        documents: vec![RerankDocument::text("doc")],
        top_n: Some(1),
        ..Default::default()
    };
    assert!(service.validate_request(&request).is_ok());
}

#[test]
fn test_cohere_provider_supports_model() {
    let Ok(provider) = CohereRerankProvider::new("test-key") else {
        panic!("official Cohere provider should build");
    };

    assert!(provider.supports_model("rerank-english-v3.0"));
    assert!(provider.supports_model("cohere/rerank-english-v3.0"));
    assert!(provider.supports_model("rerank-multilingual-v3.0"));
    assert!(!provider.supports_model("unknown-model"));
}

#[test]
fn test_jina_provider_supports_model() {
    let Ok(provider) = JinaRerankProvider::new("test-key") else {
        panic!("official Jina provider should build");
    };

    assert!(provider.supports_model("jina-reranker-v2-base-multilingual"));
    assert!(provider.supports_model("jina/jina-reranker-v2-base-multilingual"));
    assert!(!provider.supports_model("unknown-model"));
}

#[tokio::test]
async fn test_rerank_cache() {
    let cache = RerankCache::new(100, Duration::from_secs(3600));

    let request = RerankRequest {
        model: "cohere/rerank-english-v3.0".to_string(),
        query: "test query".to_string(),
        documents: vec![RerankDocument::text("test doc")],
        ..Default::default()
    };

    let response = RerankResponse {
        id: "test-id".to_string(),
        results: vec![RerankResult {
            index: 0,
            relevance_score: 0.95,
            document: Some(RerankDocument::text("test doc")),
        }],
        model: "rerank-english-v3.0".to_string(),
        usage: None,
        meta: HashMap::new(),
    };

    // Initially empty
    assert!(cache.get(&request).await.is_none());

    // Set and get
    cache.set(&request, &response).await;
    let cached = cache.get(&request).await;
    assert!(cached.is_some());
    assert_eq!(cached.unwrap().id, "test-id");

    // Stats
    let stats = cache.stats().await;
    assert_eq!(stats.total_entries, 1);
    assert_eq!(stats.valid_entries, 1);
}

#[test]
fn test_rerank_result_ordering() {
    let mut results = [
        RerankResult {
            index: 0,
            relevance_score: 0.5,
            document: None,
        },
        RerankResult {
            index: 1,
            relevance_score: 0.9,
            document: None,
        },
        RerankResult {
            index: 2,
            relevance_score: 0.7,
            document: None,
        },
    ];

    // Sort by relevance descending
    results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());

    assert_eq!(results[0].index, 1); // 0.9
    assert_eq!(results[1].index, 2); // 0.7
    assert_eq!(results[2].index, 0); // 0.5
}
