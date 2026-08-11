//! Provider Catalog - static registry of all Tier 1 providers
//!
//! Each entry fully describes an OpenAI-compatible provider.
//! Adding a new provider is a single entry here — zero code needed.

use std::collections::HashMap;
use std::sync::LazyLock;

use super::definition::{AuthType, ProviderDefinition};
use crate::core::providers::openai_like::provider::OPENAI_LIKE_CATALOG_CAPABILITIES;
use crate::core::types::model::{ModelInfo, ProviderCapability};
pub(crate) const AMAZON_NOVA_CATALOG_CAPABILITIES: &[ProviderCapability] = &[
    ProviderCapability::ChatCompletion,
    ProviderCapability::ChatCompletionStream,
    ProviderCapability::ToolCalling,
];
pub(crate) const AMAZON_NOVA_SUPPORTS_STREAMING: bool = true;
pub(crate) const AMAZON_NOVA_SUPPORTS_TOOLS: bool = true;
pub(crate) struct AmazonNovaCatalogModel {
    pub(crate) model_id: &'static str,
    pub(crate) display_name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) max_context_length: u32,
    pub(crate) max_output_length: u32,
    pub(crate) supports_multimodal: bool,
    pub(crate) supports_reasoning: bool,
    pub(crate) input_cost_per_million: f64,
    pub(crate) output_cost_per_million: f64,
}
pub(crate) static AMAZON_NOVA_CATALOG_MODELS: &[AmazonNovaCatalogModel] = &[
    AmazonNovaCatalogModel {
        model_id: "amazon.nova-2-lite-v1:0",
        display_name: "Amazon Nova 2 Lite",
        description: "Cost-efficient multimodal model for automation, documents, and support",
        max_context_length: 1_000_000,
        max_output_length: 64_000,
        supports_multimodal: true,
        supports_reasoning: true,
        input_cost_per_million: 0.3,
        output_cost_per_million: 2.5,
    },
    AmazonNovaCatalogModel {
        model_id: "amazon.nova-pro-v1:0",
        display_name: "Amazon Nova Pro",
        description: "High-capability multimodal model for complex tasks",
        max_context_length: 300_000,
        max_output_length: 5_000,
        supports_multimodal: true,
        supports_reasoning: true,
        input_cost_per_million: 0.8,
        output_cost_per_million: 3.2,
    },
    AmazonNovaCatalogModel {
        model_id: "amazon.nova-lite-v1:0",
        display_name: "Amazon Nova Lite",
        description: "Cost-effective multimodal model for everyday tasks",
        max_context_length: 300_000,
        max_output_length: 5_000,
        supports_multimodal: true,
        supports_reasoning: false,
        input_cost_per_million: 0.06,
        output_cost_per_million: 0.24,
    },
    AmazonNovaCatalogModel {
        model_id: "amazon.nova-micro-v1:0",
        display_name: "Amazon Nova Micro",
        description: "Fast text-only model optimized for speed",
        max_context_length: 128_000,
        max_output_length: 5_000,
        supports_multimodal: false,
        supports_reasoning: false,
        input_cost_per_million: 0.035,
        output_cost_per_million: 0.14,
    },
    AmazonNovaCatalogModel {
        model_id: "amazon.nova-premier-v1:0",
        display_name: "Amazon Nova Premier",
        description: "Most capable model for complex reasoning and multimodal tasks",
        max_context_length: 1_000_000,
        max_output_length: 10_000,
        supports_multimodal: true,
        supports_reasoning: true,
        input_cost_per_million: 2.5,
        output_cost_per_million: 12.5,
    },
];
pub(crate) const AMAZON_NOVA_MODEL_ALIASES: &[(&str, &str)] = &[
    ("nova-2-lite", "amazon.nova-2-lite-v1:0"),
    ("nova-pro", "amazon.nova-pro-v1:0"),
    ("nova-lite", "amazon.nova-lite-v1:0"),
    ("nova-micro", "amazon.nova-micro-v1:0"),
    ("nova-premier", "amazon.nova-premier-v1:0"),
];
static AMAZON_NOVA_MODEL_INFOS: LazyLock<Vec<ModelInfo>> = LazyLock::new(|| {
    AMAZON_NOVA_CATALOG_MODELS
        .iter()
        .map(amazon_nova_model_info_from_entry)
        .collect()
});
pub(crate) fn amazon_nova_catalog_model(model: &str) -> Option<&'static AmazonNovaCatalogModel> {
    let canonical = AMAZON_NOVA_MODEL_ALIASES
        .iter()
        .find(|(alias, _)| *alias == model)
        .map_or(model, |(_, canonical)| *canonical);
    AMAZON_NOVA_CATALOG_MODELS
        .iter()
        .find(|entry| entry.model_id == canonical)
}
pub(crate) fn amazon_nova_catalog_model_infos() -> &'static [ModelInfo] {
    &AMAZON_NOVA_MODEL_INFOS
}
pub(crate) fn amazon_nova_catalog_model_info(model: &str) -> Option<ModelInfo> {
    amazon_nova_catalog_model(model).map(amazon_nova_model_info_from_entry)
}
fn amazon_nova_model_info_from_entry(entry: &AmazonNovaCatalogModel) -> ModelInfo {
    ModelInfo {
        id: entry.model_id.to_string(),
        name: entry.display_name.to_string(),
        provider: "amazon_nova".to_string(),
        max_context_length: entry.max_context_length,
        max_output_length: Some(entry.max_output_length),
        supports_streaming: AMAZON_NOVA_SUPPORTS_STREAMING,
        supports_tools: AMAZON_NOVA_SUPPORTS_TOOLS,
        supports_multimodal: entry.supports_multimodal,
        input_cost_per_1k_tokens: Some(entry.input_cost_per_million / 1_000.0),
        output_cost_per_1k_tokens: Some(entry.output_cost_per_million / 1_000.0),
        currency: "USD".to_string(),
        capabilities: vec![
            ProviderCapability::ChatCompletion,
            ProviderCapability::ChatCompletionStream,
        ],
        metadata: HashMap::from([
            ("description".into(), entry.description.into()),
            ("supports_reasoning".into(), entry.supports_reasoning.into()),
        ]),
        ..Default::default()
    }
}

/// Global provider catalog, keyed by provider name.
pub static PROVIDER_CATALOG: LazyLock<HashMap<&'static str, ProviderDefinition>> =
    LazyLock::new(build_catalog);

/// Check if a provider name is in the Tier 1 catalog
pub fn is_tier1_provider(name: &str) -> bool {
    canonical_catalog_name(name).is_some()
}

/// Get a provider definition by name
pub fn get_definition(name: &str) -> Option<&'static ProviderDefinition> {
    let canonical = canonical_catalog_name(name)?;
    PROVIDER_CATALOG.get(canonical)
}

/// Return the canonical Tier 1 catalog selector for a provider name or alias.
pub fn canonical_catalog_name(name: &str) -> Option<&'static str> {
    let normalized = name.trim().to_ascii_lowercase().replace('-', "_");
    if let Some((canonical, _)) = PROVIDER_CATALOG.get_key_value(normalized.as_str()) {
        return Some(*canonical);
    }

    match normalized.as_str() {
        "aimlapi" => Some("aiml_api"),
        "fireworksai" => Some("fireworks_ai"),
        "togetherai" => Some("together_ai"),
        "glm" | "zhipuai" => Some("zhipu"),
        _ => None,
    }
}

fn build_catalog() -> HashMap<&'static str, ProviderDefinition> {
    let defs: Vec<ProviderDefinition> = vec![
        // ===== Group 1b: Cloud OpenAI-compatible =====
        // ===== Group 1b: Cloud OpenAI-compatible =====
        def_chat(
            "groq",
            "Groq",
            "https://api.groq.com/openai/v1",
            "GROQ_API_KEY",
        ),
        ProviderDefinition {
            alternate_auth_env_vars: &[
                "TOGETHER_AI_API_KEY",
                "TOGETHERAI_API_KEY",
                "TOGETHER_AI_TOKEN",
            ],
            ..def_chat(
                "together",
                "Together AI",
                "https://api.together.xyz/v1",
                "TOGETHER_API_KEY",
            )
        },
        ProviderDefinition {
            alternate_auth_env_vars: &[
                "TOGETHER_AI_API_KEY",
                "TOGETHERAI_API_KEY",
                "TOGETHER_AI_TOKEN",
            ],
            ..def_chat(
                "together_ai",
                "Together AI",
                "https://api.together.xyz/v1",
                "TOGETHER_API_KEY",
            )
        },
        ProviderDefinition {
            alternate_auth_env_vars: &[
                "FIREWORKS_AI_API_KEY",
                "FIREWORKSAI_API_KEY",
                "FIREWORKS_AI_TOKEN",
            ],
            ..def_chat(
                "fireworks",
                "Fireworks AI",
                "https://api.fireworks.ai/inference/v1",
                "FIREWORKS_API_KEY",
            )
        },
        ProviderDefinition {
            alternate_auth_env_vars: &[
                "FIREWORKS_AI_API_KEY",
                "FIREWORKSAI_API_KEY",
                "FIREWORKS_AI_TOKEN",
            ],
            ..def_chat(
                "fireworks_ai",
                "Fireworks AI",
                "https://api.fireworks.ai/inference/v1",
                "FIREWORKS_API_KEY",
            )
        },
        def_chat(
            "perplexity",
            "Perplexity AI",
            "https://api.perplexity.ai",
            "PERPLEXITY_API_KEY",
        ),
        def_chat(
            "cerebras",
            "Cerebras",
            "https://api.cerebras.ai/v1",
            "CEREBRAS_API_KEY",
        ),
        def_chat(
            "openrouter",
            "OpenRouter",
            "https://openrouter.ai/api/v1",
            "OPENROUTER_API_KEY",
        ),
        def_chat(
            "deepinfra",
            "DeepInfra",
            "https://api.deepinfra.com/v1/openai",
            "DEEPINFRA_API_KEY",
        ),
        def_chat(
            "deepseek",
            "DeepSeek",
            "https://api.deepseek.com",
            "DEEPSEEK_API_KEY",
        ),
        def_chat(
            "novita",
            "Novita AI",
            "https://api.novita.ai/v3/openai",
            "NOVITA_API_KEY",
        ),
        def_chat(
            "nvidia_nim",
            "NVIDIA NIM",
            "https://integrate.api.nvidia.com/v1",
            "NVIDIA_NIM_API_KEY",
        ),
        def_chat(
            "nebius",
            "Nebius AI",
            "https://api.studio.nebius.ai/v1",
            "NEBIUS_API_KEY",
        ),
        def_chat(
            "nscale",
            "Nscale",
            "https://inference.api.nscale.ai/v1",
            "NSCALE_API_KEY",
        ),
        def_chat(
            "hyperbolic",
            "Hyperbolic",
            "https://api.hyperbolic.xyz/v1",
            "HYPERBOLIC_API_KEY",
        ),
        def_chat(
            "featherless",
            "Featherless AI",
            "https://api.featherless.ai/v1",
            "FEATHERLESS_API_KEY",
        ),
        def_chat(
            "galadriel",
            "Galadriel",
            "https://api.galadriel.com/v1",
            "GALADRIEL_API_KEY",
        ),
        def_chat(
            "sambanova",
            "SambaNova",
            "https://api.sambanova.ai/v1",
            "SAMBANOVA_API_KEY",
        ),
        def_chat(
            "heroku",
            "Heroku",
            "https://us.inference.heroku.com/v1",
            "HEROKU_API_KEY",
        ),
        def_chat(
            "friendliai",
            "FriendliAI",
            "https://api.friendli.ai/v1",
            "FRIENDLIAI_API_KEY",
        ),
        ProviderDefinition {
            capabilities: super::catalog_policy::META_LLAMA_CAPABILITIES,
            ..def_chat(
                "meta_llama",
                "Meta Llama API",
                "https://api.llama.com/compat/v1",
                "META_LLAMA_API_KEY",
            )
        },
        ProviderDefinition {
            capabilities: super::catalog_policy::V0_CAPABILITIES,
            ..def_chat("v0", "Vercel v0", "https://api.v0.dev/v1", "V0_API_KEY")
        },
        ProviderDefinition {
            capabilities: AMAZON_NOVA_CATALOG_CAPABILITIES,
            ..def_chat(
                "amazon_nova",
                "Amazon Nova",
                "https://api.nova.amazon.com/v1",
                "AMAZON_NOVA_API_KEY",
            )
        },
        // github: native module deprecated in 0.6.0, slated for 0.7.0 catalog
        // demotion. The catalog route is the supported path; its base_url equals
        // the native `github::config::GITHUB_MODELS_API_BASE` constant. The
        // literal is locked by github_catalog_policy_base_url_and_auth_contract
        // in registry::github_policy and by
        // issue_606_openai_like_candidates_are_catalog_entries.
        ProviderDefinition {
            capabilities: super::github_policy::GITHUB_CATALOG_CAPABILITIES,
            ..def_chat(
                "github",
                "GitHub Models",
                "https://models.inference.ai.azure.com",
                "GITHUB_TOKEN",
            )
        },
        // xAI publishes OpenAI-compatible chat endpoints; model IDs are
        // passed through instead of enumerated in this static provider catalog.
        ProviderDefinition {
            model_prefix: Some("xai/"),
            ..def_chat("xai", "xAI", "https://api.x.ai/v1", "XAI_API_KEY")
        },
        // ===== Group 1c: Local inference (no API key) =====
        def_local_chat("vllm", "vLLM", "http://localhost:8000/v1"),
        def_local_chat("hosted_vllm", "Hosted vLLM", "http://localhost:8000/v1"),
        def_local_chat("lm_studio", "LM Studio", "http://localhost:1234/v1"),
        def_local_chat("llamafile", "Llamafile", "http://localhost:8080/v1"),
        def_local_chat(
            "docker_model_runner",
            "Docker Model Runner",
            "http://localhost:12434/engines/llama.cpp/v1",
        ),
        def_local_chat("xinference", "Xinference", "http://localhost:9997/v1"),
        def_local_chat("infinity", "Infinity", "http://localhost:7997/v1"),
        def_local_chat("oobabooga", "Oobabooga", "http://localhost:5000/v1"),
        // ===== Group 1d: Chinese OpenAI-compatible =====
        def_chat(
            "moonshot",
            "Moonshot AI",
            "https://api.moonshot.cn/v1",
            "MOONSHOT_API_KEY",
        ),
        def_chat(
            "dashscope",
            "Dashscope",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "DASHSCOPE_API_KEY",
        ),
        def_chat(
            "qwen",
            "Qwen",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "DASHSCOPE_API_KEY",
        ),
        def_chat(
            "baichuan",
            "Baichuan",
            "https://api.baichuan-ai.com/v1",
            "BAICHUAN_API_KEY",
        ),
        def_chat(
            "minimax",
            "MiniMax",
            "https://api.minimax.chat/v1",
            "MINIMAX_API_KEY",
        ),
        def_chat(
            "volcengine",
            "Volcengine",
            "https://ark.cn-beijing.volces.com/api/v3",
            "VOLCENGINE_API_KEY",
        ),
        ProviderDefinition {
            alternate_auth_env_vars: &["XIAOMI_API_KEY"],
            ..def_chat(
                "xiaomi_mimo",
                "Xiaomi MiMo",
                "https://api.xiaomimimo.com/v1",
                "MIMO_API_KEY",
            )
        },
        def_chat(
            "zhipu",
            "Zhipu AI",
            "https://open.bigmodel.cn/api/paas/v4",
            "ZHIPU_API_KEY",
        ),
        def_chat("zai", "ZAI", "https://api.z.ai/api/paas/v4", "ZAI_API_KEY"),
        // ===== Group 1e: Other OpenAI-compatible =====
        def_chat(
            "lemonade",
            "Lemonade",
            "https://api.lemonade.social/v1",
            "LEMONADE_API_KEY",
        ),
        def_chat(
            "linkup",
            "Linkup",
            "https://api.linkup.so/v1",
            "LINKUP_API_KEY",
        ),
        def_chat("poe", "Poe", "https://api.poe.com/v1", "POE_API_KEY"),
        def_chat(
            "wandb",
            "Weights & Biases",
            "https://api.wandb.ai/v1",
            "WANDB_API_KEY",
        ),
        def_chat(
            "nanogpt",
            "NanoGPT",
            "https://api.nanogpt.com/v1",
            "NANOGPT_API_KEY",
        ),
        // ===== Group 1a: Previously macro-based =====
        ProviderDefinition {
            alternate_auth_env_vars: &["AIMLAPI_KEY"],
            ..def_chat(
                "aiml_api",
                "AIML API",
                "https://api.aimlapi.com/v1",
                "AIML_API_KEY",
            )
        },
        ProviderDefinition {
            alternate_auth_env_vars: &["AIMLAPI_KEY"],
            ..def_chat(
                "aiml",
                "AIML API",
                "https://api.aimlapi.com/v1",
                "AIML_API_KEY",
            )
        },
        def_chat(
            "aleph_alpha",
            "Aleph Alpha",
            "https://api.aleph-alpha.com/v1",
            "ALEPH_ALPHA_API_KEY",
        ),
        def_chat(
            "anyscale",
            "Anyscale",
            "https://api.endpoints.anyscale.com/v1",
            "ANYSCALE_API_KEY",
        ),
        def_chat(
            "bytez",
            "Bytez",
            "https://api.bytez.com/v1",
            "BYTEZ_API_KEY",
        ),
        def_chat(
            "comet_api",
            "Comet API",
            "https://api.comet.com/v1",
            "COMET_API_KEY",
        ),
        def_chat(
            "compactifai",
            "CompactifAI",
            "https://api.compactif.ai/v1",
            "COMPACTIFAI_API_KEY",
        ),
        def_chat(
            "maritalk",
            "MariTalk",
            "https://chat.maritaca.ai/api",
            "MARITALK_API_KEY",
        ),
        def_chat(
            "siliconflow",
            "SiliconFlow",
            "https://api.siliconflow.cn/v1",
            "SILICONFLOW_API_KEY",
        ),
        def_chat("yi", "Yi", "https://api.lingyiwanwu.com/v1", "YI_API_KEY"),
        def_chat(
            "lambda_ai",
            "Lambda AI",
            "https://api.lambdalabs.com/v1",
            "LAMBDA_API_KEY",
        ),
        def_chat(
            "ovhcloud",
            "OVHcloud",
            "https://api.ai.cloud.ovh.net/v1",
            "OVHCLOUD_API_KEY",
        ),
    ];

    let mut map = HashMap::with_capacity(defs.len());
    for d in defs {
        map.insert(d.name, d);
    }
    map
}

/// Helper: standard Bearer-auth cloud provider
fn def_chat(
    name: &'static str,
    display_name: &'static str,
    base_url: &'static str,
    auth_env_var: &'static str,
) -> ProviderDefinition {
    ProviderDefinition {
        name,
        display_name,
        base_url,
        auth_env_var,
        alternate_auth_env_vars: &[],
        auth_type: AuthType::Bearer,
        skip_api_key: false,
        model_prefix: None,
        capabilities: OPENAI_LIKE_CATALOG_CAPABILITIES,
    }
}

/// Helper: local provider (no API key required)
fn def_local_chat(
    name: &'static str,
    display_name: &'static str,
    base_url: &'static str,
) -> ProviderDefinition {
    ProviderDefinition {
        name,
        display_name,
        base_url,
        auth_env_var: "",
        alternate_auth_env_vars: &[],
        auth_type: AuthType::None,
        skip_api_key: true,
        model_prefix: None,
        capabilities: OPENAI_LIKE_CATALOG_CAPABILITIES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_catalog_capability_is_executable_and_unique() {
        for definition in PROVIDER_CATALOG.values() {
            assert!(
                !definition.capabilities.is_empty(),
                "{} must declare a capability profile",
                definition.name
            );
            for (index, capability) in definition.capabilities.iter().enumerate() {
                assert!(
                    !definition.capabilities[..index].contains(capability),
                    "{} declares duplicate capability {capability:?}",
                    definition.name
                );
                assert!(
                    OPENAI_LIKE_CATALOG_CAPABILITIES.contains(capability),
                    "{} declares non-executable capability {capability:?}",
                    definition.name
                );
            }
        }
    }

    #[tokio::test]
    async fn amazon_nova_catalog_runtime_exposes_models_and_pricing() {
        use crate::core::providers::openai_like::{OpenAILikeConfig, OpenAILikeProvider};
        use crate::core::traits::provider::llm_provider::trait_definition::LLMProvider;
        let provider = OpenAILikeProvider::new_for_catalog(
            OpenAILikeConfig::with_api_key("https://8.8.8.8/v1", "catalog-runtime-test-key")
                .with_provider_name("amazon_nova"),
            AMAZON_NOVA_CATALOG_CAPABILITIES,
        )
        .await
        .expect("catalog provider must construct");
        assert_eq!(provider.models().len(), 5);
        for model in ["amazon.nova-pro-v1:0", "nova-pro"] {
            let cost = provider
                .calculate_cost(model, 1_000, 1_000)
                .await
                .expect("pricing");
            assert!((cost - 0.004).abs() < f64::EPSILON);
        }
        let unknown = provider
            .calculate_cost("grok-4.3", 1_000, 1_000)
            .await
            .unwrap();
        assert_eq!(unknown, 0.0);
    }
    #[test]
    fn test_xai_openai_compatible_pass_through_definition() {
        let Some(definition) = get_definition("xai") else {
            panic!("xAI provider definition should be registered");
        };

        assert_eq!(definition.name, "xai");
        assert_eq!(definition.display_name, "xAI");
        assert_eq!(definition.base_url, "https://api.x.ai/v1");
        assert_eq!(definition.auth_env_var, "XAI_API_KEY");
        assert!(definition.alternate_auth_env_vars.is_empty());
        assert_eq!(definition.auth_type, AuthType::Bearer);
        assert!(!definition.skip_api_key);
        assert_eq!(definition.model_prefix, Some("xai/"));

        let config = definition.to_openai_like_config(Some("sk-test"), None);
        assert_eq!(config.model_prefix.as_deref(), Some("xai/"));
    }

    #[test]
    fn test_xiaomi_mimo_uses_official_endpoint_and_key_name() {
        let Some(definition) = get_definition("xiaomi_mimo") else {
            panic!("Xiaomi MiMo provider definition should be registered");
        };

        assert_eq!(definition.display_name, "Xiaomi MiMo");
        assert_eq!(definition.base_url, "https://api.xiaomimimo.com/v1");
        assert_eq!(definition.auth_env_var, "MIMO_API_KEY");
        assert_eq!(definition.alternate_auth_env_vars, &["XIAOMI_API_KEY"]);
        assert_eq!(definition.auth_type, AuthType::Bearer);
        assert!(!definition.skip_api_key);
    }

    #[test]
    fn litellm_provider_aliases_resolve_to_canonical_catalog_definitions() {
        let cases = [
            (
                "together_ai",
                "together_ai",
                "TOGETHER_API_KEY",
                "https://api.together.xyz/v1",
            ),
            (
                "fireworks_ai",
                "fireworks_ai",
                "FIREWORKS_API_KEY",
                "https://api.fireworks.ai/inference/v1",
            ),
            ("aiml", "aiml", "AIML_API_KEY", "https://api.aimlapi.com/v1"),
            ("zai", "zai", "ZAI_API_KEY", "https://api.z.ai/api/paas/v4"),
            (
                "zhipuai",
                "zhipu",
                "ZHIPU_API_KEY",
                "https://open.bigmodel.cn/api/paas/v4",
            ),
        ];

        for (alias, canonical, auth_env_var, base_url) in cases {
            let Some(definition) = get_definition(alias) else {
                panic!("{alias} alias should resolve to a catalog definition");
            };

            assert_eq!(definition.name, canonical);
            assert_eq!(definition.auth_env_var, auth_env_var);
            assert_eq!(definition.base_url, base_url);
            assert_eq!(canonical_catalog_name(alias), Some(canonical));
            assert!(is_tier1_provider(alias));
            let canonical_definition =
                get_definition(canonical).expect("canonical catalog definition should exist");
            assert_eq!(definition.capabilities, canonical_definition.capabilities);
        }
    }

    #[test]
    fn issue_606_openai_like_candidates_are_catalog_entries() {
        let expected = [
            (
                "meta_llama",
                "https://api.llama.com/compat/v1",
                "META_LLAMA_API_KEY",
            ),
            ("v0", "https://api.v0.dev/v1", "V0_API_KEY"),
            (
                "amazon_nova",
                "https://api.nova.amazon.com/v1",
                "AMAZON_NOVA_API_KEY",
            ),
            (
                "github",
                "https://models.inference.ai.azure.com",
                "GITHUB_TOKEN",
            ),
        ];

        for (name, base_url, auth_env_var) in expected {
            let Some(definition) = get_definition(name) else {
                panic!("{name} provider definition should be registered");
            };
            assert_eq!(definition.base_url, base_url);
            assert_eq!(definition.auth_env_var, auth_env_var);
            assert_eq!(definition.auth_type, AuthType::Bearer);
            assert!(!definition.skip_api_key);
        }
    }
}
