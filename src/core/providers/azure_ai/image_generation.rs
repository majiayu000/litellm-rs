//! Azure AI Image Generation Handler - Simplified Version
//!
//! Basic image generation functionality for Azure AI using FLUX models

use reqwest::Method;
use serde_json::{Value, json};

use super::client::AzureAIClient;
use super::config::{AzureAIConfig, AzureAIEndpointType};
use crate::core::providers::base::{HttpErrorMapper, read_streaming_error_body};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::{
    context::RequestContext,
    image::ImageGenerationRequest,
    responses::{ImageData, ImageGenerationResponse},
};

/// Azure AI image generation handler
#[derive(Debug, Clone)]
pub struct AzureAIImageHandler {
    client: AzureAIClient,
}

impl AzureAIImageHandler {
    /// Create a new image generation handler
    pub fn new(config: AzureAIConfig) -> Result<Self, ProviderError> {
        Self::from_client(AzureAIClient::new(config)?)
    }

    pub(crate) fn from_client(client: AzureAIClient) -> Result<Self, ProviderError> {
        Ok(Self { client })
    }

    /// Generate image
    pub async fn generate_image(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        // Validate request
        if request.prompt.is_empty() {
            return Err(ProviderError::invalid_request(
                "azure_ai",
                "Prompt cannot be empty",
            ));
        }

        // Build request
        let azure_request = json!({
            "model": request.model.clone().unwrap_or_else(|| "flux-1.1-pro".to_string()),
            "prompt": request.prompt,
            "n": request.n.unwrap_or(1),
            "size": request.size.clone().unwrap_or_else(|| "1024x1024".to_string()),
            "quality": request.quality.clone().unwrap_or_else(|| "standard".to_string())
        });

        // Build URL
        let url = self
            .client
            .get_config()
            .build_endpoint_url(AzureAIEndpointType::ImageGeneration.as_path())
            .map_err(|e| ProviderError::configuration("azure_ai", &e))?;

        // Execute request
        let response = self
            .client
            .request(Method::POST, &url)?
            .json(&azure_request)
            .send()
            .await
            .map_err(|e| ProviderError::network("azure_ai", format!("Request failed: {}", e)))?;

        // Handle error responses
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = read_streaming_error_body(response)
                .await
                .map_err(|err| err.into_provider_error("azure_ai"))?;
            return Err(HttpErrorMapper::map_status_code(
                "azure_ai",
                status,
                &error_body,
            ));
        }

        // Parse response
        let response_json: Value = response.json().await.map_err(|e| {
            ProviderError::serialization("azure_ai", format!("Failed to parse response: {}", e))
        })?;

        Self::transform_response(response_json)
    }

    fn transform_response(response: Value) -> Result<ImageGenerationResponse, ProviderError> {
        let created = response["created"]
            .as_u64()
            .unwrap_or_else(|| chrono::Utc::now().timestamp() as u64);

        let data = response["data"]
            .as_array()
            .ok_or_else(|| ProviderError::serialization("azure_ai", "Missing data array"))?
            .iter()
            .map(|item| ImageData {
                url: item["url"].as_str().map(|url| url.to_string()),
                b64_json: item["b64_json"].as_str().map(|b64| b64.to_string()),
                revised_prompt: item["revised_prompt"]
                    .as_str()
                    .map(|prompt| prompt.to_string()),
            })
            .collect();

        Ok(ImageGenerationResponse { created, data })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation() {
        let config = AzureAIConfig::new("azure_ai");
        let _result = AzureAIImageHandler::new(config);
    }

    #[test]
    fn test_transform_response_preserves_returned_image_url() {
        let response = json!({
            "created": 1700000000_u64,
            "data": [{
                "url": "https://cdn.example.com/generated.png",
                "revised_prompt": "a generated image"
            }]
        });

        let result = AzureAIImageHandler::transform_response(response).unwrap();
        assert_eq!(result.created, 1700000000);
        assert_eq!(
            result.data[0].url.as_deref(),
            Some("https://cdn.example.com/generated.png")
        );
        assert_ne!(
            result.data[0].url.as_deref(),
            Some("https://example.com/generated_image.jpg")
        );
    }
}
