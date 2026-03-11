//! Together AI Model Information
//!
//! Contains model configurations and capabilities for Together AI supported models.
//! Together AI provides access to various open-source and proprietary models.
//!
//! Docs: <https://docs.together.ai/docs/serverless-models>

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Together AI model identifier
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TogetherModel {
    // Meta Llama models
    Llama3_3_70B_Instruct_Turbo,
    Llama3_2_90B_Vision_Instruct_Turbo,
    Llama3_2_11B_Vision_Instruct_Turbo,
    Llama3_2_3B_Instruct_Turbo,
    Llama3_1_405B_Instruct_Turbo,
    Llama3_1_70B_Instruct_Turbo,
    Llama3_1_8B_Instruct_Turbo,
    Llama3_70B_Instruct_Turbo,
    Llama3_8B_Instruct_Turbo,

    // DeepSeek models
    DeepSeekV3,
    DeepSeekR1,
    DeepSeekR1_Distill_Llama_70B,
    DeepSeekR1_Distill_Qwen_32B,

    // Qwen models
    Qwen2_5_72B_Instruct_Turbo,
    Qwen2_5_7B_Instruct_Turbo,
    QwQ_32B_Preview,

    // Mistral models
    Mixtral_8x22B_Instruct,
    Mixtral_8x7B_Instruct,
    Mistral_7B_Instruct,

    // Google models
    Gemma2_27B_IT,
    Gemma2_9B_IT,

    // NVIDIA models
    Llama3_1_Nemotron_70B_Instruct,

    // Embedding models
    M2_BERT_80M_2k,
    M2_BERT_80M_32k,
    UAE_Large_V1,
    BGE_Large_EN_V1_5,
    BGE_Base_EN_V1_5,

    // Rerank models
    RerankV2,
    Rerank_English_V3,
    Rerank_Multilingual_V3,
}

/// Model configuration
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Model ID as used in API calls
    pub model_id: &'static str,

    /// Human-friendly model name
    pub display_name: &'static str,

    /// Maximum context length (tokens)
    pub max_context_length: u32,

    /// Maximum output tokens
    pub max_output_length: u32,

    /// Whether the model supports tool/function calling
    pub supports_tools: bool,

    /// Whether the model supports vision
    pub supports_multimodal: bool,

    /// Whether this is an embedding model
    pub is_embedding: bool,

    /// Whether this is a rerank model
    pub is_rerank: bool,

    /// Cost per 1M input tokens (USD)
    pub input_cost_per_million: f64,

    /// Cost per 1M output tokens (USD)
    pub output_cost_per_million: f64,
}

/// Static model configurations
static MODEL_CONFIGS: LazyLock<HashMap<&'static str, ModelInfo>> = LazyLock::new(|| {
    let mut configs = HashMap::new();

    // Meta Llama 3.3 models
    configs.insert(
        "meta-llama/Llama-3.3-70B-Instruct-Turbo",
        ModelInfo {
            model_id: "meta-llama/Llama-3.3-70B-Instruct-Turbo",
            display_name: "Llama 3.3 70B Instruct Turbo",
            max_context_length: 131072,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.88,
            output_cost_per_million: 0.88,
        },
    );

    // Meta Llama 3.2 Vision models
    configs.insert(
        "meta-llama/Llama-3.2-90B-Vision-Instruct-Turbo",
        ModelInfo {
            model_id: "meta-llama/Llama-3.2-90B-Vision-Instruct-Turbo",
            display_name: "Llama 3.2 90B Vision Instruct Turbo",
            max_context_length: 131072,
            max_output_length: 4096,
            supports_tools: false,
            supports_multimodal: true,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 1.20,
            output_cost_per_million: 1.20,
        },
    );

    configs.insert(
        "meta-llama/Llama-3.2-11B-Vision-Instruct-Turbo",
        ModelInfo {
            model_id: "meta-llama/Llama-3.2-11B-Vision-Instruct-Turbo",
            display_name: "Llama 3.2 11B Vision Instruct Turbo",
            max_context_length: 131072,
            max_output_length: 4096,
            supports_tools: false,
            supports_multimodal: true,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.18,
            output_cost_per_million: 0.18,
        },
    );

    configs.insert(
        "meta-llama/Llama-3.2-3B-Instruct-Turbo",
        ModelInfo {
            model_id: "meta-llama/Llama-3.2-3B-Instruct-Turbo",
            display_name: "Llama 3.2 3B Instruct Turbo",
            max_context_length: 131072,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.06,
            output_cost_per_million: 0.06,
        },
    );

    // Meta Llama 3.1 models
    configs.insert(
        "meta-llama/Meta-Llama-3.1-405B-Instruct-Turbo",
        ModelInfo {
            model_id: "meta-llama/Meta-Llama-3.1-405B-Instruct-Turbo",
            display_name: "Llama 3.1 405B Instruct Turbo",
            max_context_length: 131072,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 3.50,
            output_cost_per_million: 3.50,
        },
    );

    configs.insert(
        "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo",
        ModelInfo {
            model_id: "meta-llama/Meta-Llama-3.1-70B-Instruct-Turbo",
            display_name: "Llama 3.1 70B Instruct Turbo",
            max_context_length: 131072,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.88,
            output_cost_per_million: 0.88,
        },
    );

    configs.insert(
        "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo",
        ModelInfo {
            model_id: "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo",
            display_name: "Llama 3.1 8B Instruct Turbo",
            max_context_length: 131072,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.18,
            output_cost_per_million: 0.18,
        },
    );

    // Meta Llama 3 models
    configs.insert(
        "meta-llama/Llama-3-70b-chat-hf",
        ModelInfo {
            model_id: "meta-llama/Llama-3-70b-chat-hf",
            display_name: "Llama 3 70B Chat",
            max_context_length: 8192,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.90,
            output_cost_per_million: 0.90,
        },
    );

    configs.insert(
        "meta-llama/Llama-3-8b-chat-hf",
        ModelInfo {
            model_id: "meta-llama/Llama-3-8b-chat-hf",
            display_name: "Llama 3 8B Chat",
            max_context_length: 8192,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.20,
            output_cost_per_million: 0.20,
        },
    );

    // DeepSeek models
    configs.insert(
        "deepseek-ai/DeepSeek-V3",
        ModelInfo {
            model_id: "deepseek-ai/DeepSeek-V3",
            display_name: "DeepSeek V3",
            max_context_length: 131072,
            max_output_length: 8192,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 1.25,
            output_cost_per_million: 1.25,
        },
    );

    configs.insert(
        "deepseek-ai/DeepSeek-R1",
        ModelInfo {
            model_id: "deepseek-ai/DeepSeek-R1",
            display_name: "DeepSeek R1",
            max_context_length: 131072,
            max_output_length: 8192,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 3.00,
            output_cost_per_million: 7.00,
        },
    );

    configs.insert(
        "deepseek-ai/DeepSeek-R1-Distill-Llama-70B",
        ModelInfo {
            model_id: "deepseek-ai/DeepSeek-R1-Distill-Llama-70B",
            display_name: "DeepSeek R1 Distill Llama 70B",
            max_context_length: 131072,
            max_output_length: 8192,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.88,
            output_cost_per_million: 0.88,
        },
    );

    configs.insert(
        "deepseek-ai/DeepSeek-R1-Distill-Qwen-32B",
        ModelInfo {
            model_id: "deepseek-ai/DeepSeek-R1-Distill-Qwen-32B",
            display_name: "DeepSeek R1 Distill Qwen 32B",
            max_context_length: 131072,
            max_output_length: 8192,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.65,
            output_cost_per_million: 0.65,
        },
    );

    // Qwen models
    configs.insert(
        "Qwen/Qwen2.5-72B-Instruct-Turbo",
        ModelInfo {
            model_id: "Qwen/Qwen2.5-72B-Instruct-Turbo",
            display_name: "Qwen 2.5 72B Instruct Turbo",
            max_context_length: 131072,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 1.20,
            output_cost_per_million: 1.20,
        },
    );

    configs.insert(
        "Qwen/Qwen2.5-7B-Instruct-Turbo",
        ModelInfo {
            model_id: "Qwen/Qwen2.5-7B-Instruct-Turbo",
            display_name: "Qwen 2.5 7B Instruct Turbo",
            max_context_length: 131072,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.30,
            output_cost_per_million: 0.30,
        },
    );

    configs.insert(
        "Qwen/QwQ-32B-Preview",
        ModelInfo {
            model_id: "Qwen/QwQ-32B-Preview",
            display_name: "QwQ 32B Preview",
            max_context_length: 32768,
            max_output_length: 4096,
            supports_tools: false,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 1.20,
            output_cost_per_million: 1.20,
        },
    );

    // Mistral models
    configs.insert(
        "mistralai/Mixtral-8x22B-Instruct-v0.1",
        ModelInfo {
            model_id: "mistralai/Mixtral-8x22B-Instruct-v0.1",
            display_name: "Mixtral 8x22B Instruct",
            max_context_length: 65536,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 1.20,
            output_cost_per_million: 1.20,
        },
    );

    configs.insert(
        "mistralai/Mixtral-8x7B-Instruct-v0.1",
        ModelInfo {
            model_id: "mistralai/Mixtral-8x7B-Instruct-v0.1",
            display_name: "Mixtral 8x7B Instruct",
            max_context_length: 32768,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.60,
            output_cost_per_million: 0.60,
        },
    );

    configs.insert(
        "mistralai/Mistral-7B-Instruct-v0.3",
        ModelInfo {
            model_id: "mistralai/Mistral-7B-Instruct-v0.3",
            display_name: "Mistral 7B Instruct",
            max_context_length: 32768,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.20,
            output_cost_per_million: 0.20,
        },
    );

    // Google Gemma models
    configs.insert(
        "google/gemma-2-27b-it",
        ModelInfo {
            model_id: "google/gemma-2-27b-it",
            display_name: "Gemma 2 27B IT",
            max_context_length: 8192,
            max_output_length: 4096,
            supports_tools: false,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.80,
            output_cost_per_million: 0.80,
        },
    );

    configs.insert(
        "google/gemma-2-9b-it",
        ModelInfo {
            model_id: "google/gemma-2-9b-it",
            display_name: "Gemma 2 9B IT",
            max_context_length: 8192,
            max_output_length: 4096,
            supports_tools: false,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.30,
            output_cost_per_million: 0.30,
        },
    );

    // NVIDIA models
    configs.insert(
        "nvidia/Llama-3.1-Nemotron-70B-Instruct-HF",
        ModelInfo {
            model_id: "nvidia/Llama-3.1-Nemotron-70B-Instruct-HF",
            display_name: "Llama 3.1 Nemotron 70B Instruct",
            max_context_length: 131072,
            max_output_length: 4096,
            supports_tools: true,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: false,
            input_cost_per_million: 0.88,
            output_cost_per_million: 0.88,
        },
    );

    // Embedding models
    configs.insert(
        "togethercomputer/m2-bert-80M-2k-retrieval",
        ModelInfo {
            model_id: "togethercomputer/m2-bert-80M-2k-retrieval",
            display_name: "M2 BERT 80M 2K Retrieval",
            max_context_length: 2048,
            max_output_length: 0,
            supports_tools: false,
            supports_multimodal: false,
            is_embedding: true,
            is_rerank: false,
            input_cost_per_million: 0.008,
            output_cost_per_million: 0.0,
        },
    );

    configs.insert(
        "togethercomputer/m2-bert-80M-32k-retrieval",
        ModelInfo {
            model_id: "togethercomputer/m2-bert-80M-32k-retrieval",
            display_name: "M2 BERT 80M 32K Retrieval",
            max_context_length: 32768,
            max_output_length: 0,
            supports_tools: false,
            supports_multimodal: false,
            is_embedding: true,
            is_rerank: false,
            input_cost_per_million: 0.008,
            output_cost_per_million: 0.0,
        },
    );

    configs.insert(
        "WhereIsAI/UAE-Large-V1",
        ModelInfo {
            model_id: "WhereIsAI/UAE-Large-V1",
            display_name: "UAE Large V1",
            max_context_length: 512,
            max_output_length: 0,
            supports_tools: false,
            supports_multimodal: false,
            is_embedding: true,
            is_rerank: false,
            input_cost_per_million: 0.016,
            output_cost_per_million: 0.0,
        },
    );

    configs.insert(
        "BAAI/bge-large-en-v1.5",
        ModelInfo {
            model_id: "BAAI/bge-large-en-v1.5",
            display_name: "BGE Large EN V1.5",
            max_context_length: 512,
            max_output_length: 0,
            supports_tools: false,
            supports_multimodal: false,
            is_embedding: true,
            is_rerank: false,
            input_cost_per_million: 0.016,
            output_cost_per_million: 0.0,
        },
    );

    configs.insert(
        "BAAI/bge-base-en-v1.5",
        ModelInfo {
            model_id: "BAAI/bge-base-en-v1.5",
            display_name: "BGE Base EN V1.5",
            max_context_length: 512,
            max_output_length: 0,
            supports_tools: false,
            supports_multimodal: false,
            is_embedding: true,
            is_rerank: false,
            input_cost_per_million: 0.008,
            output_cost_per_million: 0.0,
        },
    );

    // Rerank models
    configs.insert(
        "Salesforce/Llama-Rank-V1",
        ModelInfo {
            model_id: "Salesforce/Llama-Rank-V1",
            display_name: "Llama Rank V1",
            max_context_length: 8192,
            max_output_length: 0,
            supports_tools: false,
            supports_multimodal: false,
            is_embedding: false,
            is_rerank: true,
            input_cost_per_million: 0.30,
            output_cost_per_million: 0.0,
        },
    );

    configs
});

/// Get model information for a given model ID
pub fn get_model_info(model_id: &str) -> Option<&'static ModelInfo> {
    MODEL_CONFIGS.get(model_id)
}

/// Check if a model supports function calling
pub fn is_function_calling_model(model_id: &str) -> bool {
    get_model_info(model_id)
        .map(|info| info.supports_tools)
        .unwrap_or(false)
}

/// Check if a model supports vision
#[cfg(test)]
pub fn is_vision_model(model_id: &str) -> bool {
    get_model_info(model_id)
        .map(|info| info.supports_multimodal)
        .unwrap_or(false)
}

/// Check if a model is an embedding model
#[cfg(test)]
pub fn is_embedding_model(model_id: &str) -> bool {
    get_model_info(model_id)
        .map(|info| info.is_embedding)
        .unwrap_or(false)
}

/// Check if a model is a rerank model
#[cfg(test)]
pub fn is_rerank_model(model_id: &str) -> bool {
    get_model_info(model_id)
        .map(|info| info.is_rerank)
        .unwrap_or(false)
}

/// Get all available model IDs
pub fn get_available_models() -> Vec<&'static str> {
    MODEL_CONFIGS.keys().copied().collect()
}

/// Get all models that support tool/function calling
#[cfg(test)]
pub fn get_tool_capable_models() -> Vec<&'static str> {
    MODEL_CONFIGS
        .iter()
        .filter(|(_, info)| info.supports_tools)
        .map(|(id, _)| *id)
        .collect()
}

/// Get all embedding models
#[cfg(test)]
pub fn get_embedding_models() -> Vec<&'static str> {
    MODEL_CONFIGS
        .iter()
        .filter(|(_, info)| info.is_embedding)
        .map(|(id, _)| *id)
        .collect()
}

/// Get all rerank models
#[cfg(test)]
pub fn get_rerank_models() -> Vec<&'static str> {
    MODEL_CONFIGS
        .iter()
        .filter(|(_, info)| info.is_rerank)
        .map(|(id, _)| *id)
        .collect()
}

/// Get pricing category based on model size (for Together AI pricing tiers)
/// Returns the pricing category string used for cost calculation
#[cfg(test)]
pub fn get_pricing_category(model_name: &str) -> Option<&'static str> {
    // Extract parameter count from model name (e.g., "70B", "8B")
    let model_lower = model_name.to_lowercase();

    // Simple pattern matching for parameter counts without regex
    // Look for patterns like "70b", "8b", "3b", etc.
    let mut params: Option<u32> = None;

    for word in model_lower.split(|c: char| !c.is_alphanumeric()) {
        if word.ends_with('b')
            && let Ok(num) = word.trim_end_matches('b').parse::<u32>()
        {
            params = Some(num);
            break;
        }
    }

    params.map(|p| match p {
        0..=4 => "together-ai-up-to-4b",
        5..=8 => "together-ai-4.1b-8b",
        9..=21 => "together-ai-8.1b-21b",
        22..=41 => "together-ai-21.1b-41b",
        42..=80 => "together-ai-41.1b-80b",
        _ => "together-ai-81.1b-110b",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_model_info_valid() {
        let info = get_model_info("meta-llama/Llama-3.3-70B-Instruct-Turbo");
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.model_id, "meta-llama/Llama-3.3-70B-Instruct-Turbo");
        assert_eq!(info.display_name, "Llama 3.3 70B Instruct Turbo");
        assert_eq!(info.max_context_length, 131072);
        assert!(info.supports_tools);
        assert!(!info.supports_multimodal);
    }

    #[test]
    fn test_get_model_info_invalid() {
        let info = get_model_info("nonexistent-model");
        assert!(info.is_none());
    }

    #[test]
    fn test_is_function_calling_model() {
        assert!(is_function_calling_model(
            "meta-llama/Llama-3.3-70B-Instruct-Turbo"
        ));
        assert!(is_function_calling_model("deepseek-ai/DeepSeek-V3"));
        assert!(!is_function_calling_model(
            "meta-llama/Llama-3.2-90B-Vision-Instruct-Turbo"
        ));
        assert!(!is_function_calling_model("nonexistent-model"));
    }

    #[test]
    fn test_is_vision_model() {
        assert!(is_vision_model(
            "meta-llama/Llama-3.2-90B-Vision-Instruct-Turbo"
        ));
        assert!(is_vision_model(
            "meta-llama/Llama-3.2-11B-Vision-Instruct-Turbo"
        ));
        assert!(!is_vision_model("meta-llama/Llama-3.3-70B-Instruct-Turbo"));
        assert!(!is_vision_model("nonexistent-model"));
    }

    #[test]
    fn test_is_embedding_model() {
        assert!(is_embedding_model(
            "togethercomputer/m2-bert-80M-2k-retrieval"
        ));
        assert!(is_embedding_model("BAAI/bge-large-en-v1.5"));
        assert!(!is_embedding_model(
            "meta-llama/Llama-3.3-70B-Instruct-Turbo"
        ));
        assert!(!is_embedding_model("nonexistent-model"));
    }

    #[test]
    fn test_is_rerank_model() {
        assert!(is_rerank_model("Salesforce/Llama-Rank-V1"));
        assert!(!is_rerank_model("meta-llama/Llama-3.3-70B-Instruct-Turbo"));
        assert!(!is_rerank_model("nonexistent-model"));
    }

    #[test]
    fn test_get_available_models() {
        let models = get_available_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"meta-llama/Llama-3.3-70B-Instruct-Turbo"));
        assert!(models.contains(&"deepseek-ai/DeepSeek-V3"));
    }

    #[test]
    fn test_get_tool_capable_models() {
        let models = get_tool_capable_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"meta-llama/Llama-3.3-70B-Instruct-Turbo"));
        // Embedding models don't support tools
        assert!(!models.contains(&"togethercomputer/m2-bert-80M-2k-retrieval"));
    }

    #[test]
    fn test_get_embedding_models() {
        let models = get_embedding_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"togethercomputer/m2-bert-80M-2k-retrieval"));
        assert!(models.contains(&"BAAI/bge-large-en-v1.5"));
    }

    #[test]
    fn test_get_rerank_models() {
        let models = get_rerank_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"Salesforce/Llama-Rank-V1"));
    }

    #[test]
    fn test_model_info_costs() {
        let info = get_model_info("meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo").unwrap();
        assert!(info.input_cost_per_million > 0.0);
        assert!(info.output_cost_per_million > 0.0);
    }

    #[test]
    fn test_together_model_enum() {
        let model = TogetherModel::Llama3_3_70B_Instruct_Turbo;
        assert_eq!(format!("{:?}", model), "Llama3_3_70B_Instruct_Turbo");

        let model = TogetherModel::DeepSeekV3;
        assert_eq!(format!("{:?}", model), "DeepSeekV3");
    }

    #[test]
    fn test_deepseek_models() {
        let v3 = get_model_info("deepseek-ai/DeepSeek-V3").unwrap();
        assert!(v3.supports_tools);
        assert!(!v3.supports_multimodal);
        assert_eq!(v3.max_context_length, 131072);

        let r1 = get_model_info("deepseek-ai/DeepSeek-R1").unwrap();
        assert!(r1.supports_tools);
    }

    #[test]
    fn test_qwen_models() {
        let qwen = get_model_info("Qwen/Qwen2.5-72B-Instruct-Turbo").unwrap();
        assert_eq!(qwen.display_name, "Qwen 2.5 72B Instruct Turbo");
        assert!(qwen.supports_tools);
    }

    #[test]
    fn test_mistral_models() {
        let mixtral = get_model_info("mistralai/Mixtral-8x22B-Instruct-v0.1").unwrap();
        assert!(mixtral.supports_tools);
        assert_eq!(mixtral.max_context_length, 65536);
    }

    #[test]
    fn test_pricing_category() {
        assert_eq!(
            get_pricing_category("model-3b"),
            Some("together-ai-up-to-4b")
        );
        assert_eq!(
            get_pricing_category("model-7b"),
            Some("together-ai-4.1b-8b")
        );
        assert_eq!(
            get_pricing_category("model-13b"),
            Some("together-ai-8.1b-21b")
        );
        assert_eq!(
            get_pricing_category("model-34b"),
            Some("together-ai-21.1b-41b")
        );
        assert_eq!(
            get_pricing_category("model-70b"),
            Some("together-ai-41.1b-80b")
        );
        assert_eq!(
            get_pricing_category("model-100b"),
            Some("together-ai-81.1b-110b")
        );
        assert_eq!(get_pricing_category("model-unknown"), None);
    }
}
