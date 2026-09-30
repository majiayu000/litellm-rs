//! Anthropic Claude family seeds.

use super::super::super::model_config::{BedrockApiType, BedrockModelFamily};
use super::super::{
    BedrockCatalogEntry, BedrockPricing, BedrockVendor, EndpointSupport, InferenceProfileScope,
    ModelCapabilities, ModelLifecycle, ModelLimits, SourceMetadata,
};
use super::builder::{COMMON_GEO, NO_PROFILES, entry};

pub(super) fn seed(out: &mut Vec<BedrockCatalogEntry>) {
    // AWS model cards reviewed 2026-10-01; inference scopes differ per model.
    for (id, name, input, output, profiles) in [
        (
            "anthropic.claude-fable-5-1",
            "Claude Fable 5.1",
            0.010,
            0.050,
            &[
                InferenceProfileScope::Global,
                InferenceProfileScope::UnitedStates,
            ][..],
        ),
        (
            "anthropic.claude-opus-5-5",
            "Claude Opus 5.5",
            0.004,
            0.020,
            &[
                InferenceProfileScope::Global,
                InferenceProfileScope::UnitedStates,
                InferenceProfileScope::Europe,
                InferenceProfileScope::Australia,
                InferenceProfileScope::Japan,
            ][..],
        ),
        (
            "anthropic.claude-sonnet-5-5",
            "Claude Sonnet 5.5",
            0.002,
            0.010,
            &[InferenceProfileScope::Global][..],
        ),
    ] {
        out.push(entry(
            id,
            name,
            BedrockVendor::Anthropic,
            BedrockModelFamily::Claude,
            BedrockApiType::Converse,
            ModelLifecycle::Live,
            EndpointSupport::CONVERSE,
            profiles,
            ModelLimits {
                max_context_length: 1_000_000,
                max_output_length: Some(128_000),
            },
            ModelCapabilities {
                thinking: true,
                ..ModelCapabilities::CHAT_MULTIMODAL
            },
            Some(BedrockPricing::per_1k(input, output)),
            None,
            SourceMetadata {
                url: "https://docs.aws.amazon.com/bedrock/latest/userguide/models-supported.html",
                verified_date: "2026-10-01",
            },
        ));
    }
    // Modern Claude 3 / 3.5 / 4 / 4.5 / 4.6 — converse API, multimodal.
    let modern: &[(&str, &str, u32, f64, f64)] = &[
        (
            "anthropic.claude-opus-4-6-v1:0",
            "Claude Opus 4.6",
            32_000,
            0.005,
            0.025,
        ),
        (
            "anthropic.claude-opus-4-6-v1",
            "Claude Opus 4.6",
            32_000,
            0.005,
            0.025,
        ),
        (
            "anthropic.claude-opus-4-6",
            "Claude Opus 4.6",
            32_000,
            0.005,
            0.025,
        ),
        (
            "anthropic.claude-opus-4-5-v1:0",
            "Claude Opus 4.5",
            32_000,
            0.005,
            0.025,
        ),
        (
            "anthropic.claude-opus-4-5",
            "Claude Opus 4.5",
            32_000,
            0.005,
            0.025,
        ),
        (
            "anthropic.claude-sonnet-4-5-v1:0",
            "Claude Sonnet 4.5",
            16_000,
            0.003,
            0.015,
        ),
        (
            "anthropic.claude-sonnet-4-5",
            "Claude Sonnet 4.5",
            16_000,
            0.003,
            0.015,
        ),
        (
            "anthropic.claude-sonnet-4-v1:0",
            "Claude Sonnet 4",
            16_000,
            0.003,
            0.015,
        ),
        (
            "anthropic.claude-sonnet-4",
            "Claude Sonnet 4",
            16_000,
            0.003,
            0.015,
        ),
        (
            "anthropic.claude-3-opus-20240229",
            "Claude 3 Opus",
            4096,
            0.015,
            0.075,
        ),
        (
            "anthropic.claude-3-sonnet-20240229",
            "Claude 3 Sonnet",
            4096,
            0.003,
            0.015,
        ),
        (
            "anthropic.claude-3-haiku-20240307",
            "Claude 3 Haiku",
            4096,
            0.00025,
            0.00125,
        ),
        (
            "anthropic.claude-3-5-sonnet-20241022",
            "Claude 3.5 Sonnet",
            4096,
            0.003,
            0.015,
        ),
        (
            "anthropic.claude-3-5-sonnet-20241022-v2:0",
            "Claude 3.5 Sonnet v2",
            4096,
            0.003,
            0.015,
        ),
        (
            "anthropic.claude-3-5-haiku-20241022",
            "Claude 3.5 Haiku",
            4096,
            0.001,
            0.005,
        ),
        // 2025-2026 catalog expansions (aliases / new revisions).
        (
            "anthropic.claude-3-5-haiku-20241022-v1:0",
            "Claude 3.5 Haiku",
            4096,
            0.001,
            0.005,
        ),
        (
            "anthropic.claude-3-haiku-20240307-v1:0",
            "Claude 3 Haiku",
            4096,
            0.00025,
            0.00125,
        ),
        (
            "anthropic.claude-opus-4-5-20251101-v1:0",
            "Claude Opus 4.5",
            32_000,
            0.005,
            0.025,
        ),
        (
            "anthropic.claude-sonnet-4-20250514-v1:0",
            "Claude Sonnet 4",
            16_000,
            0.003,
            0.015,
        ),
        (
            "anthropic.claude-sonnet-4-5-20250929-v1:0",
            "Claude Sonnet 4.5",
            16_000,
            0.003,
            0.015,
        ),
        (
            "anthropic.claude-opus-4-1-20250805-v1:0",
            "Claude Opus 4.1",
            32_000,
            0.005,
            0.025,
        ),
        (
            "anthropic.claude-haiku-4-5-20251001-v1:0",
            "Claude Haiku 4.5",
            8192,
            0.001,
            0.005,
        ),
    ];

    for (id, name, max_out, input, output) in modern {
        out.push(entry(
            id,
            name,
            BedrockVendor::Anthropic,
            BedrockModelFamily::Claude,
            BedrockApiType::Converse,
            ModelLifecycle::Live,
            EndpointSupport::CONVERSE,
            COMMON_GEO,
            ModelLimits {
                max_context_length: 200_000,
                max_output_length: Some(*max_out),
            },
            ModelCapabilities::CHAT_MULTIMODAL,
            Some(BedrockPricing::per_1k(*input, *output)),
            None,
            SourceMetadata::AWS_BEDROCK_PRICING,
        ));
    }

    // Legacy Claude v1/v2 — invoke API, text-only, deprecated.
    let legacy: &[(&str, &str, u32, u32, f64, f64)] = &[
        (
            "anthropic.claude-v2:1",
            "Claude v2.1",
            100_000,
            4096,
            0.008,
            0.024,
        ),
        (
            "anthropic.claude-v2",
            "Claude v2",
            100_000,
            4096,
            0.008,
            0.024,
        ),
        (
            "anthropic.claude-instant-v1",
            "Claude Instant v1",
            100_000,
            4096,
            0.00163,
            0.00551,
        ),
    ];
    for (id, name, ctx, max_out, input, output) in legacy {
        out.push(entry(
            id,
            name,
            BedrockVendor::Anthropic,
            BedrockModelFamily::Claude,
            BedrockApiType::Invoke,
            ModelLifecycle::Deprecated {
                deprecation_date: "2025-07-21",
            },
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
        ));
    }
}
