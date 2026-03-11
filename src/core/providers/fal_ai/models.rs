//! Fal AI Model Registry
//!
//! Model definitions and registry for Fal AI image generation models

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Image size representation for Fal AI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ImageSize {
    /// Preset size name (e.g., "square_hd", "landscape_4_3")
    Preset(String),
    /// Custom dimensions
    Custom { width: u32, height: u32 },
}

impl ImageSize {
    /// Convert OpenAI size format (e.g., "1024x1024") to Fal AI format
    pub fn from_openai_size(size: &str) -> Self {
        // Map standard OpenAI sizes to Fal AI presets
        match size {
            "1024x1024" => ImageSize::Preset("square_hd".to_string()),
            "512x512" => ImageSize::Preset("square".to_string()),
            "1792x1024" => ImageSize::Preset("landscape_16_9".to_string()),
            "1024x1792" => ImageSize::Preset("portrait_16_9".to_string()),
            "1024x768" => ImageSize::Preset("landscape_4_3".to_string()),
            "768x1024" => ImageSize::Preset("portrait_4_3".to_string()),
            _ => {
                // Try to parse custom dimensions
                if let Some((w, h)) = size.split_once('x')
                    && let (Ok(width), Ok(height)) = (w.parse(), h.parse())
                {
                    return ImageSize::Custom { width, height };
                }
                // Default fallback
                ImageSize::Preset("landscape_4_3".to_string())
            }
        }
    }
}

/// Fal AI model definition
#[derive(Debug, Clone)]
pub struct FalAIModel {
    /// Model ID/endpoint (e.g., "fal-ai/flux/schnell")
    pub id: String,
    /// Display name
    pub name: String,
    /// Model description
    pub description: String,
    /// Cost per image in USD
    pub cost_per_image: f64,
    /// Supported image sizes
    pub supported_sizes: Vec<String>,
    /// Maximum number of images per request
    pub max_images: u32,
    /// Whether the model supports prompt enhancement
    pub supports_prompt_enhancement: bool,
}

impl FalAIModel {
    /// Create a new Fal AI model definition
    pub fn new(id: &str, name: &str, description: &str, cost_per_image: f64) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: description.to_string(),
            cost_per_image,
            supported_sizes: vec![
                "square".to_string(),
                "square_hd".to_string(),
                "portrait_4_3".to_string(),
                "portrait_16_9".to_string(),
                "landscape_4_3".to_string(),
                "landscape_16_9".to_string(),
            ],
            max_images: 4,
            supports_prompt_enhancement: false,
        }
    }

    /// Set maximum images per request
    pub fn with_max_images(mut self, max: u32) -> Self {
        self.max_images = max;
        self
    }

    /// Set supported sizes
    pub fn with_sizes(mut self, sizes: Vec<String>) -> Self {
        self.supported_sizes = sizes;
        self
    }

    /// Enable prompt enhancement support
    pub fn with_prompt_enhancement(mut self) -> Self {
        self.supports_prompt_enhancement = true;
        self
    }
}

/// Fal AI model registry
#[derive(Debug, Clone)]
pub struct FalAIModelRegistry {
    models: HashMap<String, FalAIModel>,
}

impl Default for FalAIModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FalAIModelRegistry {
    /// Create a new model registry with default models
    pub fn new() -> Self {
        let mut models = HashMap::new();

        // Flux models
        models.insert(
            "fal-ai/flux/schnell".to_string(),
            FalAIModel::new(
                "fal-ai/flux/schnell",
                "Flux Schnell",
                "Fast high-quality image generation",
                0.003,
            ),
        );

        models.insert(
            "fal-ai/flux-pro/v1.1".to_string(),
            FalAIModel::new(
                "fal-ai/flux-pro/v1.1",
                "Flux Pro v1.1",
                "Professional quality image generation",
                0.05,
            )
            .with_prompt_enhancement(),
        );

        models.insert(
            "fal-ai/flux-pro/v1.1-ultra".to_string(),
            FalAIModel::new(
                "fal-ai/flux-pro/v1.1-ultra",
                "Flux Pro v1.1 Ultra",
                "Ultra high-quality image generation",
                0.06,
            )
            .with_prompt_enhancement(),
        );

        // Stable Diffusion models
        models.insert(
            "fal-ai/stable-diffusion-v3-medium".to_string(),
            FalAIModel::new(
                "fal-ai/stable-diffusion-v3-medium",
                "Stable Diffusion 3 Medium",
                "Stable Diffusion 3 medium quality",
                0.035,
            ),
        );

        // Recraft model
        models.insert(
            "fal-ai/recraft/v3/text-to-image".to_string(),
            FalAIModel::new(
                "fal-ai/recraft/v3/text-to-image",
                "Recraft V3",
                "High-quality artistic image generation",
                0.04,
            ),
        );

        // Imagen 4
        models.insert(
            "fal-ai/imagen4/preview".to_string(),
            FalAIModel::new(
                "fal-ai/imagen4/preview",
                "Imagen 4 Preview",
                "Google Imagen 4 preview model",
                0.04,
            ),
        );

        // Ideogram
        models.insert(
            "fal-ai/ideogram/v3".to_string(),
            FalAIModel::new(
                "fal-ai/ideogram/v3",
                "Ideogram V3",
                "Ideogram text-to-image model",
                0.08,
            ),
        );

        // BRIA models
        models.insert(
            "fal-ai/bria/text-to-image/hd".to_string(),
            FalAIModel::new(
                "fal-ai/bria/text-to-image/hd",
                "BRIA HD",
                "BRIA high-definition image generation",
                0.02,
            ),
        );

        Self { models }
    }

    /// Get model by ID
    pub fn get(&self, model_id: &str) -> Option<&FalAIModel> {
        self.models.get(model_id)
    }

    /// Check if model is supported
    pub fn is_supported(&self, model_id: &str) -> bool {
        self.models.contains_key(model_id)
    }

    /// List all supported models
    pub fn list_models(&self) -> Vec<&FalAIModel> {
        self.models.values().collect()
    }

    /// Get cost per image for a model
    pub fn get_cost_per_image(&self, model_id: &str) -> f64 {
        self.models
            .get(model_id)
            .map(|m| m.cost_per_image)
            .unwrap_or(0.0)
    }

    /// Register a custom model
    pub fn register(&mut self, model: FalAIModel) {
        self.models.insert(model.id.clone(), model);
    }
}

/// Supported OpenAI parameters for image generation
pub const SUPPORTED_OPENAI_PARAMS: &[&str] = &["n", "response_format", "size"];

/// Map OpenAI parameters to Fal AI parameters
pub fn map_openai_to_fal_params(params: &serde_json::Value) -> serde_json::Value {
    let mut fal_params = serde_json::Map::new();

    if let Some(obj) = params.as_object() {
        for (key, value) in obj {
            match key.as_str() {
                "n" => {
                    fal_params.insert("num_images".to_string(), value.clone());
                }
                "response_format" => {
                    // Fal AI uses output_format
                    let format = match value.as_str() {
                        Some("b64_json") | Some("url") => "jpeg",
                        Some(f) => f,
                        None => "jpeg",
                    };
                    fal_params.insert("output_format".to_string(), serde_json::json!(format));
                }
                "size" => {
                    if let Some(size_str) = value.as_str() {
                        let image_size = ImageSize::from_openai_size(size_str);
                        fal_params.insert(
                            "image_size".to_string(),
                            serde_json::to_value(image_size).unwrap_or_default(),
                        );
                    }
                }
                _ => {
                    // Pass through other parameters
                    fal_params.insert(key.clone(), value.clone());
                }
            }
        }
    }

    serde_json::Value::Object(fal_params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_size_from_openai_standard() {
        let size = ImageSize::from_openai_size("1024x1024");
        match size {
            ImageSize::Preset(s) => assert_eq!(s, "square_hd"),
            _ => panic!("Expected preset size"),
        }
    }

    #[test]
    fn test_image_size_from_openai_custom() {
        let size = ImageSize::from_openai_size("800x600");
        match size {
            ImageSize::Custom { width, height } => {
                assert_eq!(width, 800);
                assert_eq!(height, 600);
            }
            _ => panic!("Expected custom size"),
        }
    }

    #[test]
    fn test_image_size_from_openai_invalid() {
        let size = ImageSize::from_openai_size("invalid");
        match size {
            ImageSize::Preset(s) => assert_eq!(s, "landscape_4_3"),
            _ => panic!("Expected fallback preset"),
        }
    }

    #[test]
    fn test_model_registry_default() {
        let registry = FalAIModelRegistry::new();
        assert!(registry.is_supported("fal-ai/flux/schnell"));
        assert!(registry.is_supported("fal-ai/flux-pro/v1.1"));
    }

    #[test]
    fn test_model_registry_get() {
        let registry = FalAIModelRegistry::new();
        let model = registry.get("fal-ai/flux/schnell");
        assert!(model.is_some());
        assert_eq!(model.unwrap().name, "Flux Schnell");
    }

    #[test]
    fn test_model_registry_cost() {
        let registry = FalAIModelRegistry::new();
        let cost = registry.get_cost_per_image("fal-ai/flux/schnell");
        assert!(cost > 0.0);
    }

    #[test]
    fn test_model_registry_unknown() {
        let registry = FalAIModelRegistry::new();
        assert!(!registry.is_supported("unknown-model"));
        assert_eq!(registry.get_cost_per_image("unknown-model"), 0.0);
    }

    #[test]
    fn test_map_openai_to_fal_params() {
        let openai_params = serde_json::json!({
            "n": 2,
            "size": "1024x1024",
            "response_format": "url"
        });
        let fal_params = map_openai_to_fal_params(&openai_params);

        assert_eq!(fal_params["num_images"], 2);
        assert_eq!(fal_params["output_format"], "jpeg");
        assert!(fal_params.get("image_size").is_some());
    }

    #[test]
    fn test_model_with_builder() {
        let model = FalAIModel::new("test", "Test Model", "A test model", 0.01)
            .with_max_images(8)
            .with_prompt_enhancement();

        assert_eq!(model.max_images, 8);
        assert!(model.supports_prompt_enhancement);
    }

    #[test]
    fn test_register_custom_model() {
        let mut registry = FalAIModelRegistry::new();
        let custom = FalAIModel::new("custom/model", "Custom", "Custom model", 0.1);
        registry.register(custom);

        assert!(registry.is_supported("custom/model"));
    }

    #[test]
    fn test_list_models() {
        let registry = FalAIModelRegistry::new();
        let models = registry.list_models();
        assert!(!models.is_empty());
    }
}
