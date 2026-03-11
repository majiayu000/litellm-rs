//! Cohere Rerank Handler
//!
//! Handles reranking requests for Cohere rerank models.
//! Used to reorder documents by relevance to a query.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::error::CohereError;

/// Cohere rerank request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankRequest {
    /// Model to use for reranking
    pub model: String,

    /// Query to rank documents against
    pub query: String,

    /// Documents to rerank
    pub documents: Vec<RerankDocument>,

    /// Maximum number of results to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_n: Option<u32>,

    /// Return document text in results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_documents: Option<bool>,

    /// Maximum chunks per document
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_chunks_per_doc: Option<u32>,

    /// Fields to rank by (for structured documents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rank_fields: Option<Vec<String>>,
}

/// Document for reranking
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RerankDocument {
    /// Simple text document
    Text(String),
    /// Structured document with fields
    Structured(Value),
}

/// Cohere rerank response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResponse {
    /// Response ID
    #[serde(default)]
    pub id: String,

    /// Reranked results
    pub results: Vec<RerankResult>,

    /// Metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<RerankMeta>,
}

/// Single rerank result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResult {
    /// Original index of the document
    pub index: u32,

    /// Relevance score (higher = more relevant)
    pub relevance_score: f64,

    /// Document text (if return_documents=true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<RerankResultDocument>,
}

/// Document in rerank result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResultDocument {
    /// Document text
    pub text: String,
}

/// Rerank metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankMeta {
    /// API version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_version: Option<Value>,

    /// Billed units
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billed_units: Option<RerankBilledUnits>,
}

/// Billed units for rerank
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankBilledUnits {
    /// Search units used
    #[serde(default)]
    pub search_units: u32,
}

/// Rerank handler utilities
pub struct CohereRerankHandler;

impl CohereRerankHandler {
    /// Transform RerankRequest to Cohere API format
    pub fn transform_request(request: &RerankRequest) -> Result<Value, CohereError> {
        Self::validate_request(request)?;

        let documents: Vec<Value> = request
            .documents
            .iter()
            .map(|doc| match doc {
                RerankDocument::Text(text) => json!(text),
                RerankDocument::Structured(obj) => obj.clone(),
            })
            .collect();

        let mut body = json!({
            "model": request.model,
            "query": request.query,
            "documents": documents,
        });

        if let Some(top_n) = request.top_n {
            body["top_n"] = json!(top_n);
        }

        if let Some(return_documents) = request.return_documents {
            body["return_documents"] = json!(return_documents);
        }

        if let Some(max_chunks_per_doc) = request.max_chunks_per_doc {
            body["max_chunks_per_doc"] = json!(max_chunks_per_doc);
        }

        if let Some(rank_fields) = &request.rank_fields {
            body["rank_fields"] = json!(rank_fields);
        }

        Ok(body)
    }

    /// Validate rerank request
    fn validate_request(request: &RerankRequest) -> Result<(), CohereError> {
        if request.query.trim().is_empty() {
            return Err(super::error::cohere_invalid_request(
                "Query cannot be empty",
            ));
        }

        if request.model.is_empty() {
            return Err(super::error::cohere_invalid_request(
                "Model cannot be empty",
            ));
        }

        if request.documents.is_empty() {
            return Err(super::error::cohere_invalid_request(
                "Documents list cannot be empty",
            ));
        }

        if request.documents.len() > 1000 {
            return Err(super::error::cohere_invalid_request(
                "Maximum 1000 documents allowed",
            ));
        }

        if let Some(top_n) = request.top_n
            && (top_n == 0 || top_n > 1000)
        {
            return Err(super::error::cohere_invalid_request(
                "top_n must be between 1 and 1000",
            ));
        }

        // Validate query length
        if request.query.len() > 2048 {
            return Err(super::error::cohere_invalid_request(
                "Query too long. Maximum length is 2048 characters",
            ));
        }

        Ok(())
    }

    /// Transform Cohere response to RerankResponse
    pub fn transform_response(response_json: Value) -> Result<RerankResponse, CohereError> {
        let id = response_json
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let results_array = response_json
            .get("results")
            .and_then(|r| r.as_array())
            .ok_or_else(|| {
                super::error::cohere_response_parsing("Missing or invalid 'results' field")
            })?;

        let mut results = Vec::new();
        for result_item in results_array {
            let index = result_item
                .get("index")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| super::error::cohere_response_parsing("Missing 'index' in result"))?
                as u32;

            let relevance_score = result_item
                .get("relevance_score")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| {
                    super::error::cohere_response_parsing("Missing 'relevance_score' in result")
                })?;

            let document = result_item.get("document").and_then(|doc| {
                doc.get("text")
                    .and_then(|t| t.as_str())
                    .map(|text| RerankResultDocument {
                        text: text.to_string(),
                    })
            });

            results.push(RerankResult {
                index,
                relevance_score,
                document,
            });
        }

        let meta = response_json.get("meta").map(|m| RerankMeta {
            api_version: m.get("api_version").cloned(),
            billed_units: m.get("billed_units").map(|b| RerankBilledUnits {
                search_units: b.get("search_units").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            }),
        });

        Ok(RerankResponse { id, results, meta })
    }

    /// Get maximum documents supported by model
    #[cfg(test)]
    pub fn get_max_documents(model: &str) -> u32 {
        match model {
            m if m.contains("-v3.5") => 1000,
            m if m.contains("-v3") => 1000,
            m if m.contains("-v2") => 1000,
            _ => 100,
        }
    }

    /// Get default top_n for model
    #[cfg(test)]
    pub fn get_default_top_n(model: &str, num_documents: usize) -> u32 {
        let default = match model {
            m if m.contains("rerank") => 10,
            _ => 5,
        };

        std::cmp::min(default, num_documents as u32)
    }

    /// Sort results by relevance score (descending)
    #[cfg(test)]
    pub fn sort_results_by_score(results: &mut [RerankResult]) {
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_request() -> RerankRequest {
        RerankRequest {
            model: "rerank-english-v3.0".to_string(),
            query: "What is machine learning?".to_string(),
            documents: vec![
                RerankDocument::Text("Machine learning is a subset of AI".to_string()),
                RerankDocument::Text("Deep learning uses neural networks".to_string()),
            ],
            top_n: Some(2),
            return_documents: Some(true),
            max_chunks_per_doc: None,
            rank_fields: None,
        }
    }

    #[test]
    fn test_transform_request() {
        let request = create_test_request();
        let body = CohereRerankHandler::transform_request(&request).unwrap();

        assert_eq!(body["model"], "rerank-english-v3.0");
        assert_eq!(body["query"], "What is machine learning?");
        assert_eq!(body["top_n"], 2);
        assert_eq!(body["return_documents"], true);
    }

    #[test]
    fn test_validate_empty_query() {
        let mut request = create_test_request();
        request.query = "".to_string();

        let result = CohereRerankHandler::transform_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_empty_documents() {
        let mut request = create_test_request();
        request.documents = vec![];

        let result = CohereRerankHandler::transform_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invalid_top_n() {
        let mut request = create_test_request();
        request.top_n = Some(0);

        let result = CohereRerankHandler::transform_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_transform_response() {
        let response = json!({
            "id": "test-id",
            "results": [
                {
                    "index": 0,
                    "relevance_score": 0.9,
                    "document": {"text": "ML is AI"}
                },
                {
                    "index": 1,
                    "relevance_score": 0.7,
                    "document": {"text": "DL uses NN"}
                }
            ],
            "meta": {
                "billed_units": {"search_units": 2}
            }
        });

        let result = CohereRerankHandler::transform_response(response).unwrap();

        assert_eq!(result.id, "test-id");
        assert_eq!(result.results.len(), 2);
        assert_eq!(result.results[0].relevance_score, 0.9);
        assert!(result.results[0].document.is_some());
    }

    #[test]
    fn test_sort_results_by_score() {
        let mut results = vec![
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
                relevance_score: 0.3,
                document: None,
            },
        ];

        CohereRerankHandler::sort_results_by_score(&mut results);

        assert_eq!(results[0].relevance_score, 0.9);
        assert_eq!(results[1].relevance_score, 0.5);
        assert_eq!(results[2].relevance_score, 0.3);
    }

    #[test]
    fn test_get_max_documents() {
        assert_eq!(
            CohereRerankHandler::get_max_documents("rerank-english-v3.0"),
            1000
        );
        assert_eq!(
            CohereRerankHandler::get_max_documents("rerank-multilingual-v3.0"),
            1000
        );
        assert_eq!(CohereRerankHandler::get_max_documents("unknown"), 100);
    }

    #[test]
    fn test_get_default_top_n() {
        assert_eq!(
            CohereRerankHandler::get_default_top_n("rerank-english-v3.0", 50),
            10
        );
        assert_eq!(
            CohereRerankHandler::get_default_top_n("rerank-english-v3.0", 5),
            5
        );
    }

    #[test]
    fn test_structured_documents() {
        let request = RerankRequest {
            model: "rerank-english-v3.0".to_string(),
            query: "test query".to_string(),
            documents: vec![RerankDocument::Structured(json!({
                "title": "Test Doc",
                "text": "Document content"
            }))],
            top_n: None,
            return_documents: None,
            max_chunks_per_doc: None,
            rank_fields: Some(vec!["text".to_string()]),
        };

        let body = CohereRerankHandler::transform_request(&request).unwrap();
        assert!(body["rank_fields"].as_array().is_some());
    }
}
