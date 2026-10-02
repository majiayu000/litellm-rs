//! Embedding, rerank, image, and video generation catalog seeds.

use super::super::super::model_config::{BedrockApiType, BedrockModelFamily};
use super::super::{
    BedrockCatalogEntry, BedrockPricing, BedrockVendor, EndpointSupport, ModelCapabilities,
    ModelLifecycle, ModelLimits, SourceMetadata,
};
use super::builder::{NO_PROFILES, entry};

pub(super) fn seed(out: &mut Vec<BedrockCatalogEntry>) {
    seed_embeddings(out);
    seed_image_video(out);
}

fn seed_embeddings(out: &mut Vec<BedrockCatalogEntry>) {
    let embeddings: &[(&str, &str, BedrockVendor)] = &[
        (
            "amazon.nova-2-multimodal-embeddings-v1:0",
            "Nova 2 Multimodal Embeddings",
            BedrockVendor::Amazon,
        ),
        (
            "amazon.rerank-v1:0",
            "Amazon Rerank v1",
            BedrockVendor::Amazon,
        ),
        (
            "amazon.titan-embed-g1-text-02",
            "Titan Embed G1 Text 02",
            BedrockVendor::Amazon,
        ),
        (
            "amazon.titan-embed-image-v1",
            "Titan Embed Image v1",
            BedrockVendor::Amazon,
        ),
        (
            "amazon.titan-embed-text-v2:0",
            "Titan Embed Text v2",
            BedrockVendor::Amazon,
        ),
        (
            "cohere.embed-english-v3",
            "Cohere Embed English v3",
            BedrockVendor::Cohere,
        ),
        (
            "cohere.embed-multilingual-v3",
            "Cohere Embed Multilingual v3",
            BedrockVendor::Cohere,
        ),
        (
            "cohere.embed-v4:0",
            "Cohere Embed v4",
            BedrockVendor::Cohere,
        ),
        (
            "cohere.rerank-v3-5:0",
            "Cohere Rerank v3.5",
            BedrockVendor::Cohere,
        ),
        (
            "twelvelabs.marengo-embed-2-7-v1:0",
            "TwelveLabs Marengo Embed 2.7",
            BedrockVendor::TwelveLabs,
        ),
        (
            "twelvelabs.marengo-embed-3-0-v1:0",
            "TwelveLabs Marengo Embed 3.0",
            BedrockVendor::TwelveLabs,
        ),
        (
            "twelvelabs.pegasus-1-2-v1:0",
            "TwelveLabs Pegasus 1.2",
            BedrockVendor::TwelveLabs,
        ),
    ];
    for (id, name, vendor) in embeddings {
        out.push(entry(
            id,
            name,
            *vendor,
            BedrockModelFamily::TitanEmbedding,
            BedrockApiType::Invoke,
            ModelLifecycle::Live,
            EndpointSupport::INVOKE_NON_STREAMING,
            NO_PROFILES,
            ModelLimits {
                max_context_length: 300_000,
                max_output_length: None,
            },
            ModelCapabilities::EMBEDDINGS_MULTIMODAL,
            Some(BedrockPricing::per_1k(0.0001, 0.0)),
            None,
            SourceMetadata::AWS_BEDROCK_PRICING,
        ));
    }
}

fn seed_image_video(out: &mut Vec<BedrockCatalogEntry>) {
    let titan_image: &[(&str, &str, BedrockVendor)] = &[
        (
            "amazon.nova-canvas-v1:0",
            "Nova Canvas v1",
            BedrockVendor::Amazon,
        ),
        (
            "amazon.nova-reel-v1:0",
            "Nova Reel v1",
            BedrockVendor::Amazon,
        ),
        (
            "amazon.nova-reel-v1:1",
            "Nova Reel v1.1",
            BedrockVendor::Amazon,
        ),
        (
            "amazon.titan-image-generator-v2:0",
            "Titan Image Generator v2",
            BedrockVendor::Amazon,
        ),
        ("luma.ray-v2:0", "Luma Ray v2", BedrockVendor::Luma),
    ];
    for (id, name, vendor) in titan_image {
        out.push(entry(
            id,
            name,
            *vendor,
            BedrockModelFamily::TitanImage,
            BedrockApiType::Invoke,
            // AWS model-lifecycle-legacy.html: all Nova Canvas/Reel variants
            // reached EOL on 2026-09-30; keep historical pricing only.
            if matches!(
                *id,
                "amazon.nova-canvas-v1:0" | "amazon.nova-reel-v1:0" | "amazon.nova-reel-v1:1"
            ) {
                ModelLifecycle::Retired {
                    retirement_date: "2026-09-30",
                }
            } else {
                ModelLifecycle::Live
            },
            EndpointSupport::INVOKE_NON_STREAMING,
            NO_PROFILES,
            ModelLimits {
                max_context_length: 32_768,
                max_output_length: None,
            },
            ModelCapabilities::IMAGE_GENERATION,
            Some(BedrockPricing::per_1k(0.001, 0.0)),
            None,
            SourceMetadata::AWS_BEDROCK_PRICING,
        ));
    }

    let stability: &[(&str, &str)] = &[
        ("stability.sd3-5-large-v1:0", "Stable Diffusion 3.5 Large"),
        (
            "stability.stable-conservative-upscale-v1:0",
            "Stable Conservative Upscale",
        ),
        (
            "stability.stable-creative-upscale-v1:0",
            "Stable Creative Upscale",
        ),
        ("stability.stable-fast-upscale-v1:0", "Stable Fast Upscale"),
        (
            "stability.stable-image-control-sketch-v1:0",
            "Stable Image Control Sketch",
        ),
        (
            "stability.stable-image-control-structure-v1:0",
            "Stable Image Control Structure",
        ),
        ("stability.stable-image-core-v1:1", "Stable Image Core v1.1"),
        (
            "stability.stable-image-erase-object-v1:0",
            "Stable Image Erase Object",
        ),
        (
            "stability.stable-image-inpaint-v1:0",
            "Stable Image Inpaint",
        ),
        (
            "stability.stable-image-remove-background-v1:0",
            "Stable Image Remove Background",
        ),
        (
            "stability.stable-image-search-recolor-v1:0",
            "Stable Image Search Recolor",
        ),
        (
            "stability.stable-image-search-replace-v1:0",
            "Stable Image Search Replace",
        ),
        (
            "stability.stable-image-style-guide-v1:0",
            "Stable Image Style Guide",
        ),
        (
            "stability.stable-image-ultra-v1:1",
            "Stable Image Ultra v1.1",
        ),
        ("stability.stable-outpaint-v1:0", "Stable Outpaint"),
        (
            "stability.stable-style-transfer-v1:0",
            "Stable Style Transfer",
        ),
    ];
    for (id, name) in stability {
        out.push(entry(
            id,
            name,
            BedrockVendor::Stability,
            BedrockModelFamily::StabilityAI,
            BedrockApiType::Invoke,
            ModelLifecycle::Live,
            EndpointSupport::INVOKE_NON_STREAMING,
            NO_PROFILES,
            ModelLimits {
                max_context_length: 8192,
                max_output_length: None,
            },
            ModelCapabilities::IMAGE_GENERATION,
            Some(BedrockPricing::per_1k(0.002, 0.0)),
            None,
            SourceMetadata::AWS_BEDROCK_PRICING,
        ));
    }
}
