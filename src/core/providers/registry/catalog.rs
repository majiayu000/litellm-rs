//! Provider Catalog - static registry of all Tier 1 providers
//!
//! Each entry fully describes an OpenAI-compatible provider.
//! Adding a new provider is a single entry here — zero code needed.

use std::collections::HashMap;
use std::sync::LazyLock;

use super::definition::{AuthType, ProviderDefinition};

/// Global provider catalog, keyed by provider name.
pub static PROVIDER_CATALOG: LazyLock<HashMap<&'static str, ProviderDefinition>> =
    LazyLock::new(build_catalog);

/// Check if a provider name is in the Tier 1 catalog
pub fn is_tier1_provider(name: &str) -> bool {
    PROVIDER_CATALOG.contains_key(name)
}

/// Get a provider definition by name
pub fn get_definition(name: &str) -> Option<&'static ProviderDefinition> {
    PROVIDER_CATALOG.get(name)
}

fn build_catalog() -> HashMap<&'static str, ProviderDefinition> {
    let defs: Vec<ProviderDefinition> = vec![
        // ===== Group 1b: Cloud OpenAI-compatible =====
        // ===== Group 1b: Cloud OpenAI-compatible =====
        def(
            "groq",
            "Groq",
            "https://api.groq.com/openai/v1",
            "GROQ_API_KEY",
        ),
        def(
            "together",
            "Together AI",
            "https://api.together.xyz/v1",
            "TOGETHER_API_KEY",
        ),
        def(
            "fireworks",
            "Fireworks AI",
            "https://api.fireworks.ai/inference/v1",
            "FIREWORKS_API_KEY",
        ),
        def(
            "perplexity",
            "Perplexity AI",
            "https://api.perplexity.ai",
            "PERPLEXITY_API_KEY",
        ),
        def(
            "cerebras",
            "Cerebras",
            "https://api.cerebras.ai/v1",
            "CEREBRAS_API_KEY",
        ),
        def(
            "openrouter",
            "OpenRouter",
            "https://openrouter.ai/api/v1",
            "OPENROUTER_API_KEY",
        ),
        def(
            "deepinfra",
            "DeepInfra",
            "https://api.deepinfra.com/v1/openai",
            "DEEPINFRA_API_KEY",
        ),
        def(
            "deepseek",
            "DeepSeek",
            "https://api.deepseek.com",
            "DEEPSEEK_API_KEY",
        ),
        def(
            "novita",
            "Novita AI",
            "https://api.novita.ai/v3/openai",
            "NOVITA_API_KEY",
        ),
        def(
            "nvidia_nim",
            "NVIDIA NIM",
            "https://integrate.api.nvidia.com/v1",
            "NVIDIA_NIM_API_KEY",
        ),
        def(
            "nebius",
            "Nebius AI",
            "https://api.studio.nebius.ai/v1",
            "NEBIUS_API_KEY",
        ),
        def(
            "nscale",
            "Nscale",
            "https://inference.api.nscale.ai/v1",
            "NSCALE_API_KEY",
        ),
        def(
            "hyperbolic",
            "Hyperbolic",
            "https://api.hyperbolic.xyz/v1",
            "HYPERBOLIC_API_KEY",
        ),
        def(
            "featherless",
            "Featherless AI",
            "https://api.featherless.ai/v1",
            "FEATHERLESS_API_KEY",
        ),
        def(
            "galadriel",
            "Galadriel",
            "https://api.galadriel.com/v1",
            "GALADRIEL_API_KEY",
        ),
        def(
            "sambanova",
            "SambaNova",
            "https://api.sambanova.ai/v1",
            "SAMBANOVA_API_KEY",
        ),
        def(
            "heroku",
            "Heroku",
            "https://us.inference.heroku.com/v1",
            "HEROKU_API_KEY",
        ),
        def(
            "friendliai",
            "FriendliAI",
            "https://api.friendli.ai/v1",
            "FRIENDLIAI_API_KEY",
        ),
        def("xai", "xAI", "https://api.x.ai/v1", "XAI_API_KEY"),
        // ===== Group 1c: Local inference (no API key) =====
        def_local("vllm", "vLLM", "http://localhost:8000/v1"),
        def_local("hosted_vllm", "Hosted vLLM", "http://localhost:8000/v1"),
        def_local("lm_studio", "LM Studio", "http://localhost:1234/v1"),
        def_local("llamafile", "Llamafile", "http://localhost:8080/v1"),
        def_local(
            "docker_model_runner",
            "Docker Model Runner",
            "http://localhost:12434/engines/llama.cpp/v1",
        ),
        def_local("xinference", "Xinference", "http://localhost:9997/v1"),
        def_local("infinity", "Infinity", "http://localhost:7997/v1"),
        def_local("oobabooga", "Oobabooga", "http://localhost:5000/v1"),
        // ===== Group 1d: Chinese OpenAI-compatible =====
        def(
            "moonshot",
            "Moonshot AI",
            "https://api.moonshot.cn/v1",
            "MOONSHOT_API_KEY",
        ),
        def(
            "dashscope",
            "Dashscope",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "DASHSCOPE_API_KEY",
        ),
        def(
            "qwen",
            "Qwen",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "DASHSCOPE_API_KEY",
        ),
        def(
            "baichuan",
            "Baichuan",
            "https://api.baichuan-ai.com/v1",
            "BAICHUAN_API_KEY",
        ),
        def(
            "minimax",
            "MiniMax",
            "https://api.minimax.chat/v1",
            "MINIMAX_API_KEY",
        ),
        def(
            "volcengine",
            "Volcengine",
            "https://ark.cn-beijing.volces.com/api/v3",
            "VOLCENGINE_API_KEY",
        ),
        def(
            "xiaomi_mimo",
            "Xiaomi MiMo",
            "https://api.xiaomi.com/v1",
            "XIAOMI_API_KEY",
        ),
        def(
            "zhipu",
            "Zhipu AI",
            "https://open.bigmodel.cn/api/paas/v4",
            "ZHIPU_API_KEY",
        ),
        // ===== Group 1e: Other OpenAI-compatible =====
        def(
            "lemonade",
            "Lemonade",
            "https://api.lemonade.social/v1",
            "LEMONADE_API_KEY",
        ),
        def(
            "linkup",
            "Linkup",
            "https://api.linkup.so/v1",
            "LINKUP_API_KEY",
        ),
        def("poe", "Poe", "https://api.poe.com/v1", "POE_API_KEY"),
        def(
            "wandb",
            "Weights & Biases",
            "https://api.wandb.ai/v1",
            "WANDB_API_KEY",
        ),
        def(
            "nanogpt",
            "NanoGPT",
            "https://api.nanogpt.com/v1",
            "NANOGPT_API_KEY",
        ),
        // ===== Group 1a: Previously macro-based =====
        def(
            "aiml_api",
            "AIML API",
            "https://api.aimlapi.com/v1",
            "AIML_API_KEY",
        ),
        def(
            "aleph_alpha",
            "Aleph Alpha",
            "https://api.aleph-alpha.com/v1",
            "ALEPH_ALPHA_API_KEY",
        ),
        def(
            "anyscale",
            "Anyscale",
            "https://api.endpoints.anyscale.com/v1",
            "ANYSCALE_API_KEY",
        ),
        def(
            "bytez",
            "Bytez",
            "https://api.bytez.com/v1",
            "BYTEZ_API_KEY",
        ),
        def(
            "comet_api",
            "Comet API",
            "https://api.comet.com/v1",
            "COMET_API_KEY",
        ),
        def(
            "compactifai",
            "CompactifAI",
            "https://api.compactif.ai/v1",
            "COMPACTIFAI_API_KEY",
        ),
        def(
            "maritalk",
            "MariTalk",
            "https://chat.maritaca.ai/api",
            "MARITALK_API_KEY",
        ),
        def(
            "siliconflow",
            "SiliconFlow",
            "https://api.siliconflow.cn/v1",
            "SILICONFLOW_API_KEY",
        ),
        def("yi", "Yi", "https://api.lingyiwanwu.com/v1", "YI_API_KEY"),
        def(
            "lambda_ai",
            "Lambda AI",
            "https://api.lambdalabs.com/v1",
            "LAMBDA_API_KEY",
        ),
        def(
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
fn def(
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
        auth_type: AuthType::Bearer,
        skip_api_key: false,
        model_prefix: None,
    }
}

/// Helper: local provider (no API key required)
fn def_local(
    name: &'static str,
    display_name: &'static str,
    base_url: &'static str,
) -> ProviderDefinition {
    ProviderDefinition {
        name,
        display_name,
        base_url,
        auth_env_var: "",
        auth_type: AuthType::None,
        skip_api_key: true,
        model_prefix: None,
    }
}
