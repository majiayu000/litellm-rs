//! OpenAI Fine-tuning Provider
//!
//! Implementation of fine-tuning for OpenAI API.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, warn};

use super::{FineTuningError, FineTuningProvider, FineTuningResult};
use crate::core::fine_tuning::config::ProviderFineTuningConfig;
use crate::core::fine_tuning::types::{
    CreateJobRequest, FineTuningCheckpoint, FineTuningJob, ListEventsParams, ListEventsResponse,
    ListJobsParams, ListJobsResponse,
};
use crate::utils::net::http::create_custom_client;

/// OpenAI fine-tuning provider
pub struct OpenAIFineTuningProvider {
    config: ProviderFineTuningConfig,
    client: Client,
    api_base: String,
}

impl OpenAIFineTuningProvider {
    /// Create a new OpenAI fine-tuning provider
    pub fn new(config: ProviderFineTuningConfig) -> Self {
        let client =
            create_custom_client(Duration::from_secs(config.timeout_seconds)).unwrap_or_default();

        let api_base = config
            .api_base
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string());

        Self {
            config,
            client,
            api_base,
        }
    }

    /// Create from API key
    pub fn from_api_key(api_key: impl Into<String>) -> Self {
        Self::new(ProviderFineTuningConfig::new().api_key(api_key))
    }

    /// Create from environment variable
    pub fn from_env() -> Option<Self> {
        std::env::var("OPENAI_API_KEY").ok().map(Self::from_api_key)
    }

    /// Build authorization header
    fn auth_header(&self) -> Result<String, FineTuningError> {
        self.config
            .api_key
            .as_ref()
            .map(|key| format!("Bearer {}", key))
            .ok_or_else(|| FineTuningError::auth("No API key configured"))
    }

    /// Make an authenticated request
    async fn request<T: for<'de> Deserialize<'de>>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<impl Serialize>,
    ) -> FineTuningResult<T> {
        let url = format!("{}{}", self.api_base, path);
        let auth = self.auth_header()?;

        let mut request = self
            .client
            .request(method, &url)
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

        debug!("OpenAI fine-tuning request: {}", url);

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
            let error_body = response.text().await.unwrap_or_else(|e| {
                warn!(
                    "Failed to read OpenAI fine-tuning error response body: {}",
                    e
                );
                String::new()
            });
            warn!("OpenAI API error: {} - {}", status, error_body);

            match status.as_u16() {
                401 => Err(FineTuningError::auth("Invalid API key")),
                404 => {
                    // Try to extract job ID from error
                    Err(FineTuningError::job_not_found("unknown"))
                }
                429 => {
                    // Try to parse retry-after
                    Err(FineTuningError::RateLimited {
                        retry_after_seconds: 60,
                    })
                }
                _ => Err(FineTuningError::provider(format!(
                    "API error {}: {}",
                    status, error_body
                ))),
            }
        }
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

        let mut job: FineTuningJob = self
            .request(
                reqwest::Method::POST,
                "/fine_tuning/jobs",
                Some(&openai_request),
            )
            .await?;

        // Add provider info
        job.provider = Some("openai".to_string());

        // Copy metadata from request
        job.metadata = request.metadata;

        Ok(job)
    }

    async fn list_jobs(&self, params: ListJobsParams) -> FineTuningResult<ListJobsResponse> {
        let mut query = Vec::new();
        if let Some(after) = &params.after {
            query.push(format!("after={}", after));
        }
        if let Some(limit) = params.limit {
            query.push(format!("limit={}", limit));
        }

        let path = if query.is_empty() {
            "/fine_tuning/jobs".to_string()
        } else {
            format!("/fine_tuning/jobs?{}", query.join("&"))
        };

        let mut response: ListJobsResponse = self
            .request::<ListJobsResponse>(reqwest::Method::GET, &path, None::<()>)
            .await?;

        // Add provider info to all jobs
        for job in &mut response.data {
            job.provider = Some("openai".to_string());
        }

        Ok(response)
    }

    async fn get_job(&self, job_id: &str) -> FineTuningResult<FineTuningJob> {
        let path = format!("/fine_tuning/jobs/{}", job_id);

        let mut job: FineTuningJob = self
            .request::<FineTuningJob>(reqwest::Method::GET, &path, None::<()>)
            .await?;

        job.provider = Some("openai".to_string());

        Ok(job)
    }

    async fn cancel_job(&self, job_id: &str) -> FineTuningResult<FineTuningJob> {
        let path = format!("/fine_tuning/jobs/{}/cancel", job_id);

        let mut job: FineTuningJob = self
            .request::<FineTuningJob>(reqwest::Method::POST, &path, None::<()>)
            .await?;

        job.provider = Some("openai".to_string());

        Ok(job)
    }

    async fn list_events(
        &self,
        job_id: &str,
        params: ListEventsParams,
    ) -> FineTuningResult<ListEventsResponse> {
        let mut query = Vec::new();
        if let Some(after) = &params.after {
            query.push(format!("after={}", after));
        }
        if let Some(limit) = params.limit {
            query.push(format!("limit={}", limit));
        }

        let path = if query.is_empty() {
            format!("/fine_tuning/jobs/{}/events", job_id)
        } else {
            format!("/fine_tuning/jobs/{}/events?{}", job_id, query.join("&"))
        };

        self.request::<ListEventsResponse>(reqwest::Method::GET, &path, None::<()>)
            .await
    }

    async fn list_checkpoints(&self, job_id: &str) -> FineTuningResult<Vec<FineTuningCheckpoint>> {
        let path = format!("/fine_tuning/jobs/{}/checkpoints", job_id);

        #[derive(Deserialize)]
        struct CheckpointsResponse {
            data: Vec<FineTuningCheckpoint>,
        }

        let response: CheckpointsResponse = self
            .request::<CheckpointsResponse>(reqwest::Method::GET, &path, None::<()>)
            .await?;

        Ok(response.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = OpenAIFineTuningProvider::from_api_key("sk-test");
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
        let provider = OpenAIFineTuningProvider::from_api_key("sk-test");
        let header = provider.auth_header().unwrap();
        assert_eq!(header, "Bearer sk-test");
    }

    #[test]
    fn test_auth_header_missing() {
        let provider = OpenAIFineTuningProvider::new(ProviderFineTuningConfig::new());
        let result = provider.auth_header();
        assert!(result.is_err());
    }
}
