//! Meta Llama + Mistral classic family seeds. 2025-2026 expansion variants
//! that route through the generic converse catalog live in `generic_converse.rs`.

use super::super::super::model_config::{BedrockApiType, BedrockModelFamily};
use super::super::{
    BedrockCatalogEntry, BedrockPricing, BedrockVendor, EndpointSupport, ModelCapabilities,
    ModelLifecycle, ModelLimits, SourceMetadata,
};
use super::builder::{NO_PROFILES, US_GLOBAL, entry};

pub(super) fn seed(out: &mut Vec<BedrockCatalogEntry>) {
    // Llama 3 / 3.1 / 3.2 — converse API.
    let llama_converse: &[(&str, &str, u32, f64, f64)] = &[
        (
            "meta.llama3-2-1b-instruct-v1:0",
            "Llama 3.2 1B Instruct",
            131_072,
            0.00001,
            0.00001,
        ),
        (
            "meta.llama3-2-3b-instruct-v1:0",
            "Llama 3.2 3B Instruct",
            131_072,
            0.000015,
            0.000015,
        ),
        (
            "meta.llama3-2-11b-instruct-v1:0",
            "Llama 3.2 11B Instruct",
            131_072,
            0.000032,
            0.000032,
        ),
        (
            "meta.llama3-2-90b-instruct-v1:0",
            "Llama 3.2 90B Instruct",
            131_072,
            0.00072,
            0.00072,
        ),
        (
            "meta.llama3-1-8b-instruct-v1:0",
            "Llama 3.1 8B Instruct",
            131_072,
            0.00022,
            0.00022,
        ),
        (
            "meta.llama3-1-70b-instruct-v1:0",
            "Llama 3.1 70B Instruct",
            131_072,
            0.00099,
            0.00099,
        ),
        (
            "meta.llama3-1-405b-instruct-v1:0",
            "Llama 3.1 405B Instruct",
            131_072,
            0.00532,
            0.016,
        ),
        (
            "meta.llama3-8b-instruct-v1:0",
            "Llama 3 8B Instruct",
            8192,
            0.0003,
            0.0006,
        ),
        (
            "meta.llama3-70b-instruct-v1:0",
            "Llama 3 70B Instruct",
            8192,
            0.00265,
            0.0035,
        ),
    ];
    for (id, name, ctx, input, output) in llama_converse {
        out.push(entry(
            id,
            name,
            BedrockVendor::Meta,
            BedrockModelFamily::Llama,
            BedrockApiType::Converse,
            ModelLifecycle::Live,
            EndpointSupport::CONVERSE,
            US_GLOBAL,
            ModelLimits {
                max_context_length: *ctx,
                max_output_length: Some(4096),
            },
            ModelCapabilities::CHAT_TEXT_ONLY,
            Some(BedrockPricing::per_1k(*input, *output)),
            None,
            SourceMetadata::AWS_BEDROCK_PRICING,
        ));
    }

    // Llama 2 — invoke API, deprecated.
    let llama_legacy: &[(&str, &str, u32, f64, f64)] = &[
        (
            "meta.llama2-13b-chat-v1",
            "Llama 2 13B Chat",
            4096,
            0.00075,
            0.001,
        ),
        (
            "meta.llama2-70b-chat-v1",
            "Llama 2 70B Chat",
            4096,
            0.00195,
            0.00256,
        ),
    ];
    for (id, name, ctx, input, output) in llama_legacy {
        out.push(entry(
            id,
            name,
            BedrockVendor::Meta,
            BedrockModelFamily::Llama,
            BedrockApiType::Invoke,
            ModelLifecycle::Retired {
                retirement_date: "2024-12-12",
            },
            EndpointSupport::INVOKE,
            NO_PROFILES,
            ModelLimits {
                max_context_length: *ctx,
                max_output_length: Some(4096),
            },
            ModelCapabilities::CHAT_TEXT_ONLY,
            Some(BedrockPricing::per_1k(*input, *output)),
            None,
            SourceMetadata::AWS_BEDROCK_PRICING,
        ));
    }

    // Classic Mistral / Mixtral.
    let mistral: &[(&str, &str, u32, f64, f64)] = &[
        (
            "mistral.mistral-7b-instruct-v0:2",
            "Mistral 7B Instruct",
            32_000,
            0.00015,
            0.0002,
        ),
        (
            "mistral.mixtral-8x7b-instruct-v0:1",
            "Mixtral 8x7B Instruct",
            32_000,
            0.00045,
            0.0007,
        ),
        (
            "mistral.mistral-large-2402-v1:0",
            "Mistral Large 2402",
            32_000,
            0.004,
            0.012,
        ),
        (
            "mistral.mistral-large-2407-v1:0",
            "Mistral Large 2407",
            128_000,
            0.002,
            0.006,
        ),
        (
            "mistral.mistral-small-2402-v1:0",
            "Mistral Small 2402",
            32_000,
            0.001,
            0.003,
        ),
    ];
    for (id, name, ctx, input, output) in mistral {
        out.push(entry(
            id,
            name,
            BedrockVendor::Mistral,
            BedrockModelFamily::Mistral,
            BedrockApiType::Invoke,
            ModelLifecycle::Live,
            EndpointSupport::INVOKE,
            NO_PROFILES,
            ModelLimits {
                max_context_length: *ctx,
                max_output_length: Some(4096),
            },
            ModelCapabilities::CHAT_TEXT_ONLY,
            Some(BedrockPricing::per_1k(*input, *output)),
            None,
            SourceMetadata::AWS_BEDROCK_PRICING,
        ));
    }
}
