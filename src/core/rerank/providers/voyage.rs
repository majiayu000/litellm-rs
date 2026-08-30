//! Voyage AI rerank provider implementation.

use super::rerank_upstream_error;
use crate::core::net::ProviderEndpointAccess;
use crate::core::providers::ProviderError;
use crate::core::providers::base::{BaseConfig, BaseHttpClient};
use crate::core::rerank::service::RerankProvider;
use crate::core::rerank::types::{
    RerankDocument, RerankRequest, RerankResponse, RerankResult, RerankUsage,
};
use crate::utils::error::gateway_error::{GatewayError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const VOYAGE_RERANK_MODELS: &[&str] = &[
    "rerank-2.5",
    "rerank-2.5-lite",
    "rerank-2",
    "rerank-2-lite",
    "rerank-1",
    "rerank-lite-1",
];

#[derive(Clone)]
pub struct VoyageRerankProvider {
    api_key: String,
    base_url: String,
    client: BaseHttpClient,
}

impl VoyageRerankProvider {
    pub(crate) fn from_transport(
        api_key: String,
        base_url: String,
        client: BaseHttpClient,
    ) -> Self {
        Self {
            api_key,
            base_url,
            client,
        }
    }

    pub fn new_with_endpoint(
        api_key: impl Into<String>,
        base_url: impl Into<String>,
        endpoint_access: ProviderEndpointAccess,
        timeout_seconds: u64,
    ) -> Result<Self> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(GatewayError::Config(
                "Voyage rerank api_key is required".to_string(),
            ));
        }
        let base_url = base_url.into().trim_end_matches('/').to_string();
        let client = BaseHttpClient::new_for_provider(
            "voyage",
            BaseConfig {
                api_key: Some(api_key.clone()),
                api_base: Some(base_url.clone()),
                endpoint_access,
                timeout: timeout_seconds,
                ..BaseConfig::default()
            },
        )?;
        Ok(Self {
            api_key,
            base_url,
            client,
        })
    }

    fn native_model(model: &str) -> Option<&str> {
        let model = model.strip_prefix("voyage/").unwrap_or(model);
        (!model.contains('/') && VOYAGE_RERANK_MODELS.contains(&model)).then_some(model)
    }
}

#[derive(Serialize)]
struct VoyageRerankRequest<'a> {
    model: &'a str,
    query: &'a str,
    documents: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    return_documents: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncation: Option<bool>,
}

#[derive(Deserialize)]
struct VoyageRerankResponse {
    results: Vec<VoyageRerankResult>,
    usage: crate::core::providers::voyage::VoyageUsage,
}

#[derive(Deserialize)]
struct VoyageRerankResult {
    index: usize,
    relevance_score: f64,
}

#[async_trait]
impl RerankProvider for VoyageRerankProvider {
    async fn rerank(&self, request: RerankRequest) -> Result<RerankResponse> {
        let model = Self::native_model(&request.model).ok_or_else(|| {
            GatewayError::NotFound(format!("Unknown Voyage rerank model '{}'", request.model))
        })?;
        let body = VoyageRerankRequest {
            model,
            query: &request.query,
            documents: request
                .documents
                .iter()
                .map(RerankDocument::get_text)
                .collect(),
            top_k: request.top_n,
            return_documents: request.return_documents,
            truncation: request.truncation,
        };
        let response = self
            .client
            .post(format!("{}/rerank", self.base_url))?
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|error| self.client.map_preserved_request_error(error))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.map_err(|error| {
                GatewayError::Network(format!("Failed to read Voyage rerank error: {error}"))
            })?;
            return Err(rerank_upstream_error("voyage", status, body));
        }
        let response: VoyageRerankResponse = response.json().await.map_err(|error| {
            voyage_response_parsing_error(format!(
                "Failed to parse Voyage rerank response: {error}"
            ))
        })?;
        let mut indexes = HashSet::with_capacity(response.results.len());
        let results = response
            .results
            .into_iter()
            .map(|result| {
                if result.index >= request.documents.len() || !indexes.insert(result.index) {
                    return Err(voyage_response_parsing_error(format!(
                        "Invalid Voyage rerank result index {}",
                        result.index
                    )));
                }
                Ok(RerankResult {
                    index: result.index,
                    relevance_score: result.relevance_score,
                    document: request
                        .return_documents
                        .unwrap_or(false)
                        .then(|| request.documents[result.index].clone()),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(RerankResponse {
            id: uuid::Uuid::new_v4().to_string(),
            results,
            model: model.to_string(),
            usage: Some(RerankUsage {
                total_tokens: Some(response.usage.total_tokens),
                ..RerankUsage::default()
            }),
            meta: HashMap::new(),
        })
    }

    fn provider_name(&self) -> &'static str {
        "voyage"
    }

    fn supports_model(&self, model: &str) -> bool {
        Self::native_model(model).is_some()
    }

    fn supported_models(&self) -> Vec<&'static str> {
        VOYAGE_RERANK_MODELS.to_vec()
    }
}

fn voyage_response_parsing_error(message: String) -> GatewayError {
    GatewayError::Provider(ProviderError::response_parsing("voyage", message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_upstream_response_keeps_response_parsing_category() {
        let error = voyage_response_parsing_error("malformed JSON".to_string());

        assert!(matches!(
            error,
            GatewayError::Provider(ProviderError::ResponseParsing {
                provider: "voyage",
                ..
            })
        ));
    }
}
