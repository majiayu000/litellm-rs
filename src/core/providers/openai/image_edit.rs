//! OpenAI Image Editing Module
//!
//! Image editing functionality following the unified architecture

use serde::{Deserialize, Serialize};

use crate::core::providers::unified_provider::ProviderError;

/// OpenAI Image Edit request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIImageEditRequest {
    /// The image to edit. Must be a valid PNG file, less than 4MB, and square
    pub image: String,

    /// An additional image whose fully transparent areas indicate where image should be edited
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mask: Option<String>,

    /// A text description of the desired image
    pub prompt: String,

    /// The model to use for image generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// The number of images to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,

    /// The size of the generated images
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<ImageSize>,

    /// The format in which the generated images are returned
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,

    /// A unique identifier representing your end-user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// Image size options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageSize {
    #[serde(rename = "256x256")]
    Size256x256,
    #[serde(rename = "512x512")]
    Size512x512,
    #[serde(rename = "1024x1024")]
    Size1024x1024,
}

/// Response format options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ResponseFormat {
    #[default]
    Url,
    B64Json,
}

/// OpenAI Image Edit Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIImageEditResponse {
    pub created: i64,
    pub data: Vec<ImageData>,
}

/// Image data in response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b64_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_prompt: Option<String>,
}

/// Image editing utilities
pub struct OpenAIImageEditUtils;

impl OpenAIImageEditUtils {
    /// Get supported image edit models
    pub fn get_supported_models() -> Vec<&'static str> {
        vec!["dall-e-2"]
    }

    /// Check if model supports image editing
    pub fn supports_image_editing(model_id: &str) -> bool {
        Self::get_supported_models().contains(&model_id)
    }

    /// Create image edit request
    pub fn create_edit_request(
        image: String,
        prompt: String,
        mask: Option<String>,
        size: Option<ImageSize>,
    ) -> OpenAIImageEditRequest {
        OpenAIImageEditRequest {
            image,
            mask,
            prompt,
            model: Some("dall-e-2".to_string()),
            n: Some(1),
            size,
            response_format: Some(ResponseFormat::Url),
            user: None,
        }
    }

    /// Validate image edit request
    pub fn validate_request(request: &OpenAIImageEditRequest) -> Result<(), ProviderError> {
        // Check model
        if let Some(model) = &request.model
            && !Self::supports_image_editing(model)
        {
            return Err(ProviderError::ModelNotFound {
                provider: "openai",
                model: model.clone(),
            });
        }

        // Check prompt
        if request.prompt.is_empty() {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "Prompt cannot be empty".to_string(),
            });
        }

        if request.prompt.len() > 1000 {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "Prompt must be 1000 characters or less".to_string(),
            });
        }

        // Check n parameter
        if let Some(n) = request.n
            && (n == 0 || n > 10)
        {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "n must be between 1 and 10".to_string(),
            });
        }

        // Check image format (basic validation - in real implementation would check file headers)
        if !request.image.starts_with("data:image/png;base64,")
            && !request.image.starts_with("http")
        {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "Image must be a PNG file or valid URL".to_string(),
            });
        }

        Ok(())
    }

    /// Estimate cost for image editing
    pub fn estimate_cost(n: u32, size: &ImageSize) -> Result<f64, ProviderError> {
        let base_cost = match size {
            ImageSize::Size256x256 => 0.016,
            ImageSize::Size512x512 => 0.018,
            ImageSize::Size1024x1024 => 0.020,
        };

        Ok(base_cost * n as f64)
    }

    /// Get max image size in bytes
    pub fn get_max_image_size() -> usize {
        4 * 1024 * 1024 // 4MB
    }

    /// Check if image size is supported
    pub fn is_supported_size(size: &ImageSize) -> bool {
        matches!(
            size,
            ImageSize::Size256x256 | ImageSize::Size512x512 | ImageSize::Size1024x1024
        )
    }
}

/// Default implementations
impl Default for ImageSize {
    fn default() -> Self {
        ImageSize::Size1024x1024
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supports_image_editing() {
        assert!(OpenAIImageEditUtils::supports_image_editing("dall-e-2"));
        assert!(!OpenAIImageEditUtils::supports_image_editing("dall-e-3"));
        assert!(!OpenAIImageEditUtils::supports_image_editing("gpt-4"));
    }

    #[test]
    fn test_create_edit_request() {
        let request = OpenAIImageEditUtils::create_edit_request(
            "data:image/png;base64,iVBORw0KGgo...".to_string(),
            "Add a red hat".to_string(),
            None,
            Some(ImageSize::Size512x512),
        );

        assert_eq!(request.prompt, "Add a red hat");
        assert_eq!(request.model, Some("dall-e-2".to_string()));
        assert_eq!(request.n, Some(1));
        assert!(matches!(request.size, Some(ImageSize::Size512x512)));
    }

    #[test]
    fn test_validate_request() {
        let valid_request = OpenAIImageEditUtils::create_edit_request(
            "data:image/png;base64,iVBORw0KGgo...".to_string(),
            "Add a blue background".to_string(),
            None,
            Some(ImageSize::Size256x256),
        );
        assert!(OpenAIImageEditUtils::validate_request(&valid_request).is_ok());

        // Test invalid model
        let mut invalid_model = valid_request.clone();
        invalid_model.model = Some("dall-e-3".to_string());
        assert!(OpenAIImageEditUtils::validate_request(&invalid_model).is_err());

        // Test empty prompt
        let mut empty_prompt = valid_request.clone();
        empty_prompt.prompt = "".to_string();
        assert!(OpenAIImageEditUtils::validate_request(&empty_prompt).is_err());

        // Test long prompt
        let mut long_prompt = valid_request.clone();
        long_prompt.prompt = "a".repeat(1001);
        assert!(OpenAIImageEditUtils::validate_request(&long_prompt).is_err());
    }

    #[test]
    fn test_estimate_cost() {
        let cost_256 = OpenAIImageEditUtils::estimate_cost(1, &ImageSize::Size256x256).unwrap();
        assert_eq!(cost_256, 0.016);

        let cost_512 = OpenAIImageEditUtils::estimate_cost(2, &ImageSize::Size512x512).unwrap();
        assert_eq!(cost_512, 0.036);

        let cost_1024 = OpenAIImageEditUtils::estimate_cost(3, &ImageSize::Size1024x1024).unwrap();
        assert_eq!(cost_1024, 0.060);
    }

    #[test]
    fn test_is_supported_size() {
        assert!(OpenAIImageEditUtils::is_supported_size(
            &ImageSize::Size256x256
        ));
        assert!(OpenAIImageEditUtils::is_supported_size(
            &ImageSize::Size512x512
        ));
        assert!(OpenAIImageEditUtils::is_supported_size(
            &ImageSize::Size1024x1024
        ));
    }

    #[test]
    fn test_get_max_image_size() {
        assert_eq!(OpenAIImageEditUtils::get_max_image_size(), 4 * 1024 * 1024);
    }
}
