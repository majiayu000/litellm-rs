//! Cohere + AI21 family seeds (chat / command models only). Cohere embed and
//! rerank entries live in `media.rs` together with other embeddings.

use super::super::super::model_config::{BedrockApiType, BedrockModelFamily};
use super::super::{
    BedrockCatalogEntry, BedrockPricing, BedrockVendor, EndpointSupport, ModelCapabilities,
    ModelLifecycle, ModelLimits, SourceMetadata,
};
use super::builder::{NO_PROFILES, entry};

pub(super) fn seed(out: &mut Vec<BedrockCatalogEntry>) {
    let cohere: &[(&str, &str, u32, f64, f64)] = &[
        (
            "cohere.command-r-plus-v1:0",
            "Command R+",
            128_000,
            0.003,
            0.015,
        ),
        (
            "cohere.command-r-v1:0",
            "Command R",
            128_000,
            0.0005,
            0.0015,
        ),
        (
            "cohere.command-text-v14",
            "Command Text v14",
            4096,
            0.0015,
            0.002,
        ),
        (
            "cohere.command-light-text-v14",
            "Command Light Text v14",
            4096,
            0.0003,
            0.0006,
        ),
    ];
    for (id, name, ctx, input, output) in cohere {
        out.push(entry(
            id,
            name,
            BedrockVendor::Cohere,
            BedrockModelFamily::Cohere,
            BedrockApiType::Invoke,
            // https://docs.aws.amazon.com/bedrock/latest/userguide/model-lifecycle-legacy.html
            if matches!(*id, "cohere.command-r-v1:0" | "cohere.command-r-plus-v1:0") {
                ModelLifecycle::Retired {
                    retirement_date: "2026-08-19",
                }
            } else {
                ModelLifecycle::Live
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

    let ai21: &[(&str, &str, u32, f64, f64)] = &[
        (
            "ai21.jamba-1-5-large-v1:0",
            "Jamba 1.5 Large",
            256_000,
            0.002,
            0.008,
        ),
        (
            "ai21.jamba-1-5-mini-v1:0",
            "Jamba 1.5 Mini",
            256_000,
            0.0002,
            0.0004,
        ),
        (
            "ai21.jamba-instruct-v1:0",
            "Jamba Instruct",
            70_000,
            0.0005,
            0.0007,
        ),
    ];
    for (id, name, ctx, input, output) in ai21 {
        out.push(entry(
            id,
            name,
            BedrockVendor::AI21,
            BedrockModelFamily::AI21,
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
