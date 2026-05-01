//! iFlytek Spark Model Information
//!
//! Model registry and specifications for Spark models

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::core::types::model::ModelInfo;

/// Model features
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ModelFeature {
    /// Chat completion support
    ChatCompletion,
    /// Streaming response support
    StreamingSupport,
    /// Function calling support
    FunctionCalling,
    /// System message support
    SystemMessages,
    /// WebSocket support
    WebSocketSupport,
}

/// Model pricing information
#[derive(Debug, Clone)]
pub struct ModelPricing {
    /// Input token price (USD per million tokens)
    pub input_price: f64,
    /// Output token price (USD per million tokens)
    pub output_price: f64,
}

impl ModelPricing {
    /// Convert Spark's per-million-token registry pricing into the shared cost model.
    pub fn to_core_model_pricing(&self, model_id: &str) -> crate::core::cost::types::ModelPricing {
        crate::core::cost::types::ModelPricing {
            model: model_id.to_string(),
            input_cost_per_1k_tokens: self.input_price / 1000.0,
            output_cost_per_1k_tokens: self.output_price / 1000.0,
            currency: "USD".to_string(),
            updated_at: chrono::Utc::now(),
            ..Default::default()
        }
    }
}

/// Model limits
#[derive(Debug, Clone)]
pub struct ModelLimits {
    /// Maximum context length
    pub max_context_length: u32,
    /// Maximum output tokens
    pub max_output_tokens: u32,
}

/// Model specification
#[derive(Debug, Clone)]
pub struct ModelSpec {
    /// Model information
    pub model_info: ModelInfo,
    /// Supported features
    pub features: Vec<ModelFeature>,
    /// Pricing information
    pub pricing: ModelPricing,
    /// Limits information
    pub limits: ModelLimits,
}

/// Spark model registry
#[derive(Debug, Clone)]
pub struct SparkModelRegistry {
    models: HashMap<String, ModelSpec>,
}

impl SparkModelRegistry {
    /// Create new registry
    pub fn new() -> Self {
        let mut registry = Self {
            models: HashMap::new(),
        };
        registry.initialize_models();
        registry
    }

    /// Initialize model registry
    fn initialize_models(&mut self) {
        // Spark Desk v3.5 (Latest flagship model)
        self.register_model(
            "spark-desk-v3.5",
            ModelSpec {
                model_info: ModelInfo {
                    id: "spark-desk-v3.5".to_string(),
                    name: "Spark Desk v3.5".to_string(),
                    provider: "spark".to_string(),
                    max_context_length: 8192,
                    max_output_length: Some(4096),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: false,
                    input_cost_per_1k_tokens: Some(0.003),
                    output_cost_per_1k_tokens: Some(0.006),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    ],
                    ..Default::default()
                },
                features: vec![
                    ModelFeature::ChatCompletion,
                    ModelFeature::StreamingSupport,
                    ModelFeature::FunctionCalling,
                    ModelFeature::SystemMessages,
                    ModelFeature::WebSocketSupport,
                ],
                pricing: ModelPricing {
                    input_price: 3.0,  // $3 per million tokens
                    output_price: 6.0, // $6 per million tokens
                },
                limits: ModelLimits {
                    max_context_length: 8192,
                    max_output_tokens: 4096,
                },
            },
        );

        // Spark Desk v3 (Balanced model)
        self.register_model(
            "spark-desk-v3",
            ModelSpec {
                model_info: ModelInfo {
                    id: "spark-desk-v3".to_string(),
                    name: "Spark Desk v3".to_string(),
                    provider: "spark".to_string(),
                    max_context_length: 8192,
                    max_output_length: Some(4096),
                    supports_streaming: true,
                    supports_tools: true,
                    supports_multimodal: false,
                    input_cost_per_1k_tokens: Some(0.0025),
                    output_cost_per_1k_tokens: Some(0.005),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    ],
                    ..Default::default()
                },
                features: vec![
                    ModelFeature::ChatCompletion,
                    ModelFeature::StreamingSupport,
                    ModelFeature::FunctionCalling,
                    ModelFeature::SystemMessages,
                    ModelFeature::WebSocketSupport,
                ],
                pricing: ModelPricing {
                    input_price: 2.5,  // $2.5 per million tokens
                    output_price: 5.0, // $5 per million tokens
                },
                limits: ModelLimits {
                    max_context_length: 8192,
                    max_output_tokens: 4096,
                },
            },
        );

        // Spark Desk v2 (Mid-tier model)
        self.register_model(
            "spark-desk-v2",
            ModelSpec {
                model_info: ModelInfo {
                    id: "spark-desk-v2".to_string(),
                    name: "Spark Desk v2".to_string(),
                    provider: "spark".to_string(),
                    max_context_length: 4096,
                    max_output_length: Some(2048),
                    supports_streaming: true,
                    supports_tools: false,
                    supports_multimodal: false,
                    input_cost_per_1k_tokens: Some(0.002),
                    output_cost_per_1k_tokens: Some(0.004),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    ],
                    ..Default::default()
                },
                features: vec![
                    ModelFeature::ChatCompletion,
                    ModelFeature::StreamingSupport,
                    ModelFeature::SystemMessages,
                    ModelFeature::WebSocketSupport,
                ],
                pricing: ModelPricing {
                    input_price: 2.0,  // $2 per million tokens
                    output_price: 4.0, // $4 per million tokens
                },
                limits: ModelLimits {
                    max_context_length: 4096,
                    max_output_tokens: 2048,
                },
            },
        );

        // Spark Desk v1.5 (Entry-level model)
        self.register_model(
            "spark-desk-v1.5",
            ModelSpec {
                model_info: ModelInfo {
                    id: "spark-desk-v1.5".to_string(),
                    name: "Spark Desk v1.5".to_string(),
                    provider: "spark".to_string(),
                    max_context_length: 4096,
                    max_output_length: Some(2048),
                    supports_streaming: true,
                    supports_tools: false,
                    supports_multimodal: false,
                    input_cost_per_1k_tokens: Some(0.0015),
                    output_cost_per_1k_tokens: Some(0.003),
                    currency: "USD".to_string(),
                    capabilities: vec![
                        crate::core::types::model::ProviderCapability::ChatCompletion,
                        crate::core::types::model::ProviderCapability::ChatCompletionStream,
                    ],
                    ..Default::default()
                },
                features: vec![
                    ModelFeature::ChatCompletion,
                    ModelFeature::StreamingSupport,
                    ModelFeature::SystemMessages,
                    ModelFeature::WebSocketSupport,
                ],
                pricing: ModelPricing {
                    input_price: 1.5,  // $1.5 per million tokens
                    output_price: 3.0, // $3 per million tokens
                },
                limits: ModelLimits {
                    max_context_length: 4096,
                    max_output_tokens: 2048,
                },
            },
        );
    }

    /// Register a model
    fn register_model(&mut self, id: &str, spec: ModelSpec) {
        self.models.insert(id.to_string(), spec);
    }

    /// Get model specification
    pub fn get_model_spec(&self, model_id: &str) -> Option<&ModelSpec> {
        self.models.get(model_id)
    }

    /// Get model pricing in the shared core cost model shape.
    pub fn get_core_model_pricing(
        &self,
        model_id: &str,
    ) -> Option<crate::core::cost::types::ModelPricing> {
        self.get_model_spec(model_id)
            .map(|spec| spec.pricing.to_core_model_pricing(model_id))
    }

    /// List all models
    pub fn list_models(&self) -> Vec<&ModelSpec> {
        self.models.values().collect()
    }

    /// Check if feature is supported
    pub fn supports_feature(&self, model_id: &str, feature: &ModelFeature) -> bool {
        self.get_model_spec(model_id)
            .map(|spec| spec.features.contains(feature))
            .unwrap_or(false)
    }
}

impl Default for SparkModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Get global Spark model registry
pub fn get_spark_registry() -> &'static SparkModelRegistry {
    static REGISTRY: OnceLock<SparkModelRegistry> = OnceLock::new();
    REGISTRY.get_or_init(SparkModelRegistry::new)
}

/// Cost calculator
pub struct CostCalculator;

impl CostCalculator {
    /// Calculate cost for a request
    pub fn calculate_cost(model_id: &str, input_tokens: u32, output_tokens: u32) -> Option<f64> {
        let registry = get_spark_registry();
        let pricing = registry.get_core_model_pricing(model_id)?;

        let input_cost = (input_tokens as f64 / 1000.0) * pricing.input_cost_per_1k_tokens;
        let output_cost = (output_tokens as f64 / 1000.0) * pricing.output_cost_per_1k_tokens;

        Some(input_cost + output_cost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_initialization() {
        let registry = get_spark_registry();
        let models = registry.list_models();

        assert_eq!(models.len(), 4);
        assert!(registry.get_model_spec("spark-desk-v3.5").is_some());
        assert!(registry.get_model_spec("spark-desk-v3").is_some());
        assert!(registry.get_model_spec("spark-desk-v2").is_some());
        assert!(registry.get_model_spec("spark-desk-v1.5").is_some());
    }

    #[test]
    fn test_model_features() {
        let registry = get_spark_registry();

        // v3.5 should support function calling
        assert!(registry.supports_feature("spark-desk-v3.5", &ModelFeature::FunctionCalling));
        assert!(registry.supports_feature("spark-desk-v3.5", &ModelFeature::StreamingSupport));

        // v2 should not support function calling
        assert!(!registry.supports_feature("spark-desk-v2", &ModelFeature::FunctionCalling));
        assert!(registry.supports_feature("spark-desk-v2", &ModelFeature::StreamingSupport));
    }

    #[test]
    fn test_model_limits() {
        let registry = get_spark_registry();

        let v3_5_spec = registry.get_model_spec("spark-desk-v3.5").unwrap();
        assert_eq!(v3_5_spec.limits.max_context_length, 8192);
        assert_eq!(v3_5_spec.limits.max_output_tokens, 4096);

        let v2_spec = registry.get_model_spec("spark-desk-v2").unwrap();
        assert_eq!(v2_spec.limits.max_context_length, 4096);
        assert_eq!(v2_spec.limits.max_output_tokens, 2048);
    }

    #[test]
    fn test_core_model_pricing_conversion() {
        let registry = get_spark_registry();
        let pricing = registry
            .get_core_model_pricing("spark-desk-v3.5")
            .expect("registry pricing should convert to core pricing");

        assert_eq!(pricing.model, "spark-desk-v3.5");
        assert_eq!(pricing.input_cost_per_1k_tokens, 0.003);
        assert_eq!(pricing.output_cost_per_1k_tokens, 0.006);
        assert_eq!(pricing.currency, "USD");
    }

    #[test]
    fn test_cost_calculation() {
        let cost = CostCalculator::calculate_cost("spark-desk-v3.5", 1000, 500);
        assert!(cost.is_some());

        let cost_value = cost.unwrap();
        // 1000 tokens * $3/1M + 500 tokens * $6/1M = $0.003 + $0.003 = $0.006
        assert!((cost_value - 0.006).abs() < 0.0001);
    }

    #[test]
    fn test_unknown_model() {
        let registry = get_spark_registry();
        assert!(registry.get_model_spec("unknown-model").is_none());

        let cost = CostCalculator::calculate_cost("unknown-model", 1000, 500);
        assert!(cost.is_none());
    }
}
