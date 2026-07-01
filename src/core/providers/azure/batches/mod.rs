//! Azure OpenAI Batch API
//!
//! Batch processing for multiple API requests

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// NOTE: Using local stub types; base_llm shared types not yet implemented.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBatchRequest {
    pub input_file_id: String,
    pub endpoint: String,
    pub completion_window: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBatchResponse {
    pub id: String,
    pub object: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListBatchesResponse {
    pub data: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveBatchResponse {
    pub id: String,
    pub object: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelBatchResponse {
    pub id: String,
    pub object: String,
    pub status: String,
}

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;

/// BatchError is a type alias for ProviderError (unified error handling)
pub type BatchError = ProviderError;

#[async_trait]
pub trait BaseBatchHandler {
    async fn create_batch(
        &self,
        request: CreateBatchRequest,
        api_key: Option<&str>,
        api_base: Option<&str>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<CreateBatchResponse, BatchError>;

    async fn list_batches(
        &self,
        after: Option<&str>,
        limit: Option<i32>,
        api_key: Option<&str>,
        api_base: Option<&str>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<ListBatchesResponse, BatchError>;

    async fn retrieve_batch(
        &self,
        batch_id: &str,
        api_key: Option<&str>,
        api_base: Option<&str>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<RetrieveBatchResponse, BatchError>;

    async fn cancel_batch(
        &self,
        batch_id: &str,
        api_key: Option<&str>,
        api_base: Option<&str>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<CancelBatchResponse, BatchError>;
}
use crate::core::providers::azure::client::AzureClient;
use crate::core::providers::azure::config::AzureConfig;
use crate::core::providers::azure::utils::AzureUtils;

#[derive(Debug)]
pub struct AzureBatchHandler {
    client: AzureClient,
}

impl AzureBatchHandler {
    pub fn new(config: AzureConfig) -> Result<Self, ProviderError> {
        let client = AzureClient::new(config)?;
        Ok(Self { client })
    }

    fn build_batches_url(&self, path: &str) -> String {
        format!(
            "{}openai/batches{}?api-version={}",
            self.client
                .get_config()
                .azure_endpoint
                .as_deref()
                .unwrap_or(""),
            path,
            self.client.get_config().api_version
        )
    }
}

#[async_trait]
impl BaseBatchHandler for AzureBatchHandler {
    async fn create_batch(
        &self,
        request: CreateBatchRequest,
        api_key: Option<&str>,
        _api_base: Option<&str>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<CreateBatchResponse, BatchError> {
        let api_key = api_key
            .map(|s| s.to_string())
            .or_else(|| self.client.get_config().api_key.clone())
            .ok_or_else(|| {
                ProviderError::authentication("azure", "Azure API key required".to_string())
            })?;

        let url = self.build_batches_url("");

        let mut request_headers =
            AzureUtils::create_azure_headers(self.client.get_config(), &api_key)
                .map_err(|e| ProviderError::configuration("azure", e.to_string()))?;

        if let Some(custom_headers) = headers {
            for (key, value) in custom_headers {
                let header_name =
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                        ProviderError::network("azure", format!("Invalid header: {}", e))
                    })?;
                let header_value = reqwest::header::HeaderValue::from_str(&value).map_err(|e| {
                    ProviderError::network("azure", format!("Invalid header: {}", e))
                })?;
                request_headers.insert(header_name, header_value);
            }
        }

        let response = self
            .client
            .get_http_client()
            .post(&url)
            .headers(request_headers)
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure", e.to_string()))?;

        if !response.status().is_success() {
            return Err(HttpErrorMapper::map_status_code(
                "azure",
                response.status().as_u16(),
                &response.text().await.unwrap_or_default(),
            ));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::serialization("azure", e.to_string()))
    }

    async fn list_batches(
        &self,
        after: Option<&str>,
        limit: Option<i32>,
        api_key: Option<&str>,
        _api_base: Option<&str>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<ListBatchesResponse, BatchError> {
        let api_key = api_key
            .map(|s| s.to_string())
            .or_else(|| self.client.get_config().api_key.clone())
            .ok_or_else(|| {
                ProviderError::authentication("azure", "Azure API key required".to_string())
            })?;

        let mut url = self.build_batches_url("");
        let mut query_params = Vec::new();

        if let Some(after_val) = after {
            query_params.push(format!("after={}", after_val));
        }
        if let Some(limit_val) = limit {
            query_params.push(format!("limit={}", limit_val));
        }

        if !query_params.is_empty() {
            url.push('&');
            url.push_str(&query_params.join("&"));
        }

        let mut request_headers =
            AzureUtils::create_azure_headers(self.client.get_config(), &api_key)
                .map_err(|e| ProviderError::configuration("azure", e.to_string()))?;

        if let Some(custom_headers) = headers {
            for (key, value) in custom_headers {
                let header_name =
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                        ProviderError::network("azure", format!("Invalid header: {}", e))
                    })?;
                let header_value = reqwest::header::HeaderValue::from_str(&value).map_err(|e| {
                    ProviderError::network("azure", format!("Invalid header: {}", e))
                })?;
                request_headers.insert(header_name, header_value);
            }
        }

        let response = self
            .client
            .get_http_client()
            .get(&url)
            .headers(request_headers)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure", e.to_string()))?;

        if !response.status().is_success() {
            return Err(HttpErrorMapper::map_status_code(
                "azure",
                response.status().as_u16(),
                &response.text().await.unwrap_or_default(),
            ));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::serialization("azure", e.to_string()))
    }

    async fn retrieve_batch(
        &self,
        batch_id: &str,
        api_key: Option<&str>,
        _api_base: Option<&str>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<RetrieveBatchResponse, BatchError> {
        let api_key = api_key
            .map(|s| s.to_string())
            .or_else(|| self.client.get_config().api_key.clone())
            .ok_or_else(|| {
                ProviderError::authentication("azure", "Azure API key required".to_string())
            })?;

        let url = self.build_batches_url(&format!("/{}", batch_id));

        let mut request_headers =
            AzureUtils::create_azure_headers(self.client.get_config(), &api_key)
                .map_err(|e| ProviderError::configuration("azure", e.to_string()))?;

        if let Some(custom_headers) = headers {
            for (key, value) in custom_headers {
                let header_name =
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                        ProviderError::network("azure", format!("Invalid header: {}", e))
                    })?;
                let header_value = reqwest::header::HeaderValue::from_str(&value).map_err(|e| {
                    ProviderError::network("azure", format!("Invalid header: {}", e))
                })?;
                request_headers.insert(header_name, header_value);
            }
        }

        let response = self
            .client
            .get_http_client()
            .get(&url)
            .headers(request_headers)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure", e.to_string()))?;

        if !response.status().is_success() {
            return Err(HttpErrorMapper::map_status_code(
                "azure",
                response.status().as_u16(),
                &response.text().await.unwrap_or_default(),
            ));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::serialization("azure", e.to_string()))
    }

    async fn cancel_batch(
        &self,
        batch_id: &str,
        api_key: Option<&str>,
        _api_base: Option<&str>,
        headers: Option<HashMap<String, String>>,
    ) -> Result<CancelBatchResponse, BatchError> {
        let api_key = api_key
            .map(|s| s.to_string())
            .or_else(|| self.client.get_config().api_key.clone())
            .ok_or_else(|| {
                ProviderError::authentication("azure", "Azure API key required".to_string())
            })?;

        let url = self.build_batches_url(&format!("/{}/cancel", batch_id));

        let mut request_headers =
            AzureUtils::create_azure_headers(self.client.get_config(), &api_key)
                .map_err(|e| ProviderError::configuration("azure", e.to_string()))?;

        if let Some(custom_headers) = headers {
            for (key, value) in custom_headers {
                let header_name =
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                        ProviderError::network("azure", format!("Invalid header: {}", e))
                    })?;
                let header_value = reqwest::header::HeaderValue::from_str(&value).map_err(|e| {
                    ProviderError::network("azure", format!("Invalid header: {}", e))
                })?;
                request_headers.insert(header_name, header_value);
            }
        }

        let response = self
            .client
            .get_http_client()
            .post(&url)
            .headers(request_headers)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure", e.to_string()))?;

        if !response.status().is_success() {
            return Err(HttpErrorMapper::map_status_code(
                "azure",
                response.status().as_u16(),
                &response.text().await.unwrap_or_default(),
            ));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::serialization("azure", e.to_string()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AzureBatchJob {
    pub id: String,
    pub object: String,
    pub endpoint: String,
    pub errors: Option<AzureBatchErrors>,
    pub input_file_id: String,
    pub completion_window: String,
    pub status: String,
    pub output_file_id: Option<String>,
    pub error_file_id: Option<String>,
    pub created_at: u64,
    pub in_progress_at: Option<u64>,
    pub expires_at: Option<u64>,
    pub finalizing_at: Option<u64>,
    pub completed_at: Option<u64>,
    pub failed_at: Option<u64>,
    pub expired_at: Option<u64>,
    pub cancelling_at: Option<u64>,
    pub cancelled_at: Option<u64>,
    pub request_counts: AzureBatchRequestCounts,
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AzureBatchErrors {
    pub object: String,
    pub data: Vec<AzureBatchErrorData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AzureBatchErrorData {
    pub code: String,
    pub message: String,
    pub param: Option<String>,
    pub line: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AzureBatchRequestCounts {
    pub total: u32,
    pub completed: u32,
    pub failed: u32,
}

pub struct AzureBatchUtils;

impl AzureBatchUtils {
    pub fn get_supported_batch_endpoints() -> Vec<&'static str> {
        vec!["/v1/chat/completions", "/v1/completions", "/v1/embeddings"]
    }

    pub fn validate_batch_request(request: &CreateBatchRequest) -> Result<(), BatchError> {
        if !Self::get_supported_batch_endpoints().contains(&request.endpoint.as_str()) {
            return Err(ProviderError::invalid_request(
                "azure",
                format!("Unsupported batch endpoint: {}", request.endpoint),
            ));
        }

        if request.input_file_id.is_empty() {
            return Err(ProviderError::invalid_request(
                "azure",
                "Input file ID is required".to_string(),
            ));
        }

        if request.completion_window != "24h" {
            return Err(ProviderError::invalid_request(
                "azure",
                "Only 24h completion window is supported".to_string(),
            ));
        }

        Ok(())
    }

    pub fn estimate_batch_processing_time(request_count: u32) -> std::time::Duration {
        // Rough estimate: 1 request per second processing
        std::time::Duration::from_secs(request_count as u64)
    }
}

#[cfg(test)]
#[path = "batches_tests.rs"]
mod tests;
