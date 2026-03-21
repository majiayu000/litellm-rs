//! OpenAI Images Module
//!
//! Image generation, editing, and variations following the unified architecture

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::providers::unified_provider::ProviderError;

// ============================================================================
// Common Image Types
// ============================================================================

/// Image size options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImageSize {
    #[serde(rename = "256x256")]
    Size256x256,
    #[serde(rename = "512x512")]
    Size512x512,
    #[serde(rename = "1024x1024")]
    Size1024x1024,
    #[serde(rename = "1792x1024")]
    Size1792x1024,
    #[serde(rename = "1024x1792")]
    Size1024x1792,
}

/// Image response format
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageResponseFormat {
    Url,
    B64Json,
}

/// Image quality for DALL-E 3
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageQuality {
    Standard,
    HD,
}

/// Image style for DALL-E 3
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageStyle {
    Vivid,
    Natural,
}

/// Unified Image Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageResponse {
    pub created: i64,
    pub data: Vec<ImageData>,
}

/// Image data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b64_json: Option<String>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_prompt: Option<String>,
}

// ============================================================================
// Image Generation
// ============================================================================

/// OpenAI Image Generation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIImageGenerationRequest {
    /// A text description of the desired image(s)
    pub prompt: String,

    /// The model to use for image generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// The number of images to generate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u32>,

    /// The quality of the image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<ImageQuality>,

    /// The format in which the generated images are returned
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ImageResponseFormat>,

    /// The size of the generated images
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<ImageSize>,

    /// The style of the generated images
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<ImageStyle>,

    /// A unique identifier representing your end-user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
}

// ============================================================================
// Image Editing
// ============================================================================

/// OpenAI Image Edit request
#[derive(Debug, Clone)]
pub struct OpenAIImageEditRequest {
    /// The image to edit (must be PNG, less than 4MB, and square)
    pub image: Vec<u8>,

    /// A text description of the desired image(s)
    pub prompt: String,

    /// An additional image for masking (PNG, less than 4MB)
    pub mask: Option<Vec<u8>>,

    /// The model to use (only dall-e-2 supports editing)
    pub model: Option<String>,

    /// The number of images to generate
    pub n: Option<u32>,

    /// The size of the generated images
    pub size: Option<ImageSize>,

    /// The format in which the generated images are returned
    pub response_format: Option<ImageResponseFormat>,

    /// A unique identifier representing your end-user
    pub user: Option<String>,
}

// ============================================================================
// Image Variations
// ============================================================================

/// OpenAI Image Variations request
#[derive(Debug, Clone)]
pub struct OpenAIImageVariationsRequest {
    /// The image to create variations of
    pub image: Vec<u8>,

    /// The model to use (only dall-e-2 supports variations)
    pub model: Option<String>,

    /// The number of images to generate
    pub n: Option<u32>,

    /// The format in which the generated images are returned
    pub response_format: Option<ImageResponseFormat>,

    /// The size of the generated images
    pub size: Option<ImageSize>,

    /// A unique identifier representing your end-user
    pub user: Option<String>,
}

// ============================================================================
// Transformers and Utilities
// ============================================================================

/// Image processing utilities
pub struct OpenAIImageUtils;

impl OpenAIImageUtils {
    /// Create image generation request
    pub fn create_generation_request(
        prompt: impl Into<String>,
        model: Option<String>,
        n: Option<u32>,
        size: Option<ImageSize>,
    ) -> OpenAIImageGenerationRequest {
        OpenAIImageGenerationRequest {
            prompt: prompt.into(),
            model: model.or_else(|| Some("dall-e-3".to_string())),
            n: n.or(Some(1)),
            quality: Some(ImageQuality::Standard),
            response_format: Some(ImageResponseFormat::Url),
            size: size.or(Some(ImageSize::Size1024x1024)),
            style: Some(ImageStyle::Vivid),
            user: None,
        }
    }

    /// Create image editing request
    pub fn create_edit_request(
        image_data: Vec<u8>,
        prompt: impl Into<String>,
        mask_data: Option<Vec<u8>>,
        size: Option<ImageSize>,
        n: Option<u32>,
    ) -> OpenAIImageEditRequest {
        OpenAIImageEditRequest {
            image: image_data,
            prompt: prompt.into(),
            mask: mask_data,
            model: Some("dall-e-2".to_string()), // Only DALL-E 2 supports editing
            n: n.or(Some(1)),
            size: size.or(Some(ImageSize::Size1024x1024)),
            response_format: Some(ImageResponseFormat::Url),
            user: None,
        }
    }

    /// Create image variations request
    pub fn create_variations_request(
        image_data: Vec<u8>,
        n: Option<u32>,
        size: Option<ImageSize>,
        response_format: Option<ImageResponseFormat>,
    ) -> OpenAIImageVariationsRequest {
        OpenAIImageVariationsRequest {
            image: image_data,
            model: Some("dall-e-2".to_string()), // Only DALL-E 2 supports variations
            n: n.or(Some(1)),
            response_format: response_format.or(Some(ImageResponseFormat::Url)),
            size: size.or(Some(ImageSize::Size1024x1024)),
            user: None,
        }
    }

    /// Get supported models for image generation
    pub fn get_generation_models() -> Vec<&'static str> {
        vec!["dall-e-2", "dall-e-3", "gpt-image-1", "gpt-image-1.5"]
    }

    /// Get supported models for image editing
    pub fn get_editing_models() -> Vec<&'static str> {
        vec!["dall-e-2"]
    }

    /// Get supported models for image variations
    pub fn get_variation_models() -> Vec<&'static str> {
        vec!["dall-e-2"]
    }

    /// Check if model supports image generation
    pub fn supports_generation(model: &str) -> bool {
        Self::get_generation_models().contains(&model)
    }

    /// Check if model supports image editing
    pub fn supports_editing(model: &str) -> bool {
        Self::get_editing_models().contains(&model)
    }

    /// Check if model supports image variations
    pub fn supports_variations(model: &str) -> bool {
        Self::get_variation_models().contains(&model)
    }

    /// Parse image size from string
    pub fn parse_size(size_str: &str) -> Result<ImageSize, ProviderError> {
        match size_str {
            "256x256" => Ok(ImageSize::Size256x256),
            "512x512" => Ok(ImageSize::Size512x512),
            "1024x1024" => Ok(ImageSize::Size1024x1024),
            "1792x1024" => Ok(ImageSize::Size1792x1024),
            "1024x1792" => Ok(ImageSize::Size1024x1792),
            _ => Err(ProviderError::InvalidRequest {
                message: format!("Unsupported image size: {}", size_str),
                details: None,
            }),
        }
    }

    /// Parse image quality from string
    pub fn parse_quality(quality_str: &str) -> Result<ImageQuality, ProviderError> {
        match quality_str.to_lowercase().as_str() {
            "standard" => Ok(ImageQuality::Standard),
            "hd" => Ok(ImageQuality::HD),
            _ => Err(ProviderError::InvalidRequest {
                message: format!("Unsupported image quality: {}", quality_str),
                details: None,
            }),
        }
    }

    /// Parse image style from string
    pub fn parse_style(style_str: &str) -> Result<ImageStyle, ProviderError> {
        match style_str.to_lowercase().as_str() {
            "vivid" => Ok(ImageStyle::Vivid),
            "natural" => Ok(ImageStyle::Natural),
            _ => Err(ProviderError::InvalidRequest {
                message: format!("Unsupported image style: {}", style_str),
                details: None,
            }),
        }
    }

    /// Get cost estimation for image generation
    pub fn estimate_generation_cost(model: &str, quality: &ImageQuality, size: &ImageSize, n: u32) -> Result<f64, ProviderError> {
        let base_cost = match model {
            "dall-e-2" => match size {
                ImageSize::Size256x256 => 0.016,
                ImageSize::Size512x512 => 0.018,
                ImageSize::Size1024x1024 => 0.020,
                _ => return Err(ProviderError::InvalidRequest {
                    message: "DALL-E 2 only supports 256x256, 512x512, and 1024x1024".to_string(),
                    details: None,
                }),
            },
            "dall-e-3" => {
                let size_cost = match size {
                    ImageSize::Size1024x1024 => 0.040,
                    ImageSize::Size1024x1792 | ImageSize::Size1792x1024 => 0.080,
                    _ => return Err(ProviderError::InvalidRequest {
                        message: "DALL-E 3 only supports 1024x1024, 1024x1792, and 1792x1024".to_string(),
                        details: None,
                    }),
                };
                
                let quality_multiplier = match quality {
                    ImageQuality::Standard => 1.0,
                    ImageQuality::HD => 2.0,
                };
                
                size_cost * quality_multiplier
            },
            _ => return Err(ProviderError::InvalidRequest {
                message: format!("Unknown image generation model: {}", model),
                details: None,
            }),
        };

        Ok(base_cost * n as f64)
    }
}

// ============================================================================
// Validation Functions
// ============================================================================

/// Validate image generation request
pub fn validate_generation_request(request: &OpenAIImageGenerationRequest) -> Result<(), ProviderError> {
    // Check model support
    if let Some(model) = &request.model {
        if !OpenAIImageUtils::supports_generation(model) {
            return Err(ProviderError::ModelNotFound {
                model: model.clone(),
                available_models: OpenAIImageUtils::get_generation_models().iter().map(|s| s.to_string()).collect(),
                details: Some("Model does not support image generation".to_string()),
            });
        }
    }

    // Check prompt length
    if request.prompt.len() > 4000 {
        return Err(ProviderError::InvalidRequest {
            message: "Prompt must be less than 4000 characters".to_string(),
            details: None,
        });
    }

    if request.prompt.is_empty() {
        return Err(ProviderError::InvalidRequest {
            message: "Prompt cannot be empty".to_string(),
            details: None,
        });
    }

    // Check number of images
    if let Some(n) = request.n {
        let max_images = if request.model.as_deref() == Some("dall-e-3") { 1 } else { 10 };
        if n == 0 || n > max_images {
            return Err(ProviderError::InvalidRequest {
                message: format!("n must be between 1 and {}", max_images),
                details: None,
            });
        }
    }

    // Validate model-specific constraints
    if let Some(model) = &request.model {
        match model.as_str() {
            "dall-e-2" => {
                // DALL-E 2 constraints
                if let Some(size) = &request.size {
                    if !matches!(size, ImageSize::Size256x256 | ImageSize::Size512x512 | ImageSize::Size1024x1024) {
                        return Err(ProviderError::InvalidRequest {
                            message: "DALL-E 2 only supports 256x256, 512x512, and 1024x1024 sizes".to_string(),
                            details: None,
                        });
                    }
                }
                
                if request.quality.is_some() || request.style.is_some() {
                    return Err(ProviderError::InvalidRequest {
                        message: "DALL-E 2 does not support quality or style parameters".to_string(),
                        details: None,
                    });
                }
            },
            "dall-e-3" => {
                // DALL-E 3 constraints
                if let Some(size) = &request.size {
                    if !matches!(size, ImageSize::Size1024x1024 | ImageSize::Size1024x1792 | ImageSize::Size1792x1024) {
                        return Err(ProviderError::InvalidRequest {
                            message: "DALL-E 3 only supports 1024x1024, 1024x1792, and 1792x1024 sizes".to_string(),
                            details: None,
                        });
                    }
                }
            },
            _ => {}
        }
    }

    Ok(())
}

/// Validate image edit request
pub fn validate_edit_request(request: &OpenAIImageEditRequest) -> Result<(), ProviderError> {
    // Check image size (must be less than 4MB)
    if request.image.len() > 4 * 1024 * 1024 {
        return Err(ProviderError::InvalidRequest {
            message: "Image must be less than 4MB".to_string(),
            details: None,
        });
    }

    // Check mask size if provided
    if let Some(mask) = &request.mask {
        if mask.len() > 4 * 1024 * 1024 {
            return Err(ProviderError::InvalidRequest {
                message: "Mask must be less than 4MB".to_string(),
                details: None,
            });
        }
    }

    // Check prompt length
    if request.prompt.len() > 1000 {
        return Err(ProviderError::InvalidRequest {
            message: "Prompt must be less than 1000 characters".to_string(),
            details: None,
        });
    }

    // Check number of images
    if let Some(n) = request.n {
        if n == 0 || n > 10 {
            return Err(ProviderError::InvalidRequest {
                message: "n must be between 1 and 10".to_string(),
                details: None,
            });
        }
    }

    // Only DALL-E 2 supports editing
    if let Some(model) = &request.model {
        if !OpenAIImageUtils::supports_editing(model) {
            return Err(ProviderError::InvalidRequest {
                message: "Only dall-e-2 supports image editing".to_string(),
                details: None,
            });
        }
    }

    Ok(())
}

/// Validate image variations request
pub fn validate_variations_request(request: &OpenAIImageVariationsRequest) -> Result<(), ProviderError> {
    // Check image size (must be less than 4MB)
    if request.image.len() > 4 * 1024 * 1024 {
        return Err(ProviderError::InvalidRequest {
            message: "Image must be less than 4MB".to_string(),
            details: None,
        });
    }

    // Check number of images
    if let Some(n) = request.n {
        if n == 0 || n > 10 {
            return Err(ProviderError::InvalidRequest {
                message: "n must be between 1 and 10".to_string(),
                details: None,
            });
        }
    }

    // Only DALL-E 2 supports variations
    if let Some(model) = &request.model {
        if !OpenAIImageUtils::supports_variations(model) {
            return Err(ProviderError::InvalidRequest {
                message: "Only dall-e-2 supports image variations".to_string(),
                details: None,
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_utils() {
        assert!(OpenAIImageUtils::supports_generation("dall-e-2"));
        assert!(OpenAIImageUtils::supports_generation("dall-e-3"));
        assert!(!OpenAIImageUtils::supports_generation("gpt-4"));

        assert!(OpenAIImageUtils::supports_editing("dall-e-2"));
        assert!(!OpenAIImageUtils::supports_editing("dall-e-3"));

        assert!(OpenAIImageUtils::supports_variations("dall-e-2"));
        assert!(!OpenAIImageUtils::supports_variations("dall-e-3"));
    }

    #[test]
    fn test_parse_size() {
        assert!(matches!(OpenAIImageUtils::parse_size("1024x1024").unwrap(), ImageSize::Size1024x1024));
        assert!(matches!(OpenAIImageUtils::parse_size("512x512").unwrap(), ImageSize::Size512x512));
        assert!(OpenAIImageUtils::parse_size("invalid").is_err());
    }

    #[test]
    fn test_create_generation_request() {
        let request = OpenAIImageUtils::create_generation_request(
            "A beautiful landscape",
            Some("dall-e-3".to_string()),
            Some(1),
            Some(ImageSize::Size1024x1024),
        );

        assert_eq!(request.prompt, "A beautiful landscape");
        assert_eq!(request.model, Some("dall-e-3".to_string()));
        assert_eq!(request.n, Some(1));
    }

    #[test]
    fn test_validate_generation_request() {
        let valid_request = OpenAIImageUtils::create_generation_request(
            "Test prompt",
            Some("dall-e-3".to_string()),
            Some(1),
            Some(ImageSize::Size1024x1024),
        );
        assert!(validate_generation_request(&valid_request).is_ok());

        let mut invalid_prompt = valid_request.clone();
        invalid_prompt.prompt = "".to_string();
        assert!(validate_generation_request(&invalid_prompt).is_err());

        let mut invalid_model = valid_request.clone();
        invalid_model.model = Some("invalid-model".to_string());
        assert!(validate_generation_request(&invalid_model).is_err());
    }

    #[test]
    fn test_cost_estimation() {
        let cost = OpenAIImageUtils::estimate_generation_cost(
            "dall-e-2", 
            &ImageQuality::Standard, 
            &ImageSize::Size1024x1024, 
            1
        ).unwrap();
        assert_eq!(cost, 0.020);

        let cost = OpenAIImageUtils::estimate_generation_cost(
            "dall-e-3", 
            &ImageQuality::HD, 
            &ImageSize::Size1024x1024, 
            2
        ).unwrap();
        assert_eq!(cost, 0.160); // 0.040 * 2.0 (HD) * 2 (images)
    }
}