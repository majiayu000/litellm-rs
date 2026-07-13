//! Cohere rerank provider implementation

use super::rerank_upstream_error;
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::base::{BaseConfig, BaseHttpClient};
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
    /// Policy-aware HTTP client.
    client: BaseHttpClient,
    endpoint_access: ProviderEndpointAccess,
    timeout_seconds: u64,
}

impl CohereRerankProvider {
    /// Create a new Cohere rerank provider
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::new_with_endpoint(
            api_key,
            "https://api.cohere.ai/v1",
            ProviderEndpointAccess::PublicOnly,
            30,
        )
    }

    /// Set custom base URL
    pub fn with_base_url(self, url: impl Into<String>) -> Result<Self> {
        Self::new_with_endpoint(
            self.api_key,
            url,
            self.endpoint_access,
            self.timeout_seconds,
        )
    }

    /// Create a provider bound to an exact endpoint policy and authority.
    pub fn new_with_endpoint(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        endpoint_access: ProviderEndpointAccess,
        timeout_seconds: u64,
    ) -> Result<Self> {
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let client = BaseHttpClient::new_for_provider(
            "cohere_rerank",
            BaseConfig {
                api_base: Some(base_url.clone()),
                endpoint_access,
                timeout: timeout_seconds,
                ..BaseConfig::default()
            },
        )?;
        Ok(Self {
            api_key: api_key.into(),
            base_url,
            client,
            endpoint_access,
            timeout_seconds,
        })
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
            .post(format!("{}/rerank", self.base_url))?
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::Network(format!("Cohere rerank request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.map_err(|error| {
                GatewayError::Network(format!(
                    "Failed to read Cohere rerank error response: {error}"
                ))
            })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_constructor_binds_private_cohere_authority() {
        let public = CohereRerankProvider::new_with_endpoint(
            "test-key",
            "http://127.0.0.1:11434/v1",
            ProviderEndpointAccess::PublicOnly,
            30,
        );
        assert!(public.is_err());

        let Ok(private) = CohereRerankProvider::new_with_endpoint(
            "test-key",
            "http://127.0.0.1:11434/v1",
            ProviderEndpointAccess::PrivateNetwork,
            30,
        ) else {
            panic!("private Cohere endpoint should build");
        };
        let Some(error) = private
            .client
            .post("http://127.0.0.1:11435/v1/rerank")
            .err()
        else {
            panic!("cross-authority Cohere request should fail");
        };
        assert!(error.to_string().contains("does not match"));
    }
}
