//! Azure AI Rerank Handler
//!
//! Complete Cohere-compatible reranking functionality for Azure AI services following unified architecture

use reqwest::header::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::config::{AzureAIConfig, AzureAIEndpointType};
use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;
use crate::utils::net::http::create_custom_client_with_headers;

/// Azure AI rerank handler following unified architecture
#[derive(Debug, Clone)]
pub struct AzureAIRerankHandler {
    config: AzureAIConfig,
    client: reqwest::Client,
}

impl AzureAIRerankHandler {
    /// Create a new rerank handler
    pub fn new(config: AzureAIConfig) -> Result<Self, ProviderError> {
        // Create headers for the client
        let mut headers = HeaderMap::new();
        let default_headers = config
            .create_default_headers()
            .map_err(|e| ProviderError::configuration("azure_ai", &e))?;

        for (key, value) in default_headers {
            let header_name =
                reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                    ProviderError::configuration("azure_ai", format!("Invalid header name: {}", e))
                })?;
            let header_value = reqwest::header::HeaderValue::from_str(&value).map_err(|e| {
                ProviderError::configuration("azure_ai", format!("Invalid header value: {}", e))
            })?;
            headers.insert(header_name, header_value);
        }

        let client = create_custom_client_with_headers(config.timeout(), headers).map_err(|e| {
            ProviderError::configuration("azure_ai", format!("Failed to create HTTP client: {}", e))
        })?;

        Ok(Self { config, client })
    }

    /// Handle rerank request
    pub async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse, ProviderError> {
        // Validate request
        AzureAIRerankUtils::validate_request(&request)?;

        // Transform request to Azure AI format
        let azure_request = AzureAIRerankUtils::transform_request(&request)?;

        // Build URL
        let url = self
            .config
            .build_endpoint_url(AzureAIEndpointType::Rerank.as_path())
            .map_err(|e| ProviderError::configuration("azure_ai", &e))?;

        // Execute request
        let response = self
            .client
            .post(&url)
            .json(&azure_request)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure_ai", format!("Request failed: {}", e)))?;

        // Handle error responses
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(HttpErrorMapper::map_status_code(
                "azure_ai",
                status,
                &error_body,
            ));
        }

        // Parse response
        let response_json: Value = response.json().await.map_err(|e| {
            ProviderError::response_parsing("azure_ai", format!("Failed to parse response: {}", e))
        })?;

        // Transform to standard format
        AzureAIRerankUtils::transform_response(response_json, &request.model)
    }
}

/// Rerank request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankRequest {
    /// Model to use for reranking
    pub model: String,
    /// Query text
    pub query: String,
    /// List of documents to rerank
    pub documents: Vec<String>,
    /// Maximum number of documents to return
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_n: Option<u32>,
    /// Return documents with their text
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_documents: Option<bool>,
    /// Maximum number of characters per document chunk
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_chunks_per_doc: Option<u32>,
    /// User identifier
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Rerank response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResponse {
    /// Response ID
    pub id: String,
    /// Results array
    pub results: Vec<RerankResult>,
    /// Usage information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<RerankUsage>,
}

/// Individual rerank result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResult {
    /// Document index in original list
    pub index: u32,
    /// Relevance score (higher = more relevant)
    pub relevance_score: f64,
    /// Document text (if return_documents=true)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document: Option<RerankDocument>,
}

/// Document in rerank result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankDocument {
    /// Document text
    pub text: String,
}

/// Rerank usage statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankUsage {
    /// Number of search units used
    pub search_units: u32,
}

/// Utility struct for Azure AI rerank operations
pub struct AzureAIRerankUtils;

impl AzureAIRerankUtils {
    /// Validate rerank request
    pub fn validate_request(request: &RerankRequest) -> Result<(), ProviderError> {
        if request.query.trim().is_empty() {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Query cannot be empty",
            ));
        }

        if request.model.is_empty() {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Model cannot be empty",
            ));
        }

        if request.documents.is_empty() {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Documents list cannot be empty",
            ));
        }

        // Validate number of documents
        if request.documents.len() > 1000 {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Maximum 1000 documents allowed",
            ));
        }

        // Validate top_n
        if let Some(top_n) = request.top_n
            && (top_n == 0 || top_n > 1000)
        {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "top_n must be between 1 and 1000",
            ));
        }

        // Validate document lengths
        for (i, doc) in request.documents.iter().enumerate() {
            if doc.len() > 10000 {
                return Err(ProviderError::invalid_request(
                    "azure_ai",
                    format!("Document {} exceeds 10,000 character limit", i),
                ));
            }
        }

        Ok(())
    }

    /// Transform RerankRequest to Azure AI format
    pub fn transform_request(request: &RerankRequest) -> Result<Value, ProviderError> {
        let mut azure_request = json!({
            "model": request.model,
            "query": request.query,
            "documents": request.documents,
        });

        // Add optional parameters
        if let Some(top_n) = request.top_n {
            azure_request["top_n"] = json!(top_n);
        }

        if let Some(return_documents) = request.return_documents {
            azure_request["return_documents"] = json!(return_documents);
        }

        if let Some(max_chunks_per_doc) = request.max_chunks_per_doc {
            azure_request["max_chunks_per_doc"] = json!(max_chunks_per_doc);
        }

        if let Some(ref user) = request.user {
            azure_request["user"] = json!(user);
        }

        Ok(azure_request)
    }

    /// Transform Azure AI response to RerankResponse
    pub fn transform_response(
        response: Value,
        _model: &str,
    ) -> Result<RerankResponse, ProviderError> {
        let id = response["id"]
            .as_str()
            .unwrap_or(&format!("rerank-{}", chrono::Utc::now().timestamp()))
            .to_string();

        // Parse results array
        let results_array = response["results"].as_array().ok_or_else(|| {
            ProviderError::response_parsing("azure_ai", "Missing or invalid 'results' field")
        })?;

        let mut results = Vec::new();
        for result_item in results_array.iter() {
            let index = result_item["index"].as_u64().ok_or_else(|| {
                ProviderError::response_parsing("azure_ai", "Missing 'index' in result")
            })? as u32;

            let relevance_score = result_item["relevance_score"].as_f64().ok_or_else(|| {
                ProviderError::response_parsing("azure_ai", "Missing 'relevance_score' in result")
            })?;

            let document = result_item.get("document").and_then(|doc| {
                doc.get("text")
                    .and_then(|text| text.as_str())
                    .map(|text| RerankDocument {
                        text: text.to_string(),
                    })
            });

            results.push(RerankResult {
                index,
                relevance_score,
                document,
            });
        }

        // Parse usage information if available
        let usage = response.get("usage").map(|usage_data| RerankUsage {
            search_units: usage_data["search_units"].as_u64().unwrap_or(1) as u32,
        });

        Ok(RerankResponse { id, results, usage })
    }

    /// Get maximum documents supported by model
    pub fn get_max_documents(model: &str) -> u32 {
        match model {
            m if m.contains("cohere-rerank") => 1000,
            _ => 100,
        }
    }

    /// Get maximum document length supported by model
    pub fn get_max_document_length(model: &str) -> u32 {
        match model {
            m if m.contains("cohere-rerank") => 10000,
            _ => 5000,
        }
    }

    /// Calculate search units used
    pub fn calculate_search_units(documents: &[String]) -> u32 {
        // Simplified calculation: 1 search unit per document
        // Real implementation would consider document length and complexity
        documents.len() as u32
    }

    /// Get default top_n for model
    pub fn get_default_top_n(model: &str, num_documents: usize) -> u32 {
        let default = if model.contains("cohere-rerank") {
            10
        } else {
            5
        };

        std::cmp::min(default, num_documents as u32)
    }

    /// Validate query length and content
    pub fn validate_query(query: &str) -> Result<(), ProviderError> {
        if query.trim().is_empty() {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Query cannot be empty",
            ));
        }

        if query.len() > 1000 {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Query too long. Maximum length is 1000 characters",
            ));
        }

        Ok(())
    }

    /// Preprocess documents for better reranking
    pub fn preprocess_documents(documents: &[String]) -> Vec<String> {
        documents
            .iter()
            .map(|doc| {
                // Basic preprocessing: trim whitespace and normalize
                doc.trim().to_string()
            })
            .filter(|doc| !doc.is_empty())
            .collect()
    }

    /// Sort results by relevance score (descending)
    pub fn sort_results_by_score(results: &mut [RerankResult]) {
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

/// Rerank model capabilities
#[derive(Debug, Clone)]
pub struct RerankModelCapabilities {
    pub model_name: String,
    pub max_documents: u32,
    pub max_document_length: u32,
    pub max_query_length: u32,
    pub default_top_n: u32,
    pub supports_multilingual: bool,
    pub cost_per_search_unit: f64,
}

impl RerankModelCapabilities {
    /// Get capabilities for rerank models
    pub fn for_model(model: &str) -> Self {
        match model {
            m if m.contains("cohere-rerank-v3.5") => Self {
                model_name: "Cohere Rerank v3.5".to_string(),
                max_documents: 1000,
                max_document_length: 10000,
                max_query_length: 1000,
                default_top_n: 10,
                supports_multilingual: true,
                cost_per_search_unit: 0.002,
            },
            m if m.contains("cohere-rerank-v3") => Self {
                model_name: "Cohere Rerank v3".to_string(),
                max_documents: 1000,
                max_document_length: 10000,
                max_query_length: 1000,
                default_top_n: 10,
                supports_multilingual: true,
                cost_per_search_unit: 0.002,
            },
            _ => Self {
                model_name: "Unknown".to_string(),
                max_documents: 100,
                max_document_length: 5000,
                max_query_length: 500,
                default_top_n: 5,
                supports_multilingual: false,
                cost_per_search_unit: 0.001,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::azure_ai::config::AzureAIConfig;

    #[test]
    fn test_rerank_utils_validation() {
        let mut request = RerankRequest {
            model: "cohere-rerank-v3".to_string(),
            query: "What is machine learning?".to_string(),
            documents: vec![
                "Machine learning is a subset of AI".to_string(),
                "Deep learning uses neural networks".to_string(),
            ],
            top_n: Some(2),
            return_documents: Some(true),
            max_chunks_per_doc: None,
            user: None,
        };

        // Valid request should pass
        assert!(AzureAIRerankUtils::validate_request(&request).is_ok());

        // Empty query should fail
        request.query = "".to_string();
        assert!(AzureAIRerankUtils::validate_request(&request).is_err());

        // Empty documents should fail
        request.query = "test".to_string();
        request.documents = vec![];
        assert!(AzureAIRerankUtils::validate_request(&request).is_err());

        // Too many documents should fail
        request.documents = vec!["doc".to_string(); 1001];
        assert!(AzureAIRerankUtils::validate_request(&request).is_err());
    }

    #[test]
    fn test_model_capabilities() {
        let caps = RerankModelCapabilities::for_model("cohere-rerank-v3.5");
        assert_eq!(caps.model_name, "Cohere Rerank v3.5");
        assert_eq!(caps.max_documents, 1000);
        assert_eq!(caps.max_document_length, 10000);
        assert!(caps.supports_multilingual);

        let v3_caps = RerankModelCapabilities::for_model("cohere-rerank-v3");
        assert_eq!(v3_caps.model_name, "Cohere Rerank v3");
        assert_eq!(v3_caps.cost_per_search_unit, 0.002);
    }

    #[test]
    fn test_search_units_calculation() {
        let documents = vec!["doc1".to_string(), "doc2".to_string(), "doc3".to_string()];
        let units = AzureAIRerankUtils::calculate_search_units(&documents);
        assert_eq!(units, 3);
    }

    #[test]
    fn test_default_top_n() {
        let top_n = AzureAIRerankUtils::get_default_top_n("cohere-rerank-v3", 15);
        assert_eq!(top_n, 10); // Default for cohere models

        let small_top_n = AzureAIRerankUtils::get_default_top_n("cohere-rerank-v3", 5);
        assert_eq!(small_top_n, 5); // Should use actual document count when smaller
    }

    #[test]
    fn test_document_preprocessing() {
        let docs = vec![
            "  Document with spaces  ".to_string(),
            "".to_string(), // Empty document
            "Normal document".to_string(),
        ];

        let processed = AzureAIRerankUtils::preprocess_documents(&docs);
        assert_eq!(processed.len(), 2); // Empty document should be filtered
        assert_eq!(processed[0], "Document with spaces");
    }

    #[test]
    fn test_query_validation() {
        assert!(AzureAIRerankUtils::validate_query("What is AI?").is_ok());
        assert!(AzureAIRerankUtils::validate_query("").is_err());
        assert!(AzureAIRerankUtils::validate_query(&"x".repeat(1001)).is_err());
    }

    #[test]
    fn test_request_transformation() {
        let request = RerankRequest {
            model: "cohere-rerank-v3".to_string(),
            query: "machine learning".to_string(),
            documents: vec!["AI is great".to_string(), "ML is powerful".to_string()],
            top_n: Some(2),
            return_documents: Some(true),
            max_chunks_per_doc: Some(1),
            user: Some("test-user".to_string()),
        };

        let result = AzureAIRerankUtils::transform_request(&request);
        assert!(result.is_ok());

        let azure_request = result.unwrap();
        assert_eq!(azure_request["model"], "cohere-rerank-v3");
        assert_eq!(azure_request["query"], "machine learning");
        assert_eq!(azure_request["top_n"], 2);
        assert_eq!(azure_request["return_documents"], true);
        assert_eq!(azure_request["user"], "test-user");
    }

    #[test]
    fn test_result_sorting() {
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

        AzureAIRerankUtils::sort_results_by_score(&mut results);

        // Should be sorted by descending relevance score
        assert_eq!(results[0].relevance_score, 0.9);
        assert_eq!(results[1].relevance_score, 0.5);
        assert_eq!(results[2].relevance_score, 0.3);
    }

    #[test]
    fn test_handler_creation() {
        let config = AzureAIConfig::new("azure_ai");
        // Test that handler can be created without errors
        let _result = AzureAIRerankHandler::new(config);
    }
}
