//! Azure OpenAI Image Generation Handler
//!
//! Complete image generation functionality for Azure OpenAI Service (DALL-E)

use reqwest::header::HeaderMap;
use serde_json::{Value, json};

use crate::core::types::{
    context::RequestContext,
    image::ImageGenerationRequest,
    responses::{ImageData, ImageGenerationResponse},
};

use super::config::AzureConfig;
use super::error::{azure_api_error, azure_config_error, azure_header_error};
use super::utils::{AzureEndpointType, AzureUtils};
use crate::core::providers::unified_provider::ProviderError;
use crate::core::traits::provider::ProviderConfig;
use crate::utils::net::http::create_custom_client;

/// Azure OpenAI image generation handler
#[derive(Debug, Clone)]
pub struct AzureImageHandler {
    config: AzureConfig,
    client: reqwest::Client,
}

impl AzureImageHandler {
    /// Create new image generation handler
    pub fn new(config: AzureConfig) -> Result<Self, ProviderError> {
        let client = create_custom_client(ProviderConfig::timeout(&config))
            .map_err(|e| azure_config_error(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { config, client })
    }

    /// Build request headers
    async fn build_headers(&self) -> Result<HeaderMap, ProviderError> {
        let mut headers = HeaderMap::new();

        // Add API key
        if let Some(api_key) = self.config.get_effective_api_key().await {
            headers.insert(
                "api-key",
                api_key
                    .parse()
                    .map_err(|e| azure_header_error(format!("Invalid API key: {}", e)))?,
            );
        } else {
            return Err(ProviderError::authentication(
                "azure",
                "No API key available",
            ));
        }

        headers.insert(
            "Content-Type",
            "application/json"
                .parse()
                .map_err(|e| azure_header_error(format!("Invalid content type: {}", e)))?,
        );

        // Add custom headers
        for (key, value) in &self.config.custom_headers {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| azure_header_error(format!("Invalid header name: {}", e)))?;
            let header_value = value
                .parse()
                .map_err(|e| azure_header_error(format!("Invalid header value: {}", e)))?;
            headers.insert(header_name, header_value);
        }

        Ok(headers)
    }

    /// Generate image
    pub async fn generate_image(
        &self,
        request: ImageGenerationRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        // Validate request
        AzureImageUtils::validate_request(&request)?;

        // Get deployment name (for DALL-E models)
        let model_name = request.model.as_deref().unwrap_or("dall-e-3");
        let deployment = self.config.get_effective_deployment_name(model_name);

        // Get Azure endpoint
        let azure_endpoint = self
            .config
            .get_effective_azure_endpoint()
            .ok_or_else(|| azure_config_error("Azure endpoint not configured"))?;

        // Build URL
        let url = AzureUtils::build_azure_url(
            &azure_endpoint,
            &deployment,
            &self.config.api_version,
            AzureEndpointType::Images,
        );

        // Transform request
        let azure_request = AzureImageUtils::transform_request(&request)?;

        // Build headers
        let headers = self.build_headers().await?;

        // Execute request
        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&azure_request)
            .send()
            .await?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(azure_api_error(status, error_body));
        }

        // Parse response
        let response_json: Value = response.json().await?;

        // Transform response
        AzureImageUtils::transform_response(response_json)
    }

    /// Edit image (for DALL-E 2)
    pub async fn edit_image(
        &self,
        request: ImageEditRequest,
        _context: RequestContext,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        // Get deployment name
        let model_name = request.model.as_str();
        let deployment = self.config.get_effective_deployment_name(model_name);

        // Get Azure endpoint
        let azure_endpoint = self
            .config
            .get_effective_azure_endpoint()
            .ok_or_else(|| azure_config_error("Azure endpoint not configured"))?;

        // Build URL
        let url = AzureUtils::build_azure_url(
            &azure_endpoint,
            &deployment,
            &self.config.api_version,
            AzureEndpointType::ImageEdits,
        );

        // Build multipart form for image edit
        let mut form = reqwest::multipart::Form::new()
            .text("prompt", request.prompt)
            .part("image", request.image)
            .text("n", request.n.unwrap_or(1).to_string());

        if let Some(size) = request.size {
            form = form.text("size", size);
        }

        if let Some(mask) = request.mask {
            form = form.part("mask", mask);
        }

        // Build headers
        let mut headers = self.build_headers().await?;
        headers.remove("Content-Type"); // Let reqwest set multipart content type

        // Execute request
        let response = self
            .client
            .post(&url)
            .headers(headers)
            .multipart(form)
            .send()
            .await?;

        // Check status
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(azure_api_error(status, error_body));
        }

        // Parse response
        let response_json: Value = response.json().await?;

        // Transform response
        AzureImageUtils::transform_response(response_json)
    }
}

/// Image edit request
#[derive(Debug)]
pub struct ImageEditRequest {
    pub model: String,
    pub image: reqwest::multipart::Part,
    pub mask: Option<reqwest::multipart::Part>,
    pub prompt: String,
    pub n: Option<usize>,
    pub size: Option<String>,
}

/// Azure image generation utilities
pub struct AzureImageUtils;

impl AzureImageUtils {
    /// Validate image generation request
    pub fn validate_request(request: &ImageGenerationRequest) -> Result<(), ProviderError> {
        if request.prompt.is_empty() {
            return Err(azure_config_error("Prompt cannot be empty"));
        }

        // Validate size if specified
        if let Some(size) = &request.size {
            let model_name = request.model.as_deref().unwrap_or("dall-e-3");
            if !Self::is_valid_size(size, model_name) {
                return Err(azure_config_error(format!(
                    "Invalid size '{}' for model '{}'",
                    size, model_name
                )));
            }
        }

        // Validate quality
        if let Some(quality) = &request.quality
            && !["standard", "hd"].contains(&quality.as_str())
        {
            return Err(azure_config_error(format!(
                "Invalid quality '{}'. Must be 'standard' or 'hd'",
                quality
            )));
        }

        // Validate style
        if let Some(style) = &request.style
            && !["vivid", "natural"].contains(&style.as_str())
        {
            return Err(azure_config_error(format!(
                "Invalid style '{}'. Must be 'vivid' or 'natural'",
                style
            )));
        }

        // Validate n (number of images)
        if let Some(n) = request.n
            && (n == 0 || n > 10)
        {
            return Err(azure_config_error(
                "Number of images must be between 1 and 10",
            ));
        }

        Ok(())
    }

    /// Transform request to Azure format
    pub fn transform_request(request: &ImageGenerationRequest) -> Result<Value, ProviderError> {
        let mut body = json!({
            "prompt": request.prompt,
        });

        // Add optional parameters
        if let Some(n) = request.n {
            body["n"] = json!(n);
        }

        if let Some(size) = &request.size {
            body["size"] = json!(size);
        }

        if let Some(quality) = &request.quality {
            body["quality"] = json!(quality);
        }

        if let Some(style) = &request.style {
            body["style"] = json!(style);
        }

        if let Some(response_format) = &request.response_format {
            body["response_format"] = json!(response_format);
        }

        if let Some(user) = &request.user {
            body["user"] = json!(user);
        }

        Ok(body)
    }

    /// Transform Azure response to standard format
    pub fn transform_response(response: Value) -> Result<ImageGenerationResponse, ProviderError> {
        let created = response["created"].as_u64().unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        });

        let data = response["data"]
            .as_array()
            .ok_or_else(|| ProviderError::serialization("azure", "Missing data array"))?
            .iter()
            .map(|item| {
                // Handle both URL and base64 responses
                if let Some(url) = item["url"].as_str() {
                    ImageData {
                        url: Some(url.to_string()),
                        b64_json: None,
                        revised_prompt: item["revised_prompt"].as_str().map(|s| s.to_string()),
                    }
                } else if let Some(b64) = item["b64_json"].as_str() {
                    ImageData {
                        url: None,
                        b64_json: Some(b64.to_string()),
                        revised_prompt: item["revised_prompt"].as_str().map(|s| s.to_string()),
                    }
                } else {
                    ImageData {
                        url: None,
                        b64_json: None,
                        revised_prompt: None,
                    }
                }
            })
            .collect();

        Ok(ImageGenerationResponse { created, data })
    }

    /// Check if size is valid for model
    fn is_valid_size(size: &str, model: &str) -> bool {
        let lower_model = model.to_lowercase();

        if lower_model.contains("dall-e-3") {
            // DALL-E 3 supported sizes
            matches!(size, "1024x1024" | "1024x1792" | "1792x1024")
        } else if lower_model.contains("dall-e-2") {
            // DALL-E 2 supported sizes
            matches!(size, "256x256" | "512x512" | "1024x1024")
        } else {
            // Default to true for unknown models
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_request() -> ImageGenerationRequest {
        ImageGenerationRequest {
            prompt: "A beautiful sunset over the ocean".to_string(),
            model: Some("dall-e-3".to_string()),
            n: Some(1),
            size: Some("1024x1024".to_string()),
            quality: Some("standard".to_string()),
            style: Some("vivid".to_string()),
            response_format: None,
            user: None,
        }
    }

    #[test]
    fn test_validate_request_valid() {
        let request = create_test_request();
        assert!(AzureImageUtils::validate_request(&request).is_ok());
    }

    #[test]
    fn test_validate_request_empty_prompt() {
        let request = ImageGenerationRequest {
            prompt: "".to_string(),
            model: Some("dall-e-3".to_string()),
            n: None,
            size: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };
        let result = AzureImageUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_invalid_size_dalle3() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-3".to_string()),
            size: Some("256x256".to_string()), // Not valid for DALL-E 3
            n: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };
        let result = AzureImageUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_invalid_size_dalle2() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-2".to_string()),
            size: Some("1792x1024".to_string()), // Not valid for DALL-E 2
            n: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };
        let result = AzureImageUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_invalid_quality() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-3".to_string()),
            quality: Some("ultra".to_string()), // Invalid
            n: None,
            size: None,
            response_format: None,
            style: None,
            user: None,
        };
        let result = AzureImageUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_invalid_style() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-3".to_string()),
            style: Some("artistic".to_string()), // Invalid
            n: None,
            size: None,
            quality: None,
            response_format: None,
            user: None,
        };
        let result = AzureImageUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_n_zero() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-3".to_string()),
            n: Some(0),
            size: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };
        let result = AzureImageUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_request_n_too_large() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-3".to_string()),
            n: Some(11),
            size: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };
        let result = AzureImageUtils::validate_request(&request);
        assert!(result.is_err());
    }

    #[test]
    fn test_is_valid_size_dalle3() {
        assert!(AzureImageUtils::is_valid_size("1024x1024", "dall-e-3"));
        assert!(AzureImageUtils::is_valid_size("1024x1792", "dall-e-3"));
        assert!(AzureImageUtils::is_valid_size("1792x1024", "dall-e-3"));
        assert!(!AzureImageUtils::is_valid_size("256x256", "dall-e-3"));
        assert!(!AzureImageUtils::is_valid_size("512x512", "dall-e-3"));
    }

    #[test]
    fn test_is_valid_size_dalle2() {
        assert!(AzureImageUtils::is_valid_size("256x256", "dall-e-2"));
        assert!(AzureImageUtils::is_valid_size("512x512", "dall-e-2"));
        assert!(AzureImageUtils::is_valid_size("1024x1024", "dall-e-2"));
        assert!(!AzureImageUtils::is_valid_size("1024x1792", "dall-e-2"));
        assert!(!AzureImageUtils::is_valid_size("1792x1024", "dall-e-2"));
    }

    #[test]
    fn test_is_valid_size_unknown_model() {
        // Unknown models should accept any size
        assert!(AzureImageUtils::is_valid_size("1024x1024", "unknown-model"));
        assert!(AzureImageUtils::is_valid_size("4096x4096", "future-dalle"));
    }

    #[test]
    fn test_transform_request_basic() {
        let request = ImageGenerationRequest {
            prompt: "A cat".to_string(),
            model: Some("dall-e-3".to_string()),
            n: None,
            size: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };

        let result = AzureImageUtils::transform_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["prompt"], "A cat");
    }

    #[test]
    fn test_transform_request_with_options() {
        let request = create_test_request();

        let result = AzureImageUtils::transform_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["prompt"], "A beautiful sunset over the ocean");
        assert_eq!(value["n"], 1);
        assert_eq!(value["size"], "1024x1024");
        assert_eq!(value["quality"], "standard");
        assert_eq!(value["style"], "vivid");
    }

    #[test]
    fn test_transform_request_with_user() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-3".to_string()),
            user: Some("user-123".to_string()),
            n: None,
            size: None,
            quality: None,
            response_format: None,
            style: None,
        };

        let result = AzureImageUtils::transform_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["user"], "user-123");
    }

    #[test]
    fn test_transform_response_url() {
        let response = json!({
            "created": 1234567890,
            "data": [{
                "url": "https://example.com/image.png",
                "revised_prompt": "A revised prompt"
            }]
        });

        let result = AzureImageUtils::transform_response(response);
        assert!(result.is_ok());
        let image_response = result.unwrap();
        assert_eq!(image_response.created, 1234567890);
        assert_eq!(image_response.data.len(), 1);
        assert_eq!(
            image_response.data[0].url,
            Some("https://example.com/image.png".to_string())
        );
        assert_eq!(
            image_response.data[0].revised_prompt,
            Some("A revised prompt".to_string())
        );
    }

    #[test]
    fn test_transform_response_b64() {
        let response = json!({
            "created": 1234567890,
            "data": [{
                "b64_json": "base64encodeddata",
                "revised_prompt": "Another prompt"
            }]
        });

        let result = AzureImageUtils::transform_response(response);
        assert!(result.is_ok());
        let image_response = result.unwrap();
        assert_eq!(image_response.data.len(), 1);
        assert_eq!(
            image_response.data[0].b64_json,
            Some("base64encodeddata".to_string())
        );
        assert!(image_response.data[0].url.is_none());
    }

    #[test]
    fn test_transform_response_multiple_images() {
        let response = json!({
            "created": 1234567890,
            "data": [
                {"url": "https://example.com/image1.png"},
                {"url": "https://example.com/image2.png"},
                {"url": "https://example.com/image3.png"}
            ]
        });

        let result = AzureImageUtils::transform_response(response);
        assert!(result.is_ok());
        let image_response = result.unwrap();
        assert_eq!(image_response.data.len(), 3);
    }

    #[test]
    fn test_transform_response_missing_data() {
        let response = json!({
            "created": 1234567890
        });

        let result = AzureImageUtils::transform_response(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_azure_image_handler_new() {
        let config =
            AzureConfig::new().with_azure_endpoint("https://test.openai.azure.com".to_string());
        let handler = AzureImageHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_validate_request_valid_quality_hd() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-3".to_string()),
            quality: Some("hd".to_string()),
            n: None,
            size: None,
            response_format: None,
            style: None,
            user: None,
        };
        assert!(AzureImageUtils::validate_request(&request).is_ok());
    }

    #[test]
    fn test_validate_request_valid_style_natural() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-3".to_string()),
            style: Some("natural".to_string()),
            n: None,
            size: None,
            quality: None,
            response_format: None,
            user: None,
        };
        assert!(AzureImageUtils::validate_request(&request).is_ok());
    }

    #[test]
    fn test_validate_request_n_boundary() {
        // n=1 should be valid
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-3".to_string()),
            n: Some(1),
            size: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };
        assert!(AzureImageUtils::validate_request(&request).is_ok());

        // n=10 should be valid
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-3".to_string()),
            n: Some(10),
            size: None,
            quality: None,
            response_format: None,
            style: None,
            user: None,
        };
        assert!(AzureImageUtils::validate_request(&request).is_ok());
    }

    #[test]
    fn test_transform_request_response_format() {
        let request = ImageGenerationRequest {
            prompt: "Test".to_string(),
            model: Some("dall-e-3".to_string()),
            response_format: Some("b64_json".to_string()),
            n: None,
            size: None,
            quality: None,
            style: None,
            user: None,
        };

        let result = AzureImageUtils::transform_request(&request);
        assert!(result.is_ok());
        let value = result.unwrap();
        assert_eq!(value["response_format"], "b64_json");
    }
}
