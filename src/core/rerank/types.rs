//! Rerank types and data structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Rerank request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankRequest {
    /// Model to use for reranking (e.g., "cohere/rerank-english-v3.0")
    pub model: String,

    /// The query to compare documents against
    pub query: String,

    /// List of documents to rerank
    pub documents: Vec<RerankDocument>,

    /// Number of top results to return (default: all documents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_n: Option<usize>,

    /// Whether to return the document text in results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_documents: Option<bool>,

    /// Maximum number of chunks per document (for long documents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_chunks_per_doc: Option<usize>,

    /// Whether the provider may truncate over-length query/documents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncation: Option<bool>,

    /// Additional provider-specific parameters
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra_params: HashMap<String, serde_json::Value>,
}

impl Default for RerankRequest {
    fn default() -> Self {
        Self {
            model: "cohere/rerank-english-v3.0".to_string(),
            query: String::new(),
            documents: Vec::new(),
            top_n: None,
            return_documents: Some(true),
            max_chunks_per_doc: None,
            truncation: None,
            extra_params: HashMap::new(),
        }
    }
}

/// Document for reranking - can be a simple string or structured
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RerankDocument {
    /// Simple text document
    Text(String),
    /// Structured document with metadata
    Structured {
        /// Document text content
        text: String,
        /// Optional document title
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        /// Optional document ID for tracking
        #[serde(skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        /// Additional metadata
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        metadata: HashMap<String, serde_json::Value>,
    },
}

impl RerankDocument {
    /// Create a simple text document
    pub fn text(content: impl Into<String>) -> Self {
        Self::Text(content.into())
    }

    /// Create a structured document
    pub fn structured(text: impl Into<String>) -> Self {
        Self::Structured {
            text: text.into(),
            title: None,
            id: None,
            metadata: HashMap::new(),
        }
    }

    /// Get the text content of the document
    pub fn get_text(&self) -> &str {
        match self {
            Self::Text(t) => t,
            Self::Structured { text, .. } => text,
        }
    }

    /// Get the document ID if available
    pub fn get_id(&self) -> Option<&str> {
        match self {
            Self::Text(_) => None,
            Self::Structured { id, .. } => id.as_deref(),
        }
    }
}

impl From<String> for RerankDocument {
    fn from(s: String) -> Self {
        Self::Text(s)
    }
}

impl From<&str> for RerankDocument {
    fn from(s: &str) -> Self {
        Self::Text(s.to_string())
    }
}

/// Rerank response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResponse {
    /// Unique response ID
    pub id: String,

    /// Reranked results ordered by relevance (highest first)
    pub results: Vec<RerankResult>,

    /// Model used for reranking
    pub model: String,

    /// Token usage information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<RerankUsage>,

    /// Response metadata
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub meta: HashMap<String, serde_json::Value>,
}

/// Individual rerank result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResult {
    /// Original index of the document in the input list
    pub index: usize,

    /// Relevance score (typically 0.0 to 1.0, higher is more relevant)
    pub relevance_score: f64,

    /// The document text (if return_documents was true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<RerankDocument>,
}

/// Token usage for reranking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RerankUsage {
    /// Number of tokens in the query
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_tokens: Option<u32>,

    /// Number of tokens in all documents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_tokens: Option<u32>,

    /// Total tokens processed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,

    /// Search units consumed (Cohere-specific)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_units: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== RerankRequest Tests ====================

    #[test]
    fn test_rerank_request_default() {
        let request = RerankRequest::default();

        assert_eq!(request.model, "cohere/rerank-english-v3.0");
        assert!(request.query.is_empty());
        assert!(request.documents.is_empty());
        assert!(request.top_n.is_none());
        assert_eq!(request.return_documents, Some(true));
        assert!(request.max_chunks_per_doc.is_none());
        assert!(request.extra_params.is_empty());
    }

    #[test]
    fn test_rerank_request_custom() {
        let request = RerankRequest {
            model: "cohere/rerank-multilingual-v3.0".to_string(),
            query: "What is machine learning?".to_string(),
            documents: vec![
                RerankDocument::text("Machine learning is a subset of AI."),
                RerankDocument::text("Deep learning uses neural networks."),
            ],
            top_n: Some(5),
            return_documents: Some(false),
            max_chunks_per_doc: Some(10),
            truncation: None,
            extra_params: HashMap::new(),
        };

        assert_eq!(request.query, "What is machine learning?");
        assert_eq!(request.documents.len(), 2);
        assert_eq!(request.top_n, Some(5));
    }

    #[test]
    fn test_rerank_request_with_extra_params() {
        let mut extra_params = HashMap::new();
        extra_params.insert(
            "custom_field".to_string(),
            serde_json::json!("custom_value"),
        );

        let request = RerankRequest {
            extra_params,
            ..Default::default()
        };

        assert!(request.extra_params.contains_key("custom_field"));
    }

    #[test]
    fn test_rerank_request_clone() {
        let request = RerankRequest {
            model: "test-model".to_string(),
            query: "test query".to_string(),
            documents: vec![RerankDocument::text("doc1")],
            top_n: Some(3),
            ..Default::default()
        };

        let cloned = request.clone();
        assert_eq!(request.model, cloned.model);
        assert_eq!(request.query, cloned.query);
        assert_eq!(request.top_n, cloned.top_n);
    }

    #[test]
    fn test_rerank_request_serialization() {
        let request = RerankRequest {
            model: "cohere/rerank-english-v3.0".to_string(),
            query: "What is AI?".to_string(),
            documents: vec![RerankDocument::text("AI is artificial intelligence")],
            top_n: Some(10),
            return_documents: Some(true),
            max_chunks_per_doc: None,
            truncation: None,
            extra_params: HashMap::new(),
        };

        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["model"], "cohere/rerank-english-v3.0");
        assert_eq!(json["query"], "What is AI?");
        assert_eq!(json["top_n"], 10);
        // Empty extra_params should not be serialized
        assert!(!json.as_object().unwrap().contains_key("extra_params"));
    }

    #[test]
    fn test_rerank_request_deserialization() {
        let json = r#"{
            "model": "cohere/rerank-english-v3.0",
            "query": "What is Rust?",
            "documents": ["Rust is a systems programming language"],
            "top_n": 5
        }"#;

        let request: RerankRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.model, "cohere/rerank-english-v3.0");
        assert_eq!(request.query, "What is Rust?");
        assert_eq!(request.top_n, Some(5));
    }

    // ==================== RerankDocument Tests ====================

    #[test]
    fn test_rerank_document_text() {
        let doc = RerankDocument::text("This is a document");
        assert_eq!(doc.get_text(), "This is a document");
        assert!(doc.get_id().is_none());
    }

    #[test]
    fn test_rerank_document_structured() {
        let doc = RerankDocument::structured("Document content");

        match &doc {
            RerankDocument::Structured {
                text,
                title,
                id,
                metadata,
            } => {
                assert_eq!(text, "Document content");
                assert!(title.is_none());
                assert!(id.is_none());
                assert!(metadata.is_empty());
            }
            _ => panic!("Expected Structured variant"),
        }
    }

    #[test]
    fn test_rerank_document_structured_with_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("source".to_string(), serde_json::json!("web"));

        let doc = RerankDocument::Structured {
            text: "Content".to_string(),
            title: Some("Document Title".to_string()),
            id: Some("doc-123".to_string()),
            metadata,
        };

        assert_eq!(doc.get_text(), "Content");
        assert_eq!(doc.get_id(), Some("doc-123"));
    }

    #[test]
    fn test_rerank_document_from_string() {
        let doc: RerankDocument = "Hello world".to_string().into();
        assert_eq!(doc.get_text(), "Hello world");
    }

    #[test]
    fn test_rerank_document_from_str() {
        let doc: RerankDocument = "Hello world".into();
        assert_eq!(doc.get_text(), "Hello world");
    }

    #[test]
    fn test_rerank_document_clone() {
        let doc = RerankDocument::Structured {
            text: "Clone test".to_string(),
            title: Some("Title".to_string()),
            id: Some("id-1".to_string()),
            metadata: HashMap::new(),
        };

        let cloned = doc.clone();
        assert_eq!(doc.get_text(), cloned.get_text());
        assert_eq!(doc.get_id(), cloned.get_id());
    }

    #[test]
    fn test_rerank_document_text_serialization() {
        let doc = RerankDocument::text("Simple text");
        let json = serde_json::to_value(&doc).unwrap();
        // Text variant serializes as just a string
        assert!(json.is_string());
        assert_eq!(json.as_str().unwrap(), "Simple text");
    }

    #[test]
    fn test_rerank_document_structured_serialization() {
        let doc = RerankDocument::Structured {
            text: "Structured content".to_string(),
            title: Some("Title".to_string()),
            id: Some("doc-1".to_string()),
            metadata: HashMap::new(),
        };

        let json = serde_json::to_value(&doc).unwrap();
        assert!(json.is_object());
        assert_eq!(json["text"], "Structured content");
        assert_eq!(json["title"], "Title");
        assert_eq!(json["id"], "doc-1");
    }

    #[test]
    fn test_rerank_document_text_deserialization() {
        let json = r#""Simple document text""#;
        let doc: RerankDocument = serde_json::from_str(json).unwrap();
        assert_eq!(doc.get_text(), "Simple document text");
    }

    #[test]
    fn test_rerank_document_structured_deserialization() {
        let json = r#"{
            "text": "Structured text",
            "title": "Document Title",
            "id": "doc-456"
        }"#;

        let doc: RerankDocument = serde_json::from_str(json).unwrap();
        assert_eq!(doc.get_text(), "Structured text");
        assert_eq!(doc.get_id(), Some("doc-456"));
    }

    // ==================== RerankResponse Tests ====================

    #[test]
    fn test_rerank_response_basic() {
        let response = RerankResponse {
            id: "resp-123".to_string(),
            results: vec![],
            model: "cohere/rerank-english-v3.0".to_string(),
            usage: None,
            meta: HashMap::new(),
        };

        assert_eq!(response.id, "resp-123");
        assert!(response.results.is_empty());
        assert!(response.usage.is_none());
    }

    #[test]
    fn test_rerank_response_with_results() {
        let response = RerankResponse {
            id: "resp-456".to_string(),
            results: vec![
                RerankResult {
                    index: 2,
                    relevance_score: 0.95,
                    document: Some(RerankDocument::text("Most relevant doc")),
                },
                RerankResult {
                    index: 0,
                    relevance_score: 0.72,
                    document: Some(RerankDocument::text("Second relevant doc")),
                },
            ],
            model: "cohere/rerank-english-v3.0".to_string(),
            usage: Some(RerankUsage {
                query_tokens: Some(10),
                document_tokens: Some(50),
                total_tokens: Some(60),
                search_units: Some(1),
            }),
            meta: HashMap::new(),
        };

        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].index, 2);
        assert_eq!(response.results[0].relevance_score, 0.95);
        assert!(response.usage.is_some());
    }

    #[test]
    fn test_rerank_response_clone() {
        let response = RerankResponse {
            id: "clone-test".to_string(),
            results: vec![RerankResult {
                index: 0,
                relevance_score: 0.5,
                document: None,
            }],
            model: "test-model".to_string(),
            usage: None,
            meta: HashMap::new(),
        };

        let cloned = response.clone();
        assert_eq!(response.id, cloned.id);
        assert_eq!(response.results.len(), cloned.results.len());
    }

    #[test]
    fn test_rerank_response_serialization() {
        let response = RerankResponse {
            id: "resp-ser".to_string(),
            results: vec![RerankResult {
                index: 1,
                relevance_score: 0.88,
                document: None,
            }],
            model: "cohere/rerank-english-v3.0".to_string(),
            usage: None,
            meta: HashMap::new(),
        };

        let json = serde_json::to_value(&response).unwrap();
        assert_eq!(json["id"], "resp-ser");
        assert!(json["results"].is_array());
        // Empty meta should not be serialized
        assert!(!json.as_object().unwrap().contains_key("meta"));
    }

    // ==================== RerankResult Tests ====================

    #[test]
    fn test_rerank_result_basic() {
        let result = RerankResult {
            index: 5,
            relevance_score: 0.75,
            document: None,
        };

        assert_eq!(result.index, 5);
        assert_eq!(result.relevance_score, 0.75);
        assert!(result.document.is_none());
    }

    #[test]
    fn test_rerank_result_with_document() {
        let result = RerankResult {
            index: 0,
            relevance_score: 0.99,
            document: Some(RerankDocument::text("Highly relevant document")),
        };

        assert!(result.document.is_some());
        assert_eq!(
            result.document.as_ref().unwrap().get_text(),
            "Highly relevant document"
        );
    }

    #[test]
    fn test_rerank_result_clone() {
        let result = RerankResult {
            index: 3,
            relevance_score: 0.65,
            document: Some(RerankDocument::text("Test doc")),
        };

        let cloned = result.clone();
        assert_eq!(result.index, cloned.index);
        assert_eq!(result.relevance_score, cloned.relevance_score);
    }

    #[test]
    fn test_rerank_result_serialization() {
        let result = RerankResult {
            index: 2,
            relevance_score: 0.8,
            document: Some(RerankDocument::text("Doc text")),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["index"], 2);
        assert_eq!(json["relevance_score"], 0.8);
        assert_eq!(json["document"], "Doc text");
    }

    #[test]
    fn test_rerank_result_deserialization() {
        let json = r#"{
            "index": 1,
            "relevance_score": 0.92,
            "document": "Deserialized doc"
        }"#;

        let result: RerankResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.index, 1);
        assert_eq!(result.relevance_score, 0.92);
        assert!(result.document.is_some());
    }

    // ==================== RerankUsage Tests ====================

    #[test]
    fn test_rerank_usage_default() {
        let usage = RerankUsage::default();

        assert!(usage.query_tokens.is_none());
        assert!(usage.document_tokens.is_none());
        assert!(usage.total_tokens.is_none());
        assert!(usage.search_units.is_none());
    }

    #[test]
    fn test_rerank_usage_full() {
        let usage = RerankUsage {
            query_tokens: Some(15),
            document_tokens: Some(100),
            total_tokens: Some(115),
            search_units: Some(2),
        };

        assert_eq!(usage.query_tokens, Some(15));
        assert_eq!(usage.document_tokens, Some(100));
        assert_eq!(usage.total_tokens, Some(115));
        assert_eq!(usage.search_units, Some(2));
    }

    #[test]
    fn test_rerank_usage_partial() {
        let usage = RerankUsage {
            query_tokens: Some(10),
            total_tokens: Some(50),
            ..Default::default()
        };

        assert_eq!(usage.query_tokens, Some(10));
        assert!(usage.document_tokens.is_none());
        assert_eq!(usage.total_tokens, Some(50));
    }

    #[test]
    fn test_rerank_usage_clone() {
        let usage = RerankUsage {
            query_tokens: Some(5),
            document_tokens: Some(20),
            total_tokens: Some(25),
            search_units: Some(1),
        };

        let cloned = usage.clone();
        assert_eq!(usage.query_tokens, cloned.query_tokens);
        assert_eq!(usage.search_units, cloned.search_units);
    }

    #[test]
    fn test_rerank_usage_serialization() {
        let usage = RerankUsage {
            query_tokens: Some(10),
            document_tokens: None,
            total_tokens: Some(50),
            search_units: None,
        };

        let json = serde_json::to_value(&usage).unwrap();
        assert_eq!(json["query_tokens"], 10);
        assert_eq!(json["total_tokens"], 50);
        // None fields should not be serialized
        assert!(!json.as_object().unwrap().contains_key("document_tokens"));
        assert!(!json.as_object().unwrap().contains_key("search_units"));
    }

    #[test]
    fn test_rerank_usage_deserialization() {
        let json = r#"{
            "query_tokens": 12,
            "document_tokens": 80,
            "total_tokens": 92,
            "search_units": 3
        }"#;

        let usage: RerankUsage = serde_json::from_str(json).unwrap();
        assert_eq!(usage.query_tokens, Some(12));
        assert_eq!(usage.document_tokens, Some(80));
        assert_eq!(usage.total_tokens, Some(92));
        assert_eq!(usage.search_units, Some(3));
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_rerank_workflow() {
        // Create a request
        let request = RerankRequest {
            model: "cohere/rerank-english-v3.0".to_string(),
            query: "What is machine learning?".to_string(),
            documents: vec![
                RerankDocument::text("Machine learning is a branch of AI."),
                RerankDocument::text("Python is a programming language."),
                RerankDocument::text("Neural networks power deep learning."),
            ],
            top_n: Some(2),
            return_documents: Some(true),
            max_chunks_per_doc: None,
            truncation: None,
            extra_params: HashMap::new(),
        };

        assert_eq!(request.documents.len(), 3);

        // Simulate a response (results should be ordered by relevance)
        let response = RerankResponse {
            id: "workflow-test".to_string(),
            results: vec![
                RerankResult {
                    index: 0,
                    relevance_score: 0.95,
                    document: Some(request.documents[0].clone()),
                },
                RerankResult {
                    index: 2,
                    relevance_score: 0.78,
                    document: Some(request.documents[2].clone()),
                },
            ],
            model: request.model.clone(),
            usage: Some(RerankUsage {
                query_tokens: Some(5),
                document_tokens: Some(25),
                total_tokens: Some(30),
                search_units: Some(1),
            }),
            meta: HashMap::new(),
        };

        // Verify response
        assert_eq!(response.results.len(), 2);
        assert!(response.results[0].relevance_score > response.results[1].relevance_score);
        assert_eq!(response.results[0].index, 0);
        assert!(response.usage.is_some());
    }

    #[test]
    fn test_mixed_document_types() {
        let documents = vec![
            RerankDocument::text("Simple text"),
            RerankDocument::structured("Structured text"),
            RerankDocument::Structured {
                text: "Full structured".to_string(),
                title: Some("Title".to_string()),
                id: Some("doc-1".to_string()),
                metadata: HashMap::new(),
            },
        ];

        // All should provide text
        for doc in &documents {
            assert!(!doc.get_text().is_empty());
        }

        // Only the last should have an ID
        assert!(documents[0].get_id().is_none());
        assert!(documents[1].get_id().is_none());
        assert_eq!(documents[2].get_id(), Some("doc-1"));
    }

    #[test]
    fn test_relevance_score_range() {
        let results = vec![
            RerankResult {
                index: 0,
                relevance_score: 1.0,
                document: None,
            },
            RerankResult {
                index: 1,
                relevance_score: 0.5,
                document: None,
            },
            RerankResult {
                index: 2,
                relevance_score: 0.0,
                document: None,
            },
        ];

        for result in &results {
            assert!(result.relevance_score >= 0.0);
            assert!(result.relevance_score <= 1.0);
        }
    }
}
