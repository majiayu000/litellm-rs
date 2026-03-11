//! Vertex AI Image Generation Module
//!
//! Support for generating images using Vertex AI models

use crate::ProviderError;
use serde::{Deserialize, Serialize};

/// Image generation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationRequest {
    pub prompt: String,
    pub number_of_images: Option<i32>,
    pub aspect_ratio: Option<String>,
    pub negative_prompt: Option<String>,
    pub seed: Option<i64>,
    pub guidance_scale: Option<f32>,
    pub safety_settings: Option<Vec<SafetySetting>>,
}

/// Image generation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationResponse {
    pub predictions: Vec<ImagePrediction>,
    pub metadata: Option<serde_json::Value>,
}

/// Generated image prediction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagePrediction {
    pub bytes_base64_encoded: Option<String>,
    pub mime_type: String,
    pub safety_ratings: Option<Vec<SafetyRating>>,
}

/// Safety setting for image generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetySetting {
    pub category: String,
    pub threshold: String,
}

/// Safety rating for generated content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRating {
    pub category: String,
    pub probability: String,
    pub blocked: bool,
}

/// Image generation models
#[derive(Debug, Clone)]
pub enum ImageGenerationModel {
    /// Imagen 2
    Imagen2,
    /// Imagen 3
    Imagen3,
    /// Custom model
    Custom(String),
}

impl ImageGenerationModel {
    /// Get model ID for API calls
    pub fn model_id(&self) -> String {
        match self {
            Self::Imagen2 => "imagen-2".to_string(),
            Self::Imagen3 => "imagen-3".to_string(),
            Self::Custom(id) => id.clone(),
        }
    }

    /// Check if model supports aspect ratio
    pub fn supports_aspect_ratio(&self) -> bool {
        matches!(self, Self::Imagen2 | Self::Imagen3)
    }

    /// Get supported aspect ratios
    pub fn supported_aspect_ratios(&self) -> Vec<&str> {
        match self {
            Self::Imagen2 | Self::Imagen3 => vec!["1:1", "9:16", "16:9", "3:4", "4:3"],
            Self::Custom(_) => vec!["1:1"],
        }
    }
}

/// Image generation handler
pub struct ImageGenerationHandler;

impl ImageGenerationHandler {
    /// Create new image generation handler
    pub fn new(_project_id: String, _location: String) -> Self {
        Self
    }

    /// Generate images from text prompt
    pub async fn generate_image(
        &self,
        model: &ImageGenerationModel,
        request: ImageGenerationRequest,
    ) -> Result<ImageGenerationResponse, ProviderError> {
        self.validate_request(model, &request)?;

        // Transform request for Vertex AI format
        let _vertex_request = self.transform_request(model, request)?;

        // TODO: Implement actual API call
        Ok(ImageGenerationResponse {
            predictions: vec![ImagePrediction {
                bytes_base64_encoded: Some("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==".to_string()),
                mime_type: "image/png".to_string(),
                safety_ratings: None,
            }],
            metadata: None,
        })
    }

    /// Validate image generation request
    fn validate_request(
        &self,
        model: &ImageGenerationModel,
        request: &ImageGenerationRequest,
    ) -> Result<(), ProviderError> {
        // Validate prompt
        if request.prompt.trim().is_empty() {
            return Err(ProviderError::invalid_request("vertex_ai", "Empty prompt"));
        }

        // Validate aspect ratio
        if let Some(aspect_ratio) = &request.aspect_ratio
            && model.supports_aspect_ratio()
        {
            let supported = model.supported_aspect_ratios();
            if !supported.contains(&aspect_ratio.as_str()) {
                return Err(ProviderError::invalid_request(
                    "vertex_ai",
                    format!("Unsupported aspect ratio: {}", aspect_ratio),
                ));
            }
        }

        // Validate number of images
        if let Some(count) = request.number_of_images
            && !(1..=4).contains(&count)
        {
            return Err(ProviderError::invalid_request(
                "vertex_ai",
                "Number of images must be between 1 and 4",
            ));
        }

        Ok(())
    }

    /// Transform request to Vertex AI format
    fn transform_request(
        &self,
        _model: &ImageGenerationModel,
        request: ImageGenerationRequest,
    ) -> Result<serde_json::Value, ProviderError> {
        let mut instances = serde_json::json!([{
            "prompt": request.prompt
        }]);

        if let Some(negative_prompt) = request.negative_prompt {
            instances[0]["negativePrompt"] = serde_json::Value::String(negative_prompt);
        }

        let mut parameters = serde_json::json!({});

        if let Some(count) = request.number_of_images {
            parameters["sampleCount"] = serde_json::Value::Number(count.into());
        }

        if let Some(aspect_ratio) = request.aspect_ratio {
            parameters["aspectRatio"] = serde_json::Value::String(aspect_ratio);
        }

        if let Some(seed) = request.seed {
            parameters["seed"] = serde_json::Value::Number(seed.into());
        }

        if let Some(guidance_scale) = request.guidance_scale {
            parameters["guidanceScale"] = serde_json::json!(guidance_scale);
        }

        Ok(serde_json::json!({
            "instances": instances,
            "parameters": parameters
        }))
    }

    /// Calculate generation cost
    pub fn calculate_cost(&self, model: &ImageGenerationModel, count: i32) -> f64 {
        let base_cost = match model {
            ImageGenerationModel::Imagen2 => 0.024, // $0.024 per image
            ImageGenerationModel::Imagen3 => 0.04,  // $0.04 per image
            ImageGenerationModel::Custom(_) => 0.024,
        };

        base_cost * count as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_id() {
        assert_eq!(ImageGenerationModel::Imagen2.model_id(), "imagen-2");
        assert_eq!(ImageGenerationModel::Imagen3.model_id(), "imagen-3");
    }

    #[test]
    fn test_supported_aspect_ratios() {
        let model = ImageGenerationModel::Imagen2;
        let ratios = model.supported_aspect_ratios();
        assert!(ratios.contains(&"1:1"));
        assert!(ratios.contains(&"16:9"));
    }

    #[tokio::test]
    async fn test_validate_request() {
        let handler = ImageGenerationHandler::new("test".to_string(), "us-central1".to_string());
        let model = ImageGenerationModel::Imagen2;

        let valid_request = ImageGenerationRequest {
            prompt: "A beautiful sunset".to_string(),
            number_of_images: Some(2),
            aspect_ratio: Some("16:9".to_string()),
            negative_prompt: None,
            seed: None,
            guidance_scale: None,
            safety_settings: None,
        };

        assert!(handler.validate_request(&model, &valid_request).is_ok());

        let invalid_request = ImageGenerationRequest {
            prompt: "".to_string(),
            number_of_images: Some(10),
            aspect_ratio: Some("21:9".to_string()),
            negative_prompt: None,
            seed: None,
            guidance_scale: None,
            safety_settings: None,
        };

        assert!(handler.validate_request(&model, &invalid_request).is_err());
    }
}
