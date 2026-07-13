//! OpenAI Fine-tuning Provider
//!
//! Implementation of fine-tuning for OpenAI API.

use async_trait::async_trait;
use reqwest::{Method, Url, header::RETRY_AFTER};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};

use super::{FineTuningError, FineTuningProvider, FineTuningResult};
use crate::core::fine_tuning::config::ProviderFineTuningConfig;
use crate::core::fine_tuning::types::{
    CreateJobRequest, FineTuningCheckpoint, FineTuningJob, ListEventsParams, ListEventsResponse,
    ListJobsParams, ListJobsResponse,
};
use crate::core::providers::base::{BaseConfig, BaseHttpClient};

/// OpenAI fine-tuning provider
pub struct OpenAIFineTuningProvider {
    config: ProviderFineTuningConfig,
    client: BaseHttpClient,
    api_base: String,
    provider_name: String,
}

impl OpenAIFineTuningProvider {
    /// Create a new OpenAI fine-tuning provider
    pub fn new(config: ProviderFineTuningConfig) -> FineTuningResult<Self> {
        Self::new_named(config, "openai")
    }

    /// Create a new OpenAI-compatible fine-tuning provider with a gateway provider name.
    pub fn new_named(
        config: ProviderFineTuningConfig,
        provider_name: impl Into<String>,
    ) -> FineTuningResult<Self> {
        let api_base = config
            .api_base
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
            .trim_end_matches('/')
            .to_string();
        let client = BaseHttpClient::new_for_provider(
            "openai_fine_tuning",
            BaseConfig {
                api_base: Some(api_base.clone()),
                endpoint_access: config.endpoint_access,
                timeout: config.timeout_seconds,
                ..BaseConfig::default()
            },
        )
        .map_err(|error| {
            FineTuningError::provider(format!(
                "Failed to create policy-aware fine-tuning client: {error}"
            ))
        })?;

        Ok(Self {
            config,
            client,
            api_base,
            provider_name: provider_name.into(),
        })
    }

    /// Create from API key
    pub fn from_api_key(api_key: impl Into<String>) -> FineTuningResult<Self> {
        Self::new(ProviderFineTuningConfig::new().api_key(api_key))
    }

    /// Create from environment variable
    pub fn from_env() -> FineTuningResult<Option<Self>> {
        std::env::var("OPENAI_API_KEY")
            .ok()
            .map(Self::from_api_key)
            .transpose()
    }

    /// Build authorization header
    fn auth_header(&self) -> Result<String, FineTuningError> {
        self.config
            .api_key
            .as_ref()
            .map(|key| format!("Bearer {}", key))
            .ok_or_else(|| FineTuningError::auth("No API key configured"))
    }

    /// Build a provider endpoint URL without concatenating untrusted path segments.
    fn endpoint_url(&self, segments: &[&str]) -> FineTuningResult<Url> {
        let mut url = Url::parse(&self.api_base)
            .map_err(|e| FineTuningError::provider(format!("Invalid API base URL: {}", e)))?;
        {
            let mut path_segments = url.path_segments_mut().map_err(|_| {
                FineTuningError::provider("API base URL cannot accept path segments")
            })?;
            for segment in segments {
                path_segments.push(segment);
            }
        }
        Ok(url)
    }

    /// Make an authenticated request
    async fn request<T: DeserializeOwned>(
        &self,
        method: Method,
        url: Url,
        body: Option<serde_json::Value>,
        not_found_job_id: Option<&str>,
    ) -> FineTuningResult<T> {
        let auth = self.auth_header()?;
        let request_url = url.to_string();

        let mut request = self
            .client
            .request(method, url)
            .map_err(|error| {
                FineTuningError::network(format!("Request policy rejected URL: {error}"))
            })?
            .header("Authorization", auth);

        // Add organization header if configured
        if let Some(ref org) = self.config.organization_id {
            request = request.header("OpenAI-Organization", org);
        }

        // Add custom headers
        for (key, value) in &self.config.headers {
            request = request.header(key, value);
        }

        // Add body if present
        if let Some(body) = body {
            request = request.json(&body);
        }

        debug!("OpenAI fine-tuning request: {}", request_url);

        let response = request
            .send()
            .await
            .map_err(|e| FineTuningError::network(format!("Request failed: {}", e)))?;

        let status = response.status();

        if status.is_success() {
            response
                .json::<T>()
                .await
                .map_err(|e| FineTuningError::other(format!("Failed to parse response: {}", e)))
        } else {
            let retry_after_seconds = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(60);
            let error_text = response.text().await.map_err(|error| {
                FineTuningError::network(format!(
                    "Failed to read OpenAI fine-tuning error response payload: {error}"
                ))
            })?;
            let response_bytes = error_text.len();
            let safe_error_message = extract_openai_error_message(&error_text);
            warn!(
                %status,
                response_bytes,
                "OpenAI fine-tuning API returned non-success status"
            );

            match status.as_u16() {
                401 => Err(FineTuningError::auth("Invalid API key")),
                404 => Err(FineTuningError::job_not_found(
                    not_found_job_id.unwrap_or("unknown"),
                )),
                429 => Err(FineTuningError::RateLimited {
                    retry_after_seconds,
                }),
                _ => Err(FineTuningError::provider(format!(
                    "API error {}: {}",
                    status,
                    safe_error_message.unwrap_or_else(|| format!(
                        "upstream response omitted a safe error message (response_bytes={})",
                        response_bytes
                    ))
                ))),
            }
        }
    }

    fn annotate_job(&self, mut job: FineTuningJob) -> FineTuningJob {
        job.provider = Some(self.provider_name.clone());
        job
    }
}

fn extract_openai_error_message(error_text: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(error_text).ok()?;
    let error = value.get("error").unwrap_or(&value);
    let message = error.get("message").and_then(|message| message.as_str())?;
    let message = message.trim();

    if message.is_empty() {
        None
    } else {
        Some(message.to_string())
    }
}

/// OpenAI API request for creating a fine-tuning job
#[derive(Debug, Serialize)]
struct OpenAICreateJobRequest {
    model: String,
    training_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    validation_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hyperparameters: Option<OpenAIHyperparameters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suffix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seed: Option<u64>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
struct OpenAIHyperparameters {
    #[serde(skip_serializing_if = "Option::is_none")]
    n_epochs: Option<OpenAIHyperparamValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    batch_size: Option<OpenAIHyperparamValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    learning_rate_multiplier: Option<OpenAIHyperparamValue>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OpenAIHyperparamValue {
    Int(u32),
    Float(f64),
}

impl From<&CreateJobRequest> for OpenAICreateJobRequest {
    fn from(req: &CreateJobRequest) -> Self {
        let hyperparameters = req.hyperparameters.as_ref().map(|h| OpenAIHyperparameters {
            n_epochs: h.n_epochs.map(OpenAIHyperparamValue::Int),
            batch_size: h.batch_size.map(OpenAIHyperparamValue::Int),
            learning_rate_multiplier: h.learning_rate_multiplier.map(OpenAIHyperparamValue::Float),
        });

        Self {
            model: req.model.clone(),
            training_file: req.training_file.clone(),
            validation_file: req.validation_file.clone(),
            hyperparameters,
            suffix: req.suffix.clone(),
            seed: req.seed,
            metadata: req.metadata.clone(),
        }
    }
}

#[async_trait]
impl FineTuningProvider for OpenAIFineTuningProvider {
    fn name(&self) -> &'static str {
        "openai"
    }

    async fn create_job(&self, request: CreateJobRequest) -> FineTuningResult<FineTuningJob> {
        let openai_request = OpenAICreateJobRequest::from(&request);
        let url = self.endpoint_url(&["fine_tuning", "jobs"])?;
        let body = serde_json::to_value(&openai_request)
            .map_err(|e| FineTuningError::other(format!("Failed to serialize request: {}", e)))?;

        let mut job: FineTuningJob = self.request(Method::POST, url, Some(body), None).await?;

        // Add provider info
        job.provider = Some(self.provider_name.clone());

        // Copy metadata from request
        job.metadata = request.metadata;

        Ok(job)
    }

    async fn list_jobs(&self, params: ListJobsParams) -> FineTuningResult<ListJobsResponse> {
        let mut url = self.endpoint_url(&["fine_tuning", "jobs"])?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(after) = &params.after {
                query.append_pair("after", after);
            }
            if let Some(limit) = params.limit {
                query.append_pair("limit", &limit.to_string());
            }
        }

        let mut response: ListJobsResponse = self.request(Method::GET, url, None, None).await?;

        // Add provider info to all jobs
        for job in &mut response.data {
            job.provider = Some(self.provider_name.clone());
        }

        Ok(response)
    }

    async fn get_job(&self, job_id: &str) -> FineTuningResult<FineTuningJob> {
        let url = self.endpoint_url(&["fine_tuning", "jobs", job_id])?;

        let job: FineTuningJob = self.request(Method::GET, url, None, Some(job_id)).await?;

        Ok(self.annotate_job(job))
    }

    async fn cancel_job(&self, job_id: &str) -> FineTuningResult<FineTuningJob> {
        let url = self.endpoint_url(&["fine_tuning", "jobs", job_id, "cancel"])?;

        let job: FineTuningJob = self.request(Method::POST, url, None, Some(job_id)).await?;

        Ok(self.annotate_job(job))
    }

    async fn list_events(
        &self,
        job_id: &str,
        params: ListEventsParams,
    ) -> FineTuningResult<ListEventsResponse> {
        let mut url = self.endpoint_url(&["fine_tuning", "jobs", job_id, "events"])?;
        {
            let mut query = url.query_pairs_mut();
            if let Some(after) = &params.after {
                query.append_pair("after", after);
            }
            if let Some(limit) = params.limit {
                query.append_pair("limit", &limit.to_string());
            }
        }

        self.request(Method::GET, url, None, Some(job_id)).await
    }

    async fn list_checkpoints(&self, job_id: &str) -> FineTuningResult<Vec<FineTuningCheckpoint>> {
        let url = self.endpoint_url(&["fine_tuning", "jobs", job_id, "checkpoints"])?;

        #[derive(Deserialize)]
        struct CheckpointsResponse {
            data: Vec<FineTuningCheckpoint>,
        }

        let response: CheckpointsResponse =
            self.request(Method::GET, url, None, Some(job_id)).await?;

        Ok(response.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let Ok(provider) = OpenAIFineTuningProvider::from_api_key("sk-test") else {
            panic!("official provider should build");
        };
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_create_job_request_conversion() {
        let request = CreateJobRequest::new("gpt-3.5-turbo", "file-abc123")
            .validation_file("file-def456")
            .suffix("my-model");

        let openai_request = OpenAICreateJobRequest::from(&request);

        assert_eq!(openai_request.model, "gpt-3.5-turbo");
        assert_eq!(openai_request.training_file, "file-abc123");
        assert_eq!(
            openai_request.validation_file,
            Some("file-def456".to_string())
        );
        assert_eq!(openai_request.suffix, Some("my-model".to_string()));
    }

    #[test]
    fn test_hyperparameters_conversion() {
        use crate::core::fine_tuning::types::Hyperparameters;

        let request = CreateJobRequest::new("gpt-3.5-turbo", "file-abc123")
            .hyperparameters(Hyperparameters::new().n_epochs(3).batch_size(4));

        let openai_request = OpenAICreateJobRequest::from(&request);

        assert!(openai_request.hyperparameters.is_some());
    }

    #[test]
    fn test_auth_header() {
        let Ok(provider) = OpenAIFineTuningProvider::from_api_key("sk-test") else {
            panic!("official provider should build");
        };
        let header = provider.auth_header().unwrap();
        assert_eq!(header, "Bearer sk-test");
    }

    #[test]
    fn test_auth_header_missing() {
        let Ok(provider) = OpenAIFineTuningProvider::new(ProviderFineTuningConfig::new()) else {
            panic!("official provider should build without an API key");
        };
        let result = provider.auth_header();
        assert!(result.is_err());
    }

    #[test]
    fn policy_constructor_rejects_public_loopback_and_private_metadata() {
        let public = OpenAIFineTuningProvider::new(
            ProviderFineTuningConfig::new().api_base("http://127.0.0.1:11434/v1"),
        );
        assert!(public.is_err());

        let private = OpenAIFineTuningProvider::new(
            ProviderFineTuningConfig::new()
                .api_base("http://127.0.0.1:11434/v1")
                .endpoint_access(crate::core::net::ProviderEndpointAccess::PrivateNetwork),
        );
        assert!(private.is_ok());

        let metadata = OpenAIFineTuningProvider::new(
            ProviderFineTuningConfig::new()
                .api_base("http://169.254.169.254/latest")
                .endpoint_access(crate::core::net::ProviderEndpointAccess::PrivateNetwork),
        );
        assert!(metadata.is_err());
    }

    #[test]
    fn test_extract_openai_error_message_from_nested_error() {
        let message = extract_openai_error_message(
            r#"{"error":{"message":"Invalid training_file","type":"invalid_request_error"}}"#,
        );

        assert_eq!(message.as_deref(), Some("Invalid training_file"));
    }

    #[test]
    fn test_extract_openai_error_message_rejects_non_json_body() {
        let message = extract_openai_error_message("raw upstream failure with possible content");

        assert_eq!(message, None);
    }
}
