//! OpenAI Image Variations Module
//!
//! Image variations functionality following the unified architecture

use serde::{Deserialize, Serialize};

use super::image_edit::{ImageData, ImageSize, ResponseFormat};
use crate::core::providers::unified_provider::ProviderError;

/// OpenAI Image Variations request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIImageVariationsRequest {
    /// The image to use as the basis for the variation(s)
    pub image: String,

    /// The model to use for image generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// The number of images to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,

    /// The format in which the generated images are returned
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,

    /// The size of the generated images
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<ImageSize>,

    /// A unique identifier representing your end-user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

/// OpenAI Image Variations Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIImageVariationsResponse {
    pub created: i64,
    pub data: Vec<ImageData>,
}

/// Image variations utilities
pub struct OpenAIImageVariationsUtils;

impl OpenAIImageVariationsUtils {
    /// Get supported image variations models
    pub fn get_supported_models() -> Vec<&'static str> {
        vec!["dall-e-2"]
    }

    /// Check if model supports image variations
    pub fn supports_image_variations(model_id: &str) -> bool {
        Self::get_supported_models().contains(&model_id)
    }

    /// Create image variations request
    pub fn create_variations_request(
        image: String,
        n: Option<u32>,
        size: Option<ImageSize>,
    ) -> OpenAIImageVariationsRequest {
        OpenAIImageVariationsRequest {
            image,
            model: Some("dall-e-2".to_string()),
            n: n.or(Some(1)),
            response_format: Some(ResponseFormat::Url),
            size,
            user: None,
        }
    }

    /// Validate image variations request
    pub fn validate_request(request: &OpenAIImageVariationsRequest) -> Result<(), ProviderError> {
        // Check model
        if let Some(model) = &request.model
            && !Self::supports_image_variations(model)
        {
            return Err(ProviderError::ModelNotFound {
                provider: "openai",
                model: model.clone(),
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

        // Check image format (basic validation)
        if !request.image.starts_with("data:image/png;base64,")
            && !request.image.starts_with("http")
        {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "Image must be a PNG file or valid URL".to_string(),
            });
        }

        // Check size if provided
        if let Some(size) = &request.size
            && !Self::is_supported_size(size)
        {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "Unsupported image size. Supported sizes: 256x256, 512x512, 1024x1024"
                    .to_string(),
            });
        }

        Ok(())
    }

    /// Estimate cost for image variations
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

    /// Get recommended parameters for different use cases
    pub fn get_recommended_params_for_use_case(use_case: &str) -> OpenAIImageVariationsRequest {
        match use_case.to_lowercase().as_str() {
            "avatar" => OpenAIImageVariationsRequest {
                image: String::new(), // Will be filled by caller
                model: Some("dall-e-2".to_string()),
                n: Some(4),
                response_format: Some(ResponseFormat::Url),
                size: Some(ImageSize::Size512x512),
                user: None,
            },
            "thumbnail" => OpenAIImageVariationsRequest {
                image: String::new(),
                model: Some("dall-e-2".to_string()),
                n: Some(3),
                response_format: Some(ResponseFormat::Url),
                size: Some(ImageSize::Size256x256),
                user: None,
            },
            "wallpaper" => OpenAIImageVariationsRequest {
                image: String::new(),
                model: Some("dall-e-2".to_string()),
                n: Some(2),
                response_format: Some(ResponseFormat::Url),
                size: Some(ImageSize::Size1024x1024),
                user: None,
            },
            _ => Self::create_variations_request(
                String::new(),
                Some(1),
                Some(ImageSize::Size512x512),
            ),
        }
    }

    /// Check if image meets requirements
    pub fn validate_image_requirements(image_data: &[u8]) -> Result<(), ProviderError> {
        // Check file size
        if image_data.len() > Self::get_max_image_size() {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "Image file size exceeds 4MB limit".to_string(),
            });
        }

        // Check if it's a PNG file (simple check for PNG header)
        if image_data.len() < 8 || &image_data[0..8] != b"\x89PNG\r\n\x1a\n" {
            return Err(ProviderError::InvalidRequest {
                provider: "openai",
                message: "Image must be in PNG format".to_string(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supports_image_variations() {
        assert!(OpenAIImageVariationsUtils::supports_image_variations(
            "dall-e-2"
        ));
        assert!(!OpenAIImageVariationsUtils::supports_image_variations(
            "dall-e-3"
        ));
        assert!(!OpenAIImageVariationsUtils::supports_image_variations(
            "gpt-4"
        ));
    }

    #[test]
    fn test_create_variations_request() {
        let request = OpenAIImageVariationsUtils::create_variations_request(
            "data:image/png;base64,iVBORw0KGgo...".to_string(),
            Some(3),
            Some(ImageSize::Size512x512),
        );

        assert_eq!(request.image, "data:image/png;base64,iVBORw0KGgo...");
        assert_eq!(request.model, Some("dall-e-2".to_string()));
        assert_eq!(request.n, Some(3));
        assert!(matches!(request.size, Some(ImageSize::Size512x512)));
    }

    #[test]
    fn test_validate_request() {
        let valid_request = OpenAIImageVariationsUtils::create_variations_request(
            "data:image/png;base64,iVBORw0KGgo...".to_string(),
            Some(2),
            Some(ImageSize::Size256x256),
        );
        assert!(OpenAIImageVariationsUtils::validate_request(&valid_request).is_ok());

        // Test invalid model
        let mut invalid_model = valid_request.clone();
        invalid_model.model = Some("dall-e-3".to_string());
        assert!(OpenAIImageVariationsUtils::validate_request(&invalid_model).is_err());

        // Test invalid n
        let mut invalid_n = valid_request.clone();
        invalid_n.n = Some(0);
        assert!(OpenAIImageVariationsUtils::validate_request(&invalid_n).is_err());

        let mut invalid_n_high = valid_request.clone();
        invalid_n_high.n = Some(15);
        assert!(OpenAIImageVariationsUtils::validate_request(&invalid_n_high).is_err());
    }

    #[test]
    fn test_estimate_cost() {
        let cost_256 =
            OpenAIImageVariationsUtils::estimate_cost(1, &ImageSize::Size256x256).unwrap();
        assert_eq!(cost_256, 0.016);

        let cost_512 =
            OpenAIImageVariationsUtils::estimate_cost(2, &ImageSize::Size512x512).unwrap();
        assert_eq!(cost_512, 0.036);

        let cost_1024 =
            OpenAIImageVariationsUtils::estimate_cost(3, &ImageSize::Size1024x1024).unwrap();
        assert_eq!(cost_1024, 0.060);
    }

    #[test]
    fn test_get_recommended_params_for_use_case() {
        let avatar_params =
            OpenAIImageVariationsUtils::get_recommended_params_for_use_case("avatar");
        assert_eq!(avatar_params.n, Some(4));
        assert!(matches!(avatar_params.size, Some(ImageSize::Size512x512)));

        let thumbnail_params =
            OpenAIImageVariationsUtils::get_recommended_params_for_use_case("thumbnail");
        assert_eq!(thumbnail_params.n, Some(3));
        assert!(matches!(
            thumbnail_params.size,
            Some(ImageSize::Size256x256)
        ));

        let wallpaper_params =
            OpenAIImageVariationsUtils::get_recommended_params_for_use_case("wallpaper");
        assert_eq!(wallpaper_params.n, Some(2));
        assert!(matches!(
            wallpaper_params.size,
            Some(ImageSize::Size1024x1024)
        ));
    }

    #[test]
    fn test_validate_image_requirements() {
        // Test valid PNG header
        let valid_png = b"\x89PNG\r\n\x1a\n".to_vec();
        assert!(OpenAIImageVariationsUtils::validate_image_requirements(&valid_png).is_ok());

        // Test invalid format
        let invalid_format = b"not a png file".to_vec();
        assert!(OpenAIImageVariationsUtils::validate_image_requirements(&invalid_format).is_err());

        // Test empty data
        let empty_data = b"".to_vec();
        assert!(OpenAIImageVariationsUtils::validate_image_requirements(&empty_data).is_err());
    }
}
