//! Voyage AI Model Information
//!
//! Static model information for Voyage AI embedding models including
//! dimensions, context lengths, and pricing.

/// Voyage AI model information
#[derive(Debug, Clone)]
pub struct VoyageModel {
    pub model_id: &'static str,
    pub display_name: &'static str,
    pub max_tokens: u32,
    pub embedding_dimensions: u32,
    pub supports_truncation: bool,
    pub cost_per_million_tokens: f64,
}

/// Static model registry for Voyage AI
const VOYAGE_MODELS: &[VoyageModel] = &[
    // Voyage 4 models
    VoyageModel {
        model_id: "voyage-4-large",
        display_name: "Voyage 4 Large",
        max_tokens: 32000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.12,
    },
    VoyageModel {
        model_id: "voyage-4",
        display_name: "Voyage 4",
        max_tokens: 32000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.06,
    },
    VoyageModel {
        model_id: "voyage-4-lite",
        display_name: "Voyage 4 Lite",
        max_tokens: 32000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.02,
    },
    // Voyage 3 models
    VoyageModel {
        model_id: "voyage-3-large",
        display_name: "Voyage 3 Large",
        max_tokens: 32000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.12,
    },
    VoyageModel {
        model_id: "voyage-3.5",
        display_name: "Voyage 3.5",
        max_tokens: 32000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.06,
    },
    VoyageModel {
        model_id: "voyage-3.5-lite",
        display_name: "Voyage 3.5 Lite",
        max_tokens: 32000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.02,
    },
    VoyageModel {
        model_id: "voyage-3",
        display_name: "Voyage 3",
        max_tokens: 32000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.06,
    },
    VoyageModel {
        model_id: "voyage-3-lite",
        display_name: "Voyage 3 Lite",
        max_tokens: 32000,
        embedding_dimensions: 512,
        supports_truncation: true,
        cost_per_million_tokens: 0.02,
    },
    // Voyage 2 models
    VoyageModel {
        model_id: "voyage-2",
        display_name: "Voyage 2",
        max_tokens: 4000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.10,
    },
    VoyageModel {
        model_id: "voyage-large-2",
        display_name: "Voyage Large 2",
        max_tokens: 16000,
        embedding_dimensions: 1536,
        supports_truncation: true,
        cost_per_million_tokens: 0.12,
    },
    VoyageModel {
        model_id: "voyage-large-2-instruct",
        display_name: "Voyage Large 2 Instruct",
        max_tokens: 16000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.12,
    },
    // Code models
    VoyageModel {
        model_id: "voyage-code-2",
        display_name: "Voyage Code 2",
        max_tokens: 16000,
        embedding_dimensions: 1536,
        supports_truncation: true,
        cost_per_million_tokens: 0.12,
    },
    VoyageModel {
        model_id: "voyage-code-3",
        display_name: "Voyage Code 3",
        max_tokens: 32000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.18,
    },
    // Finance model
    VoyageModel {
        model_id: "voyage-finance-2",
        display_name: "Voyage Finance 2",
        max_tokens: 32000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.12,
    },
    // Law model
    VoyageModel {
        model_id: "voyage-law-2",
        display_name: "Voyage Law 2",
        max_tokens: 16000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.12,
    },
    // Multilingual model
    VoyageModel {
        model_id: "voyage-multilingual-2",
        display_name: "Voyage Multilingual 2",
        max_tokens: 32000,
        embedding_dimensions: 1024,
        supports_truncation: true,
        cost_per_million_tokens: 0.12,
    },
];

/// Get available model IDs
pub fn get_available_models() -> Vec<&'static str> {
    VOYAGE_MODELS.iter().map(|m| m.model_id).collect()
}

/// Get model information by ID
pub fn get_model_info(model_id: &str) -> Option<&'static VoyageModel> {
    // Normalize model ID by removing common prefixes
    let normalized = normalize_model_id(model_id);

    VOYAGE_MODELS
        .iter()
        .find(|m| m.model_id == normalized || m.model_id == model_id)
}

/// Normalize model ID by removing prefixes
fn normalize_model_id(model_id: &str) -> &str {
    model_id
        .trim_start_matches("voyage/")
        .trim_start_matches("voyage_ai/")
}

/// Get default embedding model
#[cfg(test)]
pub fn get_default_model() -> &'static str {
    "voyage-4-large"
}

/// Get model dimensions
#[cfg(test)]
pub fn get_model_dimensions(model_id: &str) -> Option<u32> {
    get_model_info(model_id).map(|m| m.embedding_dimensions)
}

/// Check if model supports custom dimensions
pub fn supports_custom_dimensions(model_id: &str) -> bool {
    let normalized = normalize_model_id(model_id);
    normalized.starts_with("voyage-4")
        || matches!(
            normalized,
            "voyage-3-large" | "voyage-3.5" | "voyage-3.5-lite" | "voyage-code-3"
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_available_models() {
        let models = get_available_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"voyage-4-large"));
        assert!(models.contains(&"voyage-2"));
    }

    #[test]
    fn test_get_model_info() {
        let model = get_model_info("voyage-4-large").unwrap();
        assert_eq!(model.display_name, "Voyage 4 Large");
        assert_eq!(model.embedding_dimensions, 1024);
        assert_eq!(model.max_tokens, 32000);
    }

    #[test]
    fn test_get_model_info_with_prefix() {
        let model = get_model_info("voyage/voyage-4-large");
        assert!(model.is_some());

        let model = get_model_info("voyage_ai/voyage-4-large");
        assert!(model.is_some());
    }

    #[test]
    fn test_get_model_info_not_found() {
        let model = get_model_info("unknown-model");
        assert!(model.is_none());
    }

    #[test]
    fn test_get_default_model() {
        assert_eq!(get_default_model(), "voyage-4-large");
    }

    #[test]
    fn test_get_model_dimensions() {
        assert_eq!(get_model_dimensions("voyage-4-large"), Some(1024));
        assert_eq!(get_model_dimensions("voyage-4"), Some(1024));
        assert_eq!(get_model_dimensions("voyage-4-lite"), Some(1024));
        assert_eq!(get_model_dimensions("voyage-3"), Some(1024));
        assert_eq!(get_model_dimensions("voyage-3-lite"), Some(512));
        assert_eq!(get_model_dimensions("voyage-large-2"), Some(1536));
        assert_eq!(get_model_dimensions("unknown"), None);
    }

    #[test]
    fn test_supports_custom_dimensions() {
        assert!(supports_custom_dimensions("voyage-4-large"));
        assert!(supports_custom_dimensions("voyage-4"));
        assert!(supports_custom_dimensions("voyage-3.5"));
        assert!(supports_custom_dimensions("voyage-code-3"));
        assert!(!supports_custom_dimensions("voyage-3"));
        assert!(!supports_custom_dimensions("voyage-3-lite"));
        assert!(!supports_custom_dimensions("voyage-2"));
        assert!(!supports_custom_dimensions("voyage-large-2"));
    }

    #[test]
    fn test_model_pricing() {
        let model = get_model_info("voyage-4").unwrap();
        assert_eq!(model.cost_per_million_tokens, 0.06);

        let model = get_model_info("voyage-4-lite").unwrap();
        assert_eq!(model.cost_per_million_tokens, 0.02);

        let model = get_model_info("voyage-code-3").unwrap();
        assert_eq!(model.cost_per_million_tokens, 0.18);
    }

    #[test]
    fn test_code_models() {
        let model = get_model_info("voyage-code-2").unwrap();
        assert_eq!(model.embedding_dimensions, 1536);

        let model = get_model_info("voyage-code-3").unwrap();
        assert_eq!(model.embedding_dimensions, 1024);
    }

    #[test]
    fn test_domain_models() {
        // Finance model
        let model = get_model_info("voyage-finance-2").unwrap();
        assert_eq!(model.display_name, "Voyage Finance 2");

        // Law model
        let model = get_model_info("voyage-law-2").unwrap();
        assert_eq!(model.display_name, "Voyage Law 2");

        // Multilingual model
        let model = get_model_info("voyage-multilingual-2").unwrap();
        assert_eq!(model.display_name, "Voyage Multilingual 2");
    }
}
