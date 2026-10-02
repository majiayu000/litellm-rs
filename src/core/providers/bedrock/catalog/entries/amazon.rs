//! Amazon Titan + Nova family seeds (the text + embeddings models that ship
//! per-token pricing). Image / video Nova variants live in `media.rs`.

use super::super::super::model_config::{BedrockApiType, BedrockModelFamily};
use super::super::{
    BedrockCatalogEntry, BedrockPricing, BedrockVendor, EndpointSupport, ModelCapabilities,
    ModelLifecycle, ModelLimits, SourceMetadata,
};
use super::builder::{NO_PROFILES, US_GLOBAL, alias_entry, entry};

pub(super) fn seed(out: &mut Vec<BedrockCatalogEntry>) {
    // Titan text generation.
    let titan_text: &[(&str, &str, u32, u32, f64, f64)] = &[
        (
            "amazon.titan-text-express-v1",
            "Titan Text Express v1",
            8000,
            8000,
            0.0002,
            0.0006,
        ),
        (
            "amazon.titan-text-lite-v1",
            "Titan Text Lite v1",
            4000,
            4000,
            0.00015,
            0.0002,
        ),
        (
            "amazon.titan-text-premier-v1:0",
            "Titan Text Premier v1",
            32_000,
            32_000,
            0.0005,
            0.0015,
        ),
    ];

    let mut titan_express: Option<BedrockCatalogEntry> = None;
    for (id, name, ctx, max_out, input, output) in titan_text {
        let titan_entry = entry(
            id,
            name,
            BedrockVendor::Amazon,
            BedrockModelFamily::TitanText,
            BedrockApiType::Invoke,
            ModelLifecycle::Live,
            EndpointSupport::INVOKE,
            NO_PROFILES,
            ModelLimits {
                max_context_length: *ctx,
                max_output_length: Some(*max_out),
            },
            ModelCapabilities::CHAT_TEXT_ONLY,
            Some(BedrockPricing::per_1k(*input, *output)),
            None,
            SourceMetadata::AWS_BEDROCK_PRICING,
        );
        if *id == "amazon.titan-text-express-v1" {
            titan_express = Some(titan_entry.clone());
        }
        out.push(titan_entry);
    }

    if let Some(base) = titan_express {
        out.push(alias_entry(
            "amazon.titan-tg1-large",
            "amazon.titan-text-express-v1",
            base,
        ));
    }

    // Titan Embed Text v1: $0.0001 per 1k input tokens.
    out.push(entry(
        "amazon.titan-embed-text-v1",
        "Titan Embed Text v1",
        BedrockVendor::Amazon,
        BedrockModelFamily::TitanEmbedding,
        BedrockApiType::Invoke,
        ModelLifecycle::Live,
        EndpointSupport::INVOKE_NON_STREAMING,
        NO_PROFILES,
        ModelLimits {
            max_context_length: 8000,
            max_output_length: None,
        },
        ModelCapabilities::EMBEDDINGS,
        Some(BedrockPricing::per_1k(0.0001, 0.0)),
        None,
        SourceMetadata::AWS_BEDROCK_PRICING,
    ));

    // Nova v1 family — Converse API, multimodal.
    let nova_v1: &[(&str, &str, u32, u32, f64, f64)] = &[
        (
            "amazon.nova-micro-v1:0",
            "Nova Micro v1",
            128_000,
            4096,
            0.000035,
            0.00014,
        ),
        (
            "amazon.nova-lite-v1:0",
            "Nova Lite v1",
            300_000,
            4096,
            0.00006,
            0.00024,
        ),
        (
            "amazon.nova-pro-v1:0",
            "Nova Pro v1",
            300_000,
            4096,
            0.0008,
            0.0032,
        ),
    ];
    for (id, name, ctx, max_out, input, output) in nova_v1 {
        out.push(entry(
            id,
            name,
            BedrockVendor::Amazon,
            BedrockModelFamily::Nova,
            BedrockApiType::Converse,
            ModelLifecycle::Live,
            EndpointSupport::CONVERSE,
            US_GLOBAL,
            ModelLimits {
                max_context_length: *ctx,
                max_output_length: Some(*max_out),
            },
            ModelCapabilities::CHAT_MULTIMODAL,
            Some(BedrockPricing::per_1k(*input, *output)),
            None,
            SourceMetadata::AWS_BEDROCK_PRICING,
        ));
    }
}
