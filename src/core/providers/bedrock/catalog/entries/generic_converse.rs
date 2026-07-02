//! 2025-2026 generic converse-compatible chat catalog.
//!
//! These are the model IDs added by the legacy `generic_converse_models` list
//! in `model_config.rs` / `utils/cost.rs`. They share a default shape:
//! converse API, 300k context, 8k max output, $0.0008 / $0.0032 per 1k tokens
//! and full chat-multimodal capabilities. Family is `Nova` because the legacy
//! map used that as the catch-all variant.

use super::super::super::model_config::{BedrockApiType, BedrockModelFamily};
use super::super::{
    BedrockCatalogEntry, BedrockPricing, BedrockVendor, EndpointSupport, ModelCapabilities,
    ModelLifecycle, ModelLimits, SourceMetadata,
};
use super::builder::{US_GLOBAL, entry};

pub(super) fn seed(out: &mut Vec<BedrockCatalogEntry>) {
    let generic: &[(&str, &str, BedrockVendor)] = &[
        (
            "amazon.nova-2-lite-v1:0",
            "Nova 2 Lite",
            BedrockVendor::Amazon,
        ),
        (
            "amazon.nova-2-sonic-v1:0",
            "Nova 2 Sonic",
            BedrockVendor::Amazon,
        ),
        (
            "amazon.nova-sonic-v1:0",
            "Nova Sonic",
            BedrockVendor::Amazon,
        ),
        (
            "amazon.nova-premier-v1:0",
            "Nova Premier",
            BedrockVendor::Amazon,
        ),
        (
            "meta.llama3-3-70b-instruct-v1:0",
            "Llama 3.3 70B Instruct",
            BedrockVendor::Meta,
        ),
        (
            "meta.llama4-maverick-17b-instruct-v1:0",
            "Llama 4 Maverick 17B",
            BedrockVendor::Meta,
        ),
        (
            "meta.llama4-scout-17b-instruct-v1:0",
            "Llama 4 Scout 17B",
            BedrockVendor::Meta,
        ),
        ("deepseek.r1-v1:0", "DeepSeek R1", BedrockVendor::DeepSeek),
        ("deepseek.v3-v1:0", "DeepSeek V3", BedrockVendor::DeepSeek),
        (
            "google.gemma-3-12b-it",
            "Gemma 3 12B IT",
            BedrockVendor::Google,
        ),
        (
            "google.gemma-3-27b-it",
            "Gemma 3 27B IT",
            BedrockVendor::Google,
        ),
        (
            "google.gemma-3-4b-it",
            "Gemma 3 4B IT",
            BedrockVendor::Google,
        ),
        ("minimax.minimax-m2", "MiniMax M2", BedrockVendor::MiniMax),
        (
            "minimax.minimax-m2.1",
            "MiniMax M2.1",
            BedrockVendor::MiniMax,
        ),
        (
            "mistral.magistral-small-2509",
            "Magistral Small 2509",
            BedrockVendor::Mistral,
        ),
        (
            "mistral.ministral-3-14b-instruct",
            "Ministral 3 14B Instruct",
            BedrockVendor::Mistral,
        ),
        (
            "mistral.ministral-3-3b-instruct",
            "Ministral 3 3B Instruct",
            BedrockVendor::Mistral,
        ),
        (
            "mistral.ministral-3-8b-instruct",
            "Ministral 3 8B Instruct",
            BedrockVendor::Mistral,
        ),
        (
            "mistral.mistral-large-3-675b-instruct",
            "Mistral Large 3 675B Instruct",
            BedrockVendor::Mistral,
        ),
        (
            "mistral.pixtral-large-2502-v1:0",
            "Pixtral Large 2502",
            BedrockVendor::Mistral,
        ),
        (
            "mistral.voxtral-mini-3b-2507",
            "Voxtral Mini 3B 2507",
            BedrockVendor::Mistral,
        ),
        (
            "mistral.voxtral-small-24b-2507",
            "Voxtral Small 24B 2507",
            BedrockVendor::Mistral,
        ),
        (
            "nvidia.nemotron-nano-12b-v2",
            "Nemotron Nano 12B v2",
            BedrockVendor::Nvidia,
        ),
        (
            "nvidia.nemotron-nano-9b-v2",
            "Nemotron Nano 9B v2",
            BedrockVendor::Nvidia,
        ),
        (
            "openai.gpt-oss-120b-1:0",
            "GPT-OSS 120B",
            BedrockVendor::OpenAI,
        ),
        (
            "openai.gpt-oss-20b-1:0",
            "GPT-OSS 20B",
            BedrockVendor::OpenAI,
        ),
        (
            "openai.gpt-oss-safeguard-120b",
            "GPT-OSS Safeguard 120B",
            BedrockVendor::OpenAI,
        ),
        (
            "openai.gpt-oss-safeguard-20b",
            "GPT-OSS Safeguard 20B",
            BedrockVendor::OpenAI,
        ),
        (
            "qwen.qwen3-235b-a22b-2507-v1:0",
            "Qwen3 235B A22B 2507",
            BedrockVendor::Qwen,
        ),
        ("qwen.qwen3-32b-v1:0", "Qwen3 32B", BedrockVendor::Qwen),
        (
            "qwen.qwen3-coder-30b-a3b-v1:0",
            "Qwen3 Coder 30B A3B",
            BedrockVendor::Qwen,
        ),
        (
            "qwen.qwen3-coder-480b-a35b-v1:0",
            "Qwen3 Coder 480B A35B",
            BedrockVendor::Qwen,
        ),
        (
            "qwen.qwen3-next-80b-a3b",
            "Qwen3 Next 80B A3B",
            BedrockVendor::Qwen,
        ),
        (
            "qwen.qwen3-vl-235b-a22b",
            "Qwen3 VL 235B A22B",
            BedrockVendor::Qwen,
        ),
        (
            "writer.palmyra-x4-v1:0",
            "Palmyra X4",
            BedrockVendor::Writer,
        ),
        (
            "writer.palmyra-x5-v1:0",
            "Palmyra X5",
            BedrockVendor::Writer,
        ),
    ];

    for (id, name, vendor) in generic {
        out.push(entry(
            id,
            name,
            *vendor,
            // The former hard-coded MODEL_CONFIGS map used Nova as the
            // catch-all for these generic converse models. Preserve that
            // mapping so the public projection stays bit-identical.
            BedrockModelFamily::Nova,
            BedrockApiType::Converse,
            ModelLifecycle::Live,
            EndpointSupport::CONVERSE,
            US_GLOBAL,
            ModelLimits {
                max_context_length: 300_000,
                max_output_length: Some(8192),
            },
            ModelCapabilities::CHAT_MULTIMODAL,
            Some(BedrockPricing::per_1k(0.0008, 0.0032)),
            None,
            SourceMetadata::AWS_BEDROCK_PRICING,
        ));
    }

    let moonshot: &[(&str, &str, ModelCapabilities)] = &[
        (
            "moonshot.kimi-k2-thinking",
            "Kimi K2 Thinking",
            ModelCapabilities::CHAT_TOOLS_TEXT,
        ),
        (
            "moonshotai.kimi-k2.5",
            "Kimi K2.5",
            ModelCapabilities::CHAT_MULTIMODAL,
        ),
    ];
    for (id, name, capabilities) in moonshot {
        out.push(entry(
            id,
            name,
            BedrockVendor::Moonshot,
            // The former hard-coded MODEL_CONFIGS map used Nova as the
            // catch-all for these generic converse models. Preserve that
            // mapping so the public projection stays bit-identical.
            BedrockModelFamily::Nova,
            BedrockApiType::Converse,
            ModelLifecycle::Live,
            EndpointSupport::CONVERSE,
            US_GLOBAL,
            ModelLimits {
                max_context_length: 256_000,
                max_output_length: Some(16_000),
            },
            *capabilities,
            Some(BedrockPricing::per_1k(0.0008, 0.0032)),
            None,
            SourceMetadata::AWS_BEDROCK_PRICING,
        ));
    }
}
