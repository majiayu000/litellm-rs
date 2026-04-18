//! Batch request validation logic

use super::super::types::*;
use super::core::BatchProcessor;
use crate::utils::error::gateway_error::{GatewayError, Result};

impl BatchProcessor {
    /// Validate batch request
    pub(super) async fn validate_batch_request(&self, request: &BatchRequest) -> Result<()> {
        // Check request count limits
        if request.requests.len() > 50000 {
            return Err(GatewayError::BadRequest(
                "Batch size exceeds maximum limit of 50,000 requests".to_string(),
            ));
        }

        if request.requests.is_empty() {
            return Err(GatewayError::BadRequest(
                "Batch must contain at least one request".to_string(),
            ));
        }

        // Validate individual requests
        for item in &request.requests {
            self.validate_batch_item(item, &request.batch_type).await?;
        }

        Ok(())
    }

    /// Validate individual batch item
    pub(super) async fn validate_batch_item(
        &self,
        item: &BatchItem,
        batch_type: &BatchType,
    ) -> Result<()> {
        // Validate custom_id
        if item.custom_id.is_empty() || item.custom_id.len() > 64 {
            return Err(GatewayError::BadRequest(
                "custom_id must be 1-64 characters".to_string(),
            ));
        }

        // Validate method
        if item.method != "POST" {
            return Err(GatewayError::BadRequest(
                "Only POST method is supported for batch requests".to_string(),
            ));
        }

        // Validate URL matches batch type
        match batch_type {
            BatchType::ChatCompletion if !item.url.contains("/chat/completions") => {
                return Err(GatewayError::BadRequest(
                    "URL must be /v1/chat/completions for chat completion batches".to_string(),
                ));
            }
            BatchType::Embedding if !item.url.contains("/embeddings") => {
                return Err(GatewayError::BadRequest(
                    "URL must be /v1/embeddings for embedding batches".to_string(),
                ));
            }
            _ => {}
        }

        Ok(())
    }
}
