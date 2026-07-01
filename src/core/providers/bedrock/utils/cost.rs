//! Cost Calculation for Bedrock Models
//!
//! Provides accurate pricing information and cost calculation
//! for all supported Bedrock models.

use std::collections::HashMap;
use std::sync::LazyLock;

pub use crate::core::cost::types::ModelPricing;

fn insert_pricing(
    pricing: &mut HashMap<&'static str, ModelPricing>,
    model_id: &'static str,
    input_cost_per_1k_tokens: f64,
    output_cost_per_1k_tokens: f64,
) {
    pricing.insert(
        model_id,
        ModelPricing {
            model: model_id.to_string(),
            input_cost_per_1k_tokens,
            output_cost_per_1k_tokens,
            ..Default::default()
        },
    );
}

fn currency(pricing: &ModelPricing) -> &'static str {
    if pricing.currency == "USD" {
        "USD"
    } else {
        "UNKNOWN"
    }
}

/// Comprehensive pricing database for all Bedrock models
static MODEL_PRICING: LazyLock<HashMap<&'static str, ModelPricing>> = LazyLock::new(|| {
    let mut pricing = HashMap::new();

    // Claude models
    insert_pricing(&mut pricing, "anthropic.claude-opus-4-6-v1:0", 0.005, 0.025);
    insert_pricing(&mut pricing, "anthropic.claude-opus-4-6-v1", 0.005, 0.025);
    insert_pricing(&mut pricing, "anthropic.claude-opus-4-6", 0.005, 0.025);
    insert_pricing(&mut pricing, "anthropic.claude-opus-4-5-v1:0", 0.005, 0.025);
    insert_pricing(&mut pricing, "anthropic.claude-opus-4-5", 0.005, 0.025);
    insert_pricing(
        &mut pricing,
        "anthropic.claude-sonnet-4-5-v1:0",
        0.003,
        0.015,
    );
    insert_pricing(&mut pricing, "anthropic.claude-sonnet-4-5", 0.003, 0.015);
    insert_pricing(&mut pricing, "anthropic.claude-sonnet-4-v1:0", 0.003, 0.015);
    insert_pricing(&mut pricing, "anthropic.claude-sonnet-4", 0.003, 0.015);

    insert_pricing(
        &mut pricing,
        "anthropic.claude-3-opus-20240229",
        0.015,
        0.075,
    );
    insert_pricing(
        &mut pricing,
        "anthropic.claude-3-sonnet-20240229",
        0.003,
        0.015,
    );
    insert_pricing(
        &mut pricing,
        "anthropic.claude-3-haiku-20240307",
        0.00025,
        0.00125,
    );
    insert_pricing(
        &mut pricing,
        "anthropic.claude-3-5-sonnet-20241022",
        0.003,
        0.015,
    );
    insert_pricing(
        &mut pricing,
        "anthropic.claude-3-5-sonnet-20241022-v2:0",
        0.003,
        0.015,
    );
    insert_pricing(
        &mut pricing,
        "anthropic.claude-3-5-haiku-20241022",
        0.001,
        0.005,
    );
    insert_pricing(&mut pricing, "anthropic.claude-v2:1", 0.008, 0.024);
    insert_pricing(&mut pricing, "anthropic.claude-v2", 0.008, 0.024);
    insert_pricing(
        &mut pricing,
        "anthropic.claude-instant-v1",
        0.00163,
        0.00551,
    );

    // Titan models
    insert_pricing(&mut pricing, "amazon.titan-text-express-v1", 0.0002, 0.0006);
    insert_pricing(&mut pricing, "amazon.titan-text-lite-v1", 0.00015, 0.0002);
    insert_pricing(
        &mut pricing,
        "amazon.titan-text-premier-v1:0",
        0.0005,
        0.0015,
    );

    // Nova models
    insert_pricing(&mut pricing, "amazon.nova-micro-v1:0", 0.000035, 0.00014);
    insert_pricing(&mut pricing, "amazon.nova-lite-v1:0", 0.00006, 0.00024);
    insert_pricing(&mut pricing, "amazon.nova-pro-v1:0", 0.0008, 0.0032);

    // AI21 models
    insert_pricing(&mut pricing, "ai21.jamba-1-5-large-v1:0", 0.002, 0.008);
    insert_pricing(&mut pricing, "ai21.jamba-1-5-mini-v1:0", 0.0002, 0.0004);
    insert_pricing(&mut pricing, "ai21.jamba-instruct-v1:0", 0.0005, 0.0007);

    // Cohere models
    insert_pricing(&mut pricing, "cohere.command-r-plus-v1:0", 0.003, 0.015);
    insert_pricing(&mut pricing, "cohere.command-r-v1:0", 0.0005, 0.0015);
    insert_pricing(&mut pricing, "cohere.command-text-v14", 0.0015, 0.002);
    insert_pricing(
        &mut pricing,
        "cohere.command-light-text-v14",
        0.0003,
        0.0006,
    );

    // Mistral models
    insert_pricing(
        &mut pricing,
        "mistral.mistral-7b-instruct-v0:2",
        0.00015,
        0.0002,
    );
    insert_pricing(
        &mut pricing,
        "mistral.mixtral-8x7b-instruct-v0:1",
        0.00045,
        0.0007,
    );
    insert_pricing(
        &mut pricing,
        "mistral.mistral-large-2402-v1:0",
        0.004,
        0.012,
    );
    insert_pricing(
        &mut pricing,
        "mistral.mistral-large-2407-v1:0",
        0.002,
        0.006,
    );
    insert_pricing(
        &mut pricing,
        "mistral.mistral-small-2402-v1:0",
        0.001,
        0.003,
    );

    // Meta Llama models
    insert_pricing(
        &mut pricing,
        "meta.llama3-2-1b-instruct-v1:0",
        0.00001,
        0.00001,
    );
    insert_pricing(
        &mut pricing,
        "meta.llama3-2-3b-instruct-v1:0",
        0.000015,
        0.000015,
    );
    insert_pricing(
        &mut pricing,
        "meta.llama3-2-11b-instruct-v1:0",
        0.000032,
        0.000032,
    );
    insert_pricing(
        &mut pricing,
        "meta.llama3-2-90b-instruct-v1:0",
        0.00072,
        0.00072,
    );
    insert_pricing(
        &mut pricing,
        "meta.llama3-1-8b-instruct-v1:0",
        0.00022,
        0.00022,
    );
    insert_pricing(
        &mut pricing,
        "meta.llama3-1-70b-instruct-v1:0",
        0.00099,
        0.00099,
    );
    insert_pricing(
        &mut pricing,
        "meta.llama3-1-405b-instruct-v1:0",
        0.00532,
        0.016,
    );
    insert_pricing(&mut pricing, "meta.llama3-8b-instruct-v1:0", 0.0003, 0.0006);
    insert_pricing(
        &mut pricing,
        "meta.llama3-70b-instruct-v1:0",
        0.00265,
        0.0035,
    );
    insert_pricing(&mut pricing, "meta.llama2-13b-chat-v1", 0.00075, 0.001);
    insert_pricing(&mut pricing, "meta.llama2-70b-chat-v1", 0.00195, 0.00256);

    // --------------------------------------------------------------------
    // 2025-2026 Bedrock catalog expansions (latest model IDs)
    // --------------------------------------------------------------------
    macro_rules! insert_like {
        ($id:expr, $base_id:expr $(,)?) => {
            if let Some(base) = pricing.get($base_id).cloned() {
                pricing.insert($id, base);
            }
        };
    }

    // Anthropic aliases/new revisions
    insert_like!(
        "anthropic.claude-3-5-haiku-20241022-v1:0",
        "anthropic.claude-3-5-haiku-20241022",
    );
    insert_like!(
        "anthropic.claude-3-haiku-20240307-v1:0",
        "anthropic.claude-3-haiku-20240307",
    );
    insert_like!(
        "anthropic.claude-opus-4-5-20251101-v1:0",
        "anthropic.claude-opus-4-5-v1:0",
    );
    insert_like!(
        "anthropic.claude-sonnet-4-20250514-v1:0",
        "anthropic.claude-sonnet-4-v1:0",
    );
    insert_like!(
        "anthropic.claude-sonnet-4-5-20250929-v1:0",
        "anthropic.claude-sonnet-4-5-v1:0",
    );
    insert_like!(
        "anthropic.claude-opus-4-1-20250805-v1:0",
        "anthropic.claude-opus-4-5-v1:0",
    );
    insert_pricing(
        &mut pricing,
        "anthropic.claude-haiku-4-5-20251001-v1:0",
        0.001,
        0.005,
    );

    // Generic converse-compatible chat/text models
    let generic_converse_models = [
        "amazon.nova-2-lite-v1:0",
        "amazon.nova-2-sonic-v1:0",
        "amazon.nova-sonic-v1:0",
        "amazon.nova-premier-v1:0",
        "meta.llama3-3-70b-instruct-v1:0",
        "meta.llama4-maverick-17b-instruct-v1:0",
        "meta.llama4-scout-17b-instruct-v1:0",
        "deepseek.r1-v1:0",
        "deepseek.v3-v1:0",
        "google.gemma-3-12b-it",
        "google.gemma-3-27b-it",
        "google.gemma-3-4b-it",
        "minimax.minimax-m2",
        "minimax.minimax-m2.1",
        "mistral.magistral-small-2509",
        "mistral.ministral-3-14b-instruct",
        "mistral.ministral-3-3b-instruct",
        "mistral.ministral-3-8b-instruct",
        "mistral.mistral-large-3-675b-instruct",
        "mistral.pixtral-large-2502-v1:0",
        "mistral.voxtral-mini-3b-2507",
        "mistral.voxtral-small-24b-2507",
        "moonshot.kimi-k2-thinking",
        "moonshotai.kimi-k2.5",
        "nvidia.nemotron-nano-12b-v2",
        "nvidia.nemotron-nano-9b-v2",
        "openai.gpt-oss-120b-1:0",
        "openai.gpt-oss-20b-1:0",
        "openai.gpt-oss-safeguard-120b",
        "openai.gpt-oss-safeguard-20b",
        "qwen.qwen3-235b-a22b-2507-v1:0",
        "qwen.qwen3-32b-v1:0",
        "qwen.qwen3-coder-30b-a3b-v1:0",
        "qwen.qwen3-coder-480b-a35b-v1:0",
        "qwen.qwen3-next-80b-a3b",
        "qwen.qwen3-vl-235b-a22b",
        "writer.palmyra-x4-v1:0",
        "writer.palmyra-x5-v1:0",
    ];
    for model_id in generic_converse_models {
        insert_pricing(&mut pricing, model_id, 0.0008, 0.0032);
    }

    // Embedding/rerank catalog
    let embedding_models = [
        "amazon.nova-2-multimodal-embeddings-v1:0",
        "amazon.rerank-v1:0",
        "amazon.titan-embed-g1-text-02",
        "amazon.titan-embed-image-v1",
        "amazon.titan-embed-text-v2:0",
        "cohere.embed-english-v3",
        "cohere.embed-multilingual-v3",
        "cohere.embed-v4:0",
        "cohere.rerank-v3-5:0",
        "twelvelabs.marengo-embed-2-7-v1:0",
        "twelvelabs.marengo-embed-3-0-v1:0",
        "twelvelabs.pegasus-1-2-v1:0",
    ];
    for model_id in embedding_models {
        insert_pricing(&mut pricing, model_id, 0.0001, 0.0);
    }

    // Image/video catalog
    let titan_image_models = [
        "amazon.nova-canvas-v1:0",
        "amazon.nova-reel-v1:0",
        "amazon.nova-reel-v1:1",
        "amazon.titan-image-generator-v2:0",
        "luma.ray-v2:0",
    ];
    for model_id in titan_image_models {
        insert_pricing(&mut pricing, model_id, 0.001, 0.0);
    }

    let stability_models = [
        "stability.sd3-5-large-v1:0",
        "stability.stable-conservative-upscale-v1:0",
        "stability.stable-creative-upscale-v1:0",
        "stability.stable-fast-upscale-v1:0",
        "stability.stable-image-control-sketch-v1:0",
        "stability.stable-image-control-structure-v1:0",
        "stability.stable-image-core-v1:1",
        "stability.stable-image-erase-object-v1:0",
        "stability.stable-image-inpaint-v1:0",
        "stability.stable-image-remove-background-v1:0",
        "stability.stable-image-search-recolor-v1:0",
        "stability.stable-image-search-replace-v1:0",
        "stability.stable-image-style-guide-v1:0",
        "stability.stable-image-ultra-v1:1",
        "stability.stable-outpaint-v1:0",
        "stability.stable-style-transfer-v1:0",
    ];
    for model_id in stability_models {
        insert_pricing(&mut pricing, model_id, 0.002, 0.0);
    }

    insert_like!("amazon.titan-tg1-large", "amazon.titan-text-express-v1");

    pricing
});

/// Cost calculator for Bedrock models
pub struct CostCalculator;

impl CostCalculator {
    /// Calculate cost for a specific model and token usage
    pub fn calculate_cost(model_id: &str, input_tokens: u32, output_tokens: u32) -> Option<f64> {
        MODEL_PRICING.get(model_id).map(|pricing| {
            let input_cost = (input_tokens as f64 / 1000.0) * pricing.input_cost_per_1k_tokens;
            let output_cost = (output_tokens as f64 / 1000.0) * pricing.output_cost_per_1k_tokens;
            input_cost + output_cost
        })
    }

    /// Get pricing information for a model
    pub fn get_model_pricing(model_id: &str) -> Option<&'static ModelPricing> {
        MODEL_PRICING.get(model_id)
    }

    /// Get pricing information in the shared core cost model shape.
    pub fn get_core_model_pricing(model_id: &str) -> Option<ModelPricing> {
        MODEL_PRICING.get(model_id).cloned()
    }

    /// Get all available models with pricing
    pub fn get_all_models() -> Vec<&'static str> {
        MODEL_PRICING.keys().copied().collect()
    }

    /// Calculate cost with breakdown
    pub fn calculate_detailed_cost(
        model_id: &str,
        input_tokens: u32,
        output_tokens: u32,
    ) -> Option<CostBreakdown> {
        MODEL_PRICING.get(model_id).map(|pricing| {
            let input_cost = (input_tokens as f64 / 1000.0) * pricing.input_cost_per_1k_tokens;
            let output_cost = (output_tokens as f64 / 1000.0) * pricing.output_cost_per_1k_tokens;

            CostBreakdown {
                input_tokens,
                output_tokens,
                input_cost,
                output_cost,
                total_cost: input_cost + output_cost,
                currency: currency(pricing),
            }
        })
    }
}

/// Detailed cost breakdown
#[derive(Debug, Clone)]
pub struct CostBreakdown {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub input_cost: f64,
    pub output_cost: f64,
    pub total_cost: f64,
    pub currency: &'static str,
}

#[cfg(test)]
#[path = "cost_tests.rs"]
mod tests;
