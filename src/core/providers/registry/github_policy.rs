//! GitHub Models catalog policy.
//!
//! Catalog route metadata, pricing and capabilities. The unused native module
//! was removed after its 0.7 deprecation deadline. Historical metadata here
//! remains subject to the separate model-catalog audit; removal does not
//! establish current supplier availability.

use std::sync::LazyLock;

use crate::core::types::model::{ModelInfo, ProviderCapability};

pub(crate) const GITHUB_CATALOG_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];
pub(crate) const GITHUB_SUPPORTS_STREAMING: bool = true;

pub(crate) struct GitHubCatalogModel {
    pub(crate) model_id: &'static str,
    pub(crate) display_name: &'static str,
    pub(crate) max_context_length: u32,
    pub(crate) max_output_length: u32,
    pub(crate) supports_tools: bool,
    pub(crate) supports_multimodal: bool,
    pub(crate) input_cost_per_million: f64,
    pub(crate) output_cost_per_million: f64,
}

pub(crate) static GITHUB_CATALOG_MODELS: &[GitHubCatalogModel] = &[
    // OpenAI Models
    GitHubCatalogModel {
        model_id: "gpt-4o",
        display_name: "GPT-4o",
        max_context_length: 128_000,
        max_output_length: 16_384,
        supports_tools: true,
        supports_multimodal: true,
        input_cost_per_million: 2.5,
        output_cost_per_million: 10.0,
    },
    GitHubCatalogModel {
        model_id: "gpt-4o-mini",
        display_name: "GPT-4o Mini",
        max_context_length: 128_000,
        max_output_length: 16_384,
        supports_tools: true,
        supports_multimodal: true,
        input_cost_per_million: 0.15,
        output_cost_per_million: 0.6,
    },
    GitHubCatalogModel {
        model_id: "o1-preview",
        display_name: "O1 Preview",
        max_context_length: 128_000,
        max_output_length: 32_768,
        supports_tools: false,
        supports_multimodal: false,
        input_cost_per_million: 15.0,
        output_cost_per_million: 60.0,
    },
    GitHubCatalogModel {
        model_id: "o1-mini",
        display_name: "O1 Mini",
        max_context_length: 128_000,
        max_output_length: 65_536,
        supports_tools: false,
        supports_multimodal: false,
        input_cost_per_million: 3.0,
        output_cost_per_million: 12.0,
    },
    // Meta Llama Models
    GitHubCatalogModel {
        model_id: "meta-llama-3.1-405b-instruct",
        display_name: "Meta Llama 3.1 405B Instruct",
        max_context_length: 128_000,
        max_output_length: 4_096,
        supports_tools: true,
        supports_multimodal: false,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
    GitHubCatalogModel {
        model_id: "meta-llama-3.1-70b-instruct",
        display_name: "Meta Llama 3.1 70B Instruct",
        max_context_length: 128_000,
        max_output_length: 4_096,
        supports_tools: true,
        supports_multimodal: false,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
    GitHubCatalogModel {
        model_id: "meta-llama-3.1-8b-instruct",
        display_name: "Meta Llama 3.1 8B Instruct",
        max_context_length: 128_000,
        max_output_length: 4_096,
        supports_tools: true,
        supports_multimodal: false,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
    // Mistral Models
    GitHubCatalogModel {
        model_id: "mistral-large-2407",
        display_name: "Mistral Large 2407",
        max_context_length: 128_000,
        max_output_length: 4_096,
        supports_tools: true,
        supports_multimodal: false,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
    GitHubCatalogModel {
        model_id: "mistral-small-2409",
        display_name: "Mistral Small 2409",
        max_context_length: 32_000,
        max_output_length: 4_096,
        supports_tools: true,
        supports_multimodal: false,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
    // Cohere Models
    GitHubCatalogModel {
        model_id: "cohere-command-r-plus",
        display_name: "Cohere Command R+",
        max_context_length: 128_000,
        max_output_length: 4_096,
        supports_tools: true,
        supports_multimodal: false,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
    GitHubCatalogModel {
        model_id: "cohere-command-r",
        display_name: "Cohere Command R",
        max_context_length: 128_000,
        max_output_length: 4_096,
        supports_tools: true,
        supports_multimodal: false,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
    // AI21 Models
    GitHubCatalogModel {
        model_id: "ai21-jamba-1.5-large",
        display_name: "AI21 Jamba 1.5 Large",
        max_context_length: 256_000,
        max_output_length: 4_096,
        supports_tools: false,
        supports_multimodal: false,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
    GitHubCatalogModel {
        model_id: "ai21-jamba-1.5-mini",
        display_name: "AI21 Jamba 1.5 Mini",
        max_context_length: 256_000,
        max_output_length: 4_096,
        supports_tools: false,
        supports_multimodal: false,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
    // Phi Models
    GitHubCatalogModel {
        model_id: "phi-3.5-moe-instruct",
        display_name: "Phi 3.5 MoE Instruct",
        max_context_length: 128_000,
        max_output_length: 4_096,
        supports_tools: false,
        supports_multimodal: false,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
    GitHubCatalogModel {
        model_id: "phi-3.5-mini-instruct",
        display_name: "Phi 3.5 Mini Instruct",
        max_context_length: 128_000,
        max_output_length: 4_096,
        supports_tools: false,
        supports_multimodal: false,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
    GitHubCatalogModel {
        model_id: "phi-3.5-vision-instruct",
        display_name: "Phi 3.5 Vision Instruct",
        max_context_length: 128_000,
        max_output_length: 4_096,
        supports_tools: false,
        supports_multimodal: true,
        input_cost_per_million: 0.0,
        output_cost_per_million: 0.0,
    },
];

// The full info list is the single ModelInfo projection source
// (`github_catalog_model_info`). `OpenAILikeProvider::get_model_info`
// single-model resolution does not consume it yet (amazon_nova parity is
// deferred: `openai_like/provider.rs` is at the U-16 800-line hard ceiling);
// tracked in the GH837 T9/T14 gate alongside the pricing authority hook.
static GITHUB_MODEL_INFOS: LazyLock<Vec<ModelInfo>> = LazyLock::new(|| {
    GITHUB_CATALOG_MODELS
        .iter()
        .filter_map(|entry| github_catalog_model_info(entry.model_id))
        .collect()
});

pub(crate) fn github_catalog_model(model: &str) -> Option<&'static GitHubCatalogModel> {
    GITHUB_CATALOG_MODELS
        .iter()
        .find(|entry| entry.model_id == model)
}

pub(crate) fn github_catalog_model_infos() -> &'static [ModelInfo] {
    &GITHUB_MODEL_INFOS
}

pub(crate) fn github_catalog_model_info(model: &str) -> Option<ModelInfo> {
    github_catalog_model(model).map(github_model_info_from_entry)
}

// Mirrors the native `github` provider's per-model capability projection:
// chat + streaming chat always, tool-calling only when the model supports it.
fn github_model_info_from_entry(entry: &GitHubCatalogModel) -> ModelInfo {
    let mut capabilities = vec![
        ProviderCapability::ChatCompletion,
        ProviderCapability::ChatCompletionStream,
    ];
    if entry.supports_tools {
        capabilities.push(ProviderCapability::ToolCalling);
    }
    ModelInfo {
        id: entry.model_id.to_string(),
        name: entry.display_name.to_string(),
        provider: "github".to_string(),
        max_context_length: entry.max_context_length,
        max_output_length: Some(entry.max_output_length),
        supports_streaming: GITHUB_SUPPORTS_STREAMING,
        supports_tools: entry.supports_tools,
        supports_multimodal: entry.supports_multimodal,
        input_cost_per_1k_tokens: Some(entry.input_cost_per_million / 1_000.0),
        output_cost_per_1k_tokens: Some(entry.output_cost_per_million / 1_000.0),
        currency: "USD".to_string(),
        capabilities,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::providers::registry::catalog::get_definition;
    use crate::core::types::model::ProviderCapability;

    // Locks the GitHub Models catalog policy: the full 16-model roster, the
    // transcribed pricing, and the provider capability profile. The native
    // Historical 0.6 metadata; provider availability is audited separately.
    #[test]
    fn github_catalog_policy_locks_models_pricing_and_capabilities() {
        assert_eq!(GITHUB_CATALOG_MODELS.len(), 16);
        assert_eq!(github_catalog_model_infos().len(), 16);

        // Paid models keep their transcribed per-million pricing.
        let gpt_4o = github_catalog_model("gpt-4o").expect("gpt-4o must be catalogued");
        assert_eq!(gpt_4o.display_name, "GPT-4o");
        assert_eq!(gpt_4o.max_context_length, 128_000);
        assert_eq!(gpt_4o.max_output_length, 16_384);
        assert!(gpt_4o.supports_tools);
        assert!(gpt_4o.supports_multimodal);
        assert_eq!(gpt_4o.input_cost_per_million, 2.5);
        assert_eq!(gpt_4o.output_cost_per_million, 10.0);

        let o1_preview = github_catalog_model("o1-preview").expect("o1-preview must be catalogued");
        assert_eq!(o1_preview.max_output_length, 32_768);
        assert!(!o1_preview.supports_tools);
        assert!(!o1_preview.supports_multimodal);
        assert_eq!(o1_preview.input_cost_per_million, 15.0);
        assert_eq!(o1_preview.output_cost_per_million, 60.0);

        // Free models genuinely carry zero pricing in the native authority.
        let llama = github_catalog_model("meta-llama-3.1-70b-instruct")
            .expect("meta-llama-3.1-70b-instruct must be catalogued");
        assert_eq!(llama.input_cost_per_million, 0.0);
        assert_eq!(llama.output_cost_per_million, 0.0);

        // Per-model ModelInfo projection: pricing per 1k tokens and the
        // native-equivalent conditional capability set.
        let info = github_catalog_model_info("gpt-4o").expect("gpt-4o model info");
        assert_eq!(info.provider, "github");
        assert_eq!(info.input_cost_per_1k_tokens, Some(2.5 / 1_000.0));
        assert_eq!(info.output_cost_per_1k_tokens, Some(10.0 / 1_000.0));
        assert!(info.capabilities.contains(&ProviderCapability::ToolCalling));

        let no_tools = github_catalog_model_info("o1-preview").expect("o1-preview model info");
        assert!(
            !no_tools
                .capabilities
                .contains(&ProviderCapability::ToolCalling)
        );

        // Provider-level capability profile mirrors the amazon_nova policy.
        assert_eq!(
            GITHUB_CATALOG_CAPABILITIES,
            &[
                ProviderCapability::ChatCompletion,
                ProviderCapability::ChatCompletionStream,
                ProviderCapability::ToolCalling,
            ]
        );
    }

    // The catalog route must keep the fixed GitHub Models endpoint and Bearer
    // auth contract. The literal equals the native `GITHUB_MODELS_API_BASE`
    // constant (`src/core/providers/github/config.rs`); this test locks the
    // literal directly. The constant is not re-exported from the deprecated
    // native module, so a live-constant comparison is intentionally avoided;
    // the native module is slated for removal in 0.7.0.
    #[test]
    fn github_catalog_policy_base_url_and_auth_contract() {
        let definition = get_definition("github").expect("github catalog definition must exist");
        assert_eq!(definition.base_url, "https://models.inference.ai.azure.com");
        assert_eq!(definition.auth_env_var, "GITHUB_TOKEN");
        assert_eq!(definition.capabilities, GITHUB_CATALOG_CAPABILITIES);
    }
}
