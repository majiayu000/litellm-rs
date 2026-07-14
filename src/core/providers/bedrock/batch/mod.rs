//! Batch Inference Module for Bedrock
//!
//! Handles S3-based batch processing jobs

use crate::core::providers::unified_provider::ProviderError;
use serde::{Deserialize, Serialize};

/// Batch inference job request
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBatchJobRequest {
    pub job_name: String,
    pub model_id: String,
    pub input_data_config: InputDataConfig,
    pub output_data_config: OutputDataConfig,
    pub role_arn: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<Tag>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_duration_in_hours: Option<u32>,
}

/// Input data configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputDataConfig {
    pub s3_input_data_config: S3InputDataConfig,
}

/// S3 input data configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct S3InputDataConfig {
    pub s3_uri: String,
    pub s3_input_format: String, // "JSONL"
}

/// Output data configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputDataConfig {
    pub s3_output_data_config: S3OutputDataConfig,
}

/// S3 output data configuration
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct S3OutputDataConfig {
    pub s3_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub s3_encryption_key_id: Option<String>,
}

/// Tag
#[derive(Debug, Serialize, Deserialize)]
pub struct Tag {
    pub key: String,
    pub value: String,
}

/// Batch job response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchJobResponse {
    pub job_arn: String,
}

/// Batch job details
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchJobDetails {
    pub job_arn: String,
    pub job_name: String,
    pub status: BatchJobStatus,
    pub model_id: String,
    pub input_data_config: InputDataConfig,
    pub output_data_config: OutputDataConfig,
    pub role_arn: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_expiration_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_duration_in_hours: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_statistics: Option<JobStatistics>,
}

/// Batch job status
#[derive(Debug, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BatchJobStatus {
    Submitted,
    InProgress,
    Completed,
    Failed,
    Stopping,
    Stopped,
    PartiallyCompleted,
    Expired,
    Validating,
    Scheduled,
}

/// Job statistics
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatistics {
    pub input_token_count: Option<u64>,
    pub output_token_count: Option<u64>,
}

/// Batch client
pub struct BatchClient<'a> {
    client: &'a crate::core::providers::bedrock::client::BedrockClient,
}

impl<'a> BatchClient<'a> {
    /// Create a new batch client
    pub fn new(client: &'a crate::core::providers::bedrock::client::BedrockClient) -> Self {
        Self { client }
    }

    /// Create a batch inference job
    pub async fn create_job(
        &self,
        request: CreateBatchJobRequest,
    ) -> Result<BatchJobResponse, ProviderError> {
        let response = self
            .client
            .send_request("", "model-invocation-job", &serde_json::to_value(request)?)
            .await?;
        let job_response: BatchJobResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))?;

        Ok(job_response)
    }

    /// Get batch job details
    pub async fn get_job(&self, job_identifier: &str) -> Result<BatchJobDetails, ProviderError> {
        let url = format!("model-invocation-job/{}", job_identifier);
        let response = self.client.send_get_request(&url).await?;
        let job_details: BatchJobDetails = response
            .json()
            .await
            .map_err(|e| ProviderError::response_parsing("bedrock", e.to_string()))?;

        Ok(job_details)
    }

    /// Stop a batch job
    pub async fn stop_job(&self, job_identifier: &str) -> Result<(), ProviderError> {
        let url = format!("model-invocation-job/{}/stop", job_identifier);
        self.client.send_empty_post_request(&url).await?;
        Ok(())
    }
}
