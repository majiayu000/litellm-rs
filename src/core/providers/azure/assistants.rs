//! Azure OpenAI Assistants API
//!
//! AI assistants with function calling and code interpreter

use async_trait::async_trait;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// NOTE: Using local stub types; base_llm shared types not yet implemented.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAssistantRequest {
    pub model: String,
    pub name: Option<String>,
    pub description: Option<String>,
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAssistantResponse {
    pub id: String,
    pub object: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAssistantsResponse {
    pub data: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveAssistantResponse {
    pub id: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifyAssistantRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteAssistantResponse {
    pub id: String,
    pub deleted: bool,
}

#[derive(Debug, Clone)]
pub struct AssistantApiConfig {
    pub api_key: Option<String>,
    pub api_base: Option<String>,
    pub headers: Option<HashMap<String, String>>,
}

impl AssistantApiConfig {
    pub fn new(
        api_key: Option<&str>,
        api_base: Option<&str>,
        headers: Option<HashMap<String, String>>,
    ) -> Self {
        Self {
            api_key: api_key.map(|s| s.to_string()),
            api_base: api_base.map(|s| s.to_string()),
            headers,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateThreadRequest {
    pub messages: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateThreadResponse {
    pub id: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveThreadResponse {
    pub id: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifyThreadRequest {
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteThreadResponse {
    pub id: String,
    pub deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageRequest {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateMessageResponse {
    pub id: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMessagesResponse {
    pub data: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveMessageResponse {
    pub id: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRunRequest {
    pub assistant_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateRunResponse {
    pub id: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListRunsResponse {
    pub data: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrieveRunResponse {
    pub id: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitToolOutputsRequest {
    pub tool_outputs: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitToolOutputsResponse {
    pub id: String,
    pub object: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CancelRunResponse {
    pub id: String,
    pub object: String,
}

use crate::core::providers::base::HttpErrorMapper;
use crate::core::providers::unified_provider::ProviderError;

/// AssistantError is a type alias for ProviderError (unified error handling)
pub type AssistantError = ProviderError;

#[async_trait]
pub trait BaseAssistantHandler {
    async fn create_assistant(
        &self,
        request: CreateAssistantRequest,
        config: &AssistantApiConfig,
    ) -> Result<CreateAssistantResponse, AssistantError>;
    async fn list_assistants(
        &self,
        limit: Option<i32>,
        order: Option<&str>,
        after: Option<&str>,
        before: Option<&str>,
        config: &AssistantApiConfig,
    ) -> Result<ListAssistantsResponse, AssistantError>;
    async fn retrieve_assistant(
        &self,
        assistant_id: &str,
        config: &AssistantApiConfig,
    ) -> Result<RetrieveAssistantResponse, AssistantError>;
    async fn modify_assistant(
        &self,
        assistant_id: &str,
        request: ModifyAssistantRequest,
        config: &AssistantApiConfig,
    ) -> Result<RetrieveAssistantResponse, AssistantError>;
    async fn delete_assistant(
        &self,
        assistant_id: &str,
        config: &AssistantApiConfig,
    ) -> Result<DeleteAssistantResponse, AssistantError>;
}
use super::client::AzureClient;
use super::config::AzureConfig;
use super::utils::AzureUtils;

#[derive(Debug)]
pub struct AzureAssistantHandler {
    client: AzureClient,
}

impl AzureAssistantHandler {
    pub fn new(config: AzureConfig) -> Result<Self, ProviderError> {
        let client = AzureClient::new(config)?;
        Ok(Self { client })
    }

    fn build_api_url(&self, resource: &str, path: &str) -> String {
        let endpoint = self
            .client
            .get_config()
            .azure_endpoint
            .as_deref()
            .unwrap_or("")
            .trim_end_matches('/');
        let base = if endpoint.is_empty() {
            format!("openai/{}", resource)
        } else {
            format!("{}/openai/{}", endpoint, resource)
        };

        format!(
            "{}{}?api-version={}",
            base,
            path,
            self.client.get_config().api_version
        )
    }

    fn build_assistants_url(&self, path: &str) -> String {
        self.build_api_url("assistants", path)
    }

    #[cfg(test)]
    fn build_threads_url(&self, path: &str) -> String {
        self.build_api_url("threads", path)
    }
}

#[async_trait]
impl BaseAssistantHandler for AzureAssistantHandler {
    async fn create_assistant(
        &self,
        request: CreateAssistantRequest,
        config: &AssistantApiConfig,
    ) -> Result<CreateAssistantResponse, AssistantError> {
        self.client
            .validate_api_base_override(config.api_base.as_deref())?;
        let api_key = config
            .api_key
            .as_deref()
            .or_else(|| self.client.get_config().api_key.as_deref())
            .ok_or_else(|| {
                ProviderError::authentication("azure", "Azure API key required".to_string())
            })?;

        let url = self.build_assistants_url("");

        let mut request_headers =
            AzureUtils::create_azure_headers(self.client.get_config(), api_key)
                .map_err(|e| ProviderError::configuration("azure", e.to_string()))?;

        if let Some(custom_headers) = &config.headers {
            for (key, value) in custom_headers {
                let header_name =
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                        ProviderError::network("azure", format!("Invalid header: {}", e))
                    })?;
                let header_value = reqwest::header::HeaderValue::from_str(value).map_err(|e| {
                    ProviderError::network("azure", format!("Invalid header: {}", e))
                })?;
                request_headers.insert(header_name, header_value);
            }
        }

        let response = self
            .client
            .request(Method::POST, &url)?
            .headers(request_headers)
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.map_err(|error| {
                ProviderError::network("azure", format!("failed to read error body: {error}"))
            })?;
            return Err(HttpErrorMapper::map_status_code("azure", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::serialization("azure", e.to_string()))
    }

    async fn list_assistants(
        &self,
        limit: Option<i32>,
        order: Option<&str>,
        after: Option<&str>,
        before: Option<&str>,
        config: &AssistantApiConfig,
    ) -> Result<ListAssistantsResponse, AssistantError> {
        self.client
            .validate_api_base_override(config.api_base.as_deref())?;
        let api_key = config
            .api_key
            .as_deref()
            .or_else(|| self.client.get_config().api_key.as_deref())
            .ok_or_else(|| {
                ProviderError::authentication("azure", "Azure API key required".to_string())
            })?;

        let mut url = self.build_assistants_url("");
        let mut query_params = Vec::new();

        if let Some(limit_val) = limit {
            query_params.push(format!("limit={}", limit_val));
        }
        if let Some(order_val) = order {
            query_params.push(format!("order={}", order_val));
        }
        if let Some(after_val) = after {
            query_params.push(format!("after={}", after_val));
        }
        if let Some(before_val) = before {
            query_params.push(format!("before={}", before_val));
        }

        if !query_params.is_empty() {
            url.push('&');
            url.push_str(&query_params.join("&"));
        }

        let mut request_headers =
            AzureUtils::create_azure_headers(self.client.get_config(), api_key)
                .map_err(|e| ProviderError::configuration("azure", e.to_string()))?;

        if let Some(custom_headers) = &config.headers {
            for (key, value) in custom_headers {
                let header_name =
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                        ProviderError::network("azure", format!("Invalid header: {}", e))
                    })?;
                let header_value = reqwest::header::HeaderValue::from_str(value).map_err(|e| {
                    ProviderError::network("azure", format!("Invalid header: {}", e))
                })?;
                request_headers.insert(header_name, header_value);
            }
        }

        let response = self
            .client
            .request(Method::GET, &url)?
            .headers(request_headers)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.map_err(|error| {
                ProviderError::network("azure", format!("failed to read error body: {error}"))
            })?;
            return Err(HttpErrorMapper::map_status_code("azure", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::serialization("azure", e.to_string()))
    }

    async fn retrieve_assistant(
        &self,
        assistant_id: &str,
        config: &AssistantApiConfig,
    ) -> Result<RetrieveAssistantResponse, AssistantError> {
        self.client
            .validate_api_base_override(config.api_base.as_deref())?;
        let api_key = config
            .api_key
            .as_deref()
            .or_else(|| self.client.get_config().api_key.as_deref())
            .ok_or_else(|| {
                ProviderError::authentication("azure", "Azure API key required".to_string())
            })?;

        let url = self.build_assistants_url(&format!("/{}", assistant_id));

        let mut request_headers =
            AzureUtils::create_azure_headers(self.client.get_config(), api_key)
                .map_err(|e| ProviderError::configuration("azure", e.to_string()))?;

        if let Some(custom_headers) = &config.headers {
            for (key, value) in custom_headers {
                let header_name =
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                        ProviderError::network("azure", format!("Invalid header: {}", e))
                    })?;
                let header_value = reqwest::header::HeaderValue::from_str(value).map_err(|e| {
                    ProviderError::network("azure", format!("Invalid header: {}", e))
                })?;
                request_headers.insert(header_name, header_value);
            }
        }

        let response = self
            .client
            .request(Method::GET, &url)?
            .headers(request_headers)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.map_err(|error| {
                ProviderError::network("azure", format!("failed to read error body: {error}"))
            })?;
            return Err(HttpErrorMapper::map_status_code("azure", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::serialization("azure", e.to_string()))
    }

    async fn modify_assistant(
        &self,
        assistant_id: &str,
        request: ModifyAssistantRequest,
        config: &AssistantApiConfig,
    ) -> Result<RetrieveAssistantResponse, AssistantError> {
        self.client
            .validate_api_base_override(config.api_base.as_deref())?;
        let api_key = config
            .api_key
            .as_deref()
            .or_else(|| self.client.get_config().api_key.as_deref())
            .ok_or_else(|| {
                ProviderError::authentication("azure", "Azure API key required".to_string())
            })?;

        let url = self.build_assistants_url(&format!("/{}", assistant_id));

        let mut request_headers =
            AzureUtils::create_azure_headers(self.client.get_config(), api_key)
                .map_err(|e| ProviderError::configuration("azure", e.to_string()))?;

        if let Some(custom_headers) = &config.headers {
            for (key, value) in custom_headers {
                let header_name =
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                        ProviderError::network("azure", format!("Invalid header: {}", e))
                    })?;
                let header_value = reqwest::header::HeaderValue::from_str(value).map_err(|e| {
                    ProviderError::network("azure", format!("Invalid header: {}", e))
                })?;
                request_headers.insert(header_name, header_value);
            }
        }

        let response = self
            .client
            .request(Method::POST, &url)?
            .headers(request_headers)
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.map_err(|error| {
                ProviderError::network("azure", format!("failed to read error body: {error}"))
            })?;
            return Err(HttpErrorMapper::map_status_code("azure", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::serialization("azure", e.to_string()))
    }

    async fn delete_assistant(
        &self,
        assistant_id: &str,
        config: &AssistantApiConfig,
    ) -> Result<DeleteAssistantResponse, AssistantError> {
        self.client
            .validate_api_base_override(config.api_base.as_deref())?;
        let api_key = config
            .api_key
            .as_deref()
            .or_else(|| self.client.get_config().api_key.as_deref())
            .ok_or_else(|| {
                ProviderError::authentication("azure", "Azure API key required".to_string())
            })?;

        let url = self.build_assistants_url(&format!("/{}", assistant_id));

        let mut request_headers =
            AzureUtils::create_azure_headers(self.client.get_config(), api_key)
                .map_err(|e| ProviderError::configuration("azure", e.to_string()))?;

        if let Some(custom_headers) = &config.headers {
            for (key, value) in custom_headers {
                let header_name =
                    reqwest::header::HeaderName::from_bytes(key.as_bytes()).map_err(|e| {
                        ProviderError::network("azure", format!("Invalid header: {}", e))
                    })?;
                let header_value = reqwest::header::HeaderValue::from_str(value).map_err(|e| {
                    ProviderError::network("azure", format!("Invalid header: {}", e))
                })?;
                request_headers.insert(header_name, header_value);
            }
        }

        let response = self
            .client
            .request(Method::DELETE, &url)?
            .headers(request_headers)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.map_err(|error| {
                ProviderError::network("azure", format!("failed to read error body: {error}"))
            })?;
            return Err(HttpErrorMapper::map_status_code("azure", status, &body));
        }

        response
            .json()
            .await
            .map_err(|e| ProviderError::serialization("azure", e.to_string()))
    }
}

pub struct AzureAssistantUtils;

impl AzureAssistantUtils {
    pub fn get_supported_assistant_models() -> Vec<&'static str> {
        vec!["gpt-4", "gpt-4-turbo", "gpt-4o", "gpt-35-turbo"]
    }

    pub fn validate_assistant_request(
        request: &CreateAssistantRequest,
    ) -> Result<(), AssistantError> {
        if !Self::get_supported_assistant_models().contains(&request.model.as_str()) {
            return Err(ProviderError::invalid_request(
                "azure",
                format!("Unsupported assistant model: {}", request.model),
            ));
        }

        if let Some(instructions) = &request.instructions
            && instructions.len() > 32768
        {
            return Err(ProviderError::invalid_request(
                "azure",
                "Instructions exceed maximum length of 32768 characters".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "assistants_tests.rs"]
mod tests;
