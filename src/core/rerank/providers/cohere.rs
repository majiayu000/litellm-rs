//! Cohere rerank provider implementation

use super::rerank_upstream_error;
use crate::core::rerank::service::RerankProvider;
use crate::core::rerank::types::{RerankRequest, RerankResponse, RerankResult, RerankUsage};
use crate::utils::error::gateway_error::{GatewayError, Result};
use async_trait::async_trait;
use std::collections::HashMap;

/// Cohere rerank provider implementation
pub struct CohereRerankProvider {
    /// API key
    api_key: String,
    /// API base URL
    base_url: String,
    /// HTTP client
    client: reqwest::Client,
}

impl CohereRerankProvider {
    /// Create a new Cohere rerank provider
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.cohere.ai/v1".to_string(),
            client: crate::core::http::outbound::default_outbound_client().clone(),
        }
    }

    /// Set custom base URL
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

#[async_trait]
impl RerankProvider for CohereRerankProvider {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse> {
        // Extract model name (remove provider prefix)
        let model = if request.model.contains('/') {
            request
                .model
                .split('/')
                .next_back()
                .unwrap_or(&request.model)
        } else {
            &request.model
        };

        // Build Cohere request
        let documents: Vec<String> = request
            .documents
            .iter()
            .map(|d| d.get_text().to_string())
            .collect();

        let mut body = serde_json::json!({
            "model": model,
            "query": request.query,
            "documents": documents,
        });

        if let Some(top_n) = request.top_n {
            body["top_n"] = serde_json::json!(top_n);
        }

        if let Some(return_docs) = request.return_documents {
            body["return_documents"] = serde_json::json!(return_docs);
        }

        // Send request
        let response = self
            .client
            .post(format!("{}/rerank", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::Network(format!("Cohere rerank request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(rerank_upstream_error("cohere", status, error_text));
        }

        // Parse response
        let cohere_response: serde_json::Value = response.json().await.map_err(|e| {
            GatewayError::Validation(format!("Failed to parse Cohere response: {}", e))
        })?;

        // Convert to our response format
        let results = cohere_response["results"]
            .as_array()
            .ok_or_else(|| GatewayError::Validation("Missing results in response".to_string()))?
            .iter()
            .map(|r| {
                let index = r["index"].as_u64().unwrap_or(0) as usize;
                let relevance_score = r["relevance_score"].as_f64().unwrap_or(0.0);
                let document = if request.return_documents.unwrap_or(true) {
                    request.documents.get(index).cloned()
                } else {
                    None
                };

                RerankResult {
                    index,
                    relevance_score,
                    document,
                }
            })
            .collect();

        let usage = cohere_response.get("meta").and_then(|m| {
            m.get("billed_units").map(|bu| RerankUsage {
                query_tokens: None,
                document_tokens: None,
                total_tokens: None,
                search_units: bu
                    .get("search_units")
                    .and_then(|s| s.as_u64())
                    .map(|s| s as u32),
            })
        });

        Ok(RerankResponse {
            id: cohere_response["id"]
                .as_str()
                .unwrap_or("unknown")
                .to_string(),
            results,
            model: model.to_string(),
            usage,
            meta: HashMap::new(),
        })
    }

    fn provider_name(&self) -> &'static str {
        "cohere"
    }

    fn supports_model(&self, model: &str) -> bool {
        let model_name = model.split('/').next_back().unwrap_or(model);
        matches!(
            model_name,
            "rerank-english-v3.0"
                | "rerank-multilingual-v3.0"
                | "rerank-english-v2.0"
                | "rerank-multilingual-v2.0"
        )
    }

    fn supported_models(&self) -> Vec<&'static str> {
        vec![
            "rerank-english-v3.0",
            "rerank-multilingual-v3.0",
            "rerank-english-v2.0",
            "rerank-multilingual-v2.0",
        ]
    }
}
