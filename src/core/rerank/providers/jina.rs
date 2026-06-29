//! Jina AI rerank provider implementation

use super::rerank_upstream_error;
use crate::core::rerank::service::RerankProvider;
use crate::core::rerank::types::{RerankRequest, RerankResponse, RerankResult, RerankUsage};
use crate::utils::error::gateway_error::{GatewayError, Result};
use async_trait::async_trait;
use std::collections::HashMap;

/// Jina AI rerank provider implementation
pub struct JinaRerankProvider {
    /// API key
    api_key: String,
    /// API base URL
    base_url: String,
    /// HTTP client
    client: reqwest::Client,
}

impl JinaRerankProvider {
    /// Create a new Jina rerank provider
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.jina.ai/v1".to_string(),
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
impl RerankProvider for JinaRerankProvider {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse> {
        let model = if request.model.contains('/') {
            request
                .model
                .split('/')
                .next_back()
                .unwrap_or(&request.model)
        } else {
            &request.model
        };

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

        let response = self
            .client
            .post(format!("{}/rerank", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::Network(format!("Jina rerank request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(rerank_upstream_error("jina", status, error_text));
        }

        let jina_response: serde_json::Value = response.json().await.map_err(|e| {
            GatewayError::Validation(format!("Failed to parse Jina response: {}", e))
        })?;

        let results = jina_response["results"]
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

        let usage = jina_response.get("usage").map(|u| RerankUsage {
            query_tokens: u
                .get("prompt_tokens")
                .and_then(|t| t.as_u64())
                .map(|t| t as u32),
            document_tokens: None,
            total_tokens: u
                .get("total_tokens")
                .and_then(|t| t.as_u64())
                .map(|t| t as u32),
            search_units: None,
        });

        Ok(RerankResponse {
            id: uuid::Uuid::new_v4().to_string(),
            results,
            model: model.to_string(),
            usage,
            meta: HashMap::new(),
        })
    }

    fn provider_name(&self) -> &'static str {
        "jina"
    }

    fn supports_model(&self, model: &str) -> bool {
        let model_name = model.split('/').next_back().unwrap_or(model);
        matches!(
            model_name,
            "jina-reranker-v2-base-multilingual"
                | "jina-reranker-v1-base-en"
                | "jina-reranker-v1-turbo-en"
        )
    }

    fn supported_models(&self) -> Vec<&'static str> {
        vec![
            "jina-reranker-v2-base-multilingual",
            "jina-reranker-v1-base-en",
            "jina-reranker-v1-turbo-en",
        ]
    }
}
