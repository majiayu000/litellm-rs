//! Provider type enumeration and string conversion

/// Provider type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ProviderType {
    OpenAI,
    Anthropic,
    Bedrock,
    OpenRouter,
    VertexAI,
    Azure,
    AzureAI,
    DeepSeek,
    DeepInfra,
    V0,
    MetaLlama,
    Mistral,
    Moonshot,
    Minimax,
    Dashscope,
    Groq,
    XAI,
    Cloudflare,
    Perplexity,
    Replicate,
    FalAI,
    AmazonNova,
    GitHub,
    GitHubCopilot,
    Hyperbolic,
    Infinity,
    Novita,
    Volcengine,
    Nebius,
    Nscale,
    PydanticAI,
    OpenAICompatible,
    Custom(String),
}

impl From<&str> for ProviderType {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "openai" => ProviderType::OpenAI,
            "anthropic" => ProviderType::Anthropic,
            "bedrock" | "aws-bedrock" => ProviderType::Bedrock,
            "openrouter" => ProviderType::OpenRouter,
            "vertex_ai" | "vertexai" | "vertex-ai" => ProviderType::VertexAI,
            "azure" | "azure-openai" => ProviderType::Azure,
            "azure_ai" | "azureai" | "azure-ai" => ProviderType::AzureAI,
            "deepseek" | "deep-seek" => ProviderType::DeepSeek,
            "deepinfra" | "deep-infra" => ProviderType::DeepInfra,
            "v0" => ProviderType::V0,
            "meta_llama" | "llama" | "meta-llama" => ProviderType::MetaLlama,
            "mistral" | "mistralai" => ProviderType::Mistral,
            "moonshot" | "moonshot-ai" => ProviderType::Moonshot,
            "minimax" | "minimax-ai" => ProviderType::Minimax,
            "dashscope" | "alibaba" | "qwen" | "tongyi" => ProviderType::Dashscope,
            "groq" => ProviderType::Groq,
            "xai" => ProviderType::XAI,
            "cloudflare" | "cf" | "workers-ai" => ProviderType::Cloudflare,
            "perplexity" | "perplexity-ai" | "pplx" => ProviderType::Perplexity,
            "replicate" | "replicate-ai" => ProviderType::Replicate,
            "fal_ai" | "fal-ai" | "fal" => ProviderType::FalAI,
            "amazon_nova" | "amazon-nova" | "nova" => ProviderType::AmazonNova,
            "github" | "github-models" => ProviderType::GitHub,
            "github_copilot" | "github-copilot" | "copilot" => ProviderType::GitHubCopilot,
            "hyperbolic" | "hyperbolic-ai" => ProviderType::Hyperbolic,
            "infinity" | "infinity-embedding" => ProviderType::Infinity,
            "novita" | "novita-ai" => ProviderType::Novita,
            "volcengine" | "volc" | "doubao" | "bytedance" => ProviderType::Volcengine,
            "nebius" | "nebius-ai" => ProviderType::Nebius,
            "nscale" | "nscale-ai" => ProviderType::Nscale,
            "pydantic_ai" | "pydantic-ai" | "pydantic" => ProviderType::PydanticAI,
            "openai_compatible" | "openai-compatible" | "openai_like" | "openai-like" => {
                ProviderType::OpenAICompatible
            }
            _ => ProviderType::Custom(s.to_string()),
        }
    }
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::OpenAI => write!(f, "openai"),
            ProviderType::Anthropic => write!(f, "anthropic"),
            ProviderType::Bedrock => write!(f, "bedrock"),
            ProviderType::OpenRouter => write!(f, "openrouter"),
            ProviderType::VertexAI => write!(f, "vertex_ai"),
            ProviderType::Azure => write!(f, "azure"),
            ProviderType::AzureAI => write!(f, "azure_ai"),
            ProviderType::DeepSeek => write!(f, "deepseek"),
            ProviderType::DeepInfra => write!(f, "deepinfra"),
            ProviderType::V0 => write!(f, "v0"),
            ProviderType::MetaLlama => write!(f, "meta_llama"),
            ProviderType::Mistral => write!(f, "mistral"),
            ProviderType::Moonshot => write!(f, "moonshot"),
            ProviderType::Minimax => write!(f, "minimax"),
            ProviderType::Dashscope => write!(f, "dashscope"),
            ProviderType::Groq => write!(f, "groq"),
            ProviderType::XAI => write!(f, "xai"),
            ProviderType::Cloudflare => write!(f, "cloudflare"),
            ProviderType::Perplexity => write!(f, "perplexity"),
            ProviderType::Replicate => write!(f, "replicate"),
            ProviderType::FalAI => write!(f, "fal_ai"),
            ProviderType::AmazonNova => write!(f, "amazon_nova"),
            ProviderType::GitHub => write!(f, "github"),
            ProviderType::GitHubCopilot => write!(f, "github_copilot"),
            ProviderType::Hyperbolic => write!(f, "hyperbolic"),
            ProviderType::Infinity => write!(f, "infinity"),
            ProviderType::Novita => write!(f, "novita"),
            ProviderType::Volcengine => write!(f, "volcengine"),
            ProviderType::Nebius => write!(f, "nebius"),
            ProviderType::Nscale => write!(f, "nscale"),
            ProviderType::PydanticAI => write!(f, "pydantic_ai"),
            ProviderType::OpenAICompatible => write!(f, "openai_compatible"),
            ProviderType::Custom(name) => write!(f, "{}", name),
        }
    }
}

/// All non-custom ProviderType variants (useful for exhaustive testing).
pub fn all_non_custom_provider_types() -> Vec<ProviderType> {
    vec![
        ProviderType::OpenAI,
        ProviderType::Anthropic,
        ProviderType::Bedrock,
        ProviderType::OpenRouter,
        ProviderType::VertexAI,
        ProviderType::Azure,
        ProviderType::AzureAI,
        ProviderType::DeepSeek,
        ProviderType::DeepInfra,
        ProviderType::V0,
        ProviderType::MetaLlama,
        ProviderType::Mistral,
        ProviderType::Moonshot,
        ProviderType::Minimax,
        ProviderType::Dashscope,
        ProviderType::Groq,
        ProviderType::XAI,
        ProviderType::Cloudflare,
        ProviderType::Perplexity,
        ProviderType::Replicate,
        ProviderType::FalAI,
        ProviderType::AmazonNova,
        ProviderType::GitHub,
        ProviderType::GitHubCopilot,
        ProviderType::Hyperbolic,
        ProviderType::Infinity,
        ProviderType::Novita,
        ProviderType::Volcengine,
        ProviderType::Nebius,
        ProviderType::Nscale,
        ProviderType::PydanticAI,
        ProviderType::OpenAICompatible,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_type_from_str_openai() {
        assert_eq!(ProviderType::from("openai"), ProviderType::OpenAI);
        assert_eq!(ProviderType::from("OpenAI"), ProviderType::OpenAI);
        assert_eq!(ProviderType::from("OPENAI"), ProviderType::OpenAI);
    }

    #[test]
    fn test_provider_type_from_str_anthropic() {
        assert_eq!(ProviderType::from("anthropic"), ProviderType::Anthropic);
        assert_eq!(ProviderType::from("Anthropic"), ProviderType::Anthropic);
    }

    #[test]
    fn test_provider_type_from_str_bedrock() {
        assert_eq!(ProviderType::from("bedrock"), ProviderType::Bedrock);
        assert_eq!(ProviderType::from("aws-bedrock"), ProviderType::Bedrock);
    }

    #[test]
    fn test_provider_type_from_str_vertex_ai() {
        assert_eq!(ProviderType::from("vertex_ai"), ProviderType::VertexAI);
        assert_eq!(ProviderType::from("vertexai"), ProviderType::VertexAI);
        assert_eq!(ProviderType::from("vertex-ai"), ProviderType::VertexAI);
    }

    #[test]
    fn test_provider_type_from_str_azure() {
        assert_eq!(ProviderType::from("azure"), ProviderType::Azure);
        assert_eq!(ProviderType::from("azure-openai"), ProviderType::Azure);
    }

    #[test]
    fn test_provider_type_from_str_azure_ai() {
        assert_eq!(ProviderType::from("azure_ai"), ProviderType::AzureAI);
        assert_eq!(ProviderType::from("azureai"), ProviderType::AzureAI);
        assert_eq!(ProviderType::from("azure-ai"), ProviderType::AzureAI);
    }

    #[test]
    fn test_provider_type_from_str_deepseek() {
        assert_eq!(ProviderType::from("deepseek"), ProviderType::DeepSeek);
        assert_eq!(ProviderType::from("deep-seek"), ProviderType::DeepSeek);
    }

    #[test]
    fn test_provider_type_from_str_deepinfra() {
        assert_eq!(ProviderType::from("deepinfra"), ProviderType::DeepInfra);
        assert_eq!(ProviderType::from("deep-infra"), ProviderType::DeepInfra);
    }

    #[test]
    fn test_provider_type_from_str_meta_llama() {
        assert_eq!(ProviderType::from("meta_llama"), ProviderType::MetaLlama);
        assert_eq!(ProviderType::from("llama"), ProviderType::MetaLlama);
        assert_eq!(ProviderType::from("meta-llama"), ProviderType::MetaLlama);
    }

    #[test]
    fn test_provider_type_from_str_mistral() {
        assert_eq!(ProviderType::from("mistral"), ProviderType::Mistral);
        assert_eq!(ProviderType::from("mistralai"), ProviderType::Mistral);
    }

    #[test]
    fn test_provider_type_from_str_moonshot() {
        assert_eq!(ProviderType::from("moonshot"), ProviderType::Moonshot);
        assert_eq!(ProviderType::from("moonshot-ai"), ProviderType::Moonshot);
    }

    #[test]
    fn test_provider_type_from_str_cloudflare() {
        assert_eq!(ProviderType::from("cloudflare"), ProviderType::Cloudflare);
        assert_eq!(ProviderType::from("cf"), ProviderType::Cloudflare);
        assert_eq!(ProviderType::from("workers-ai"), ProviderType::Cloudflare);
    }

    #[test]
    fn test_provider_type_from_str_other_providers() {
        assert_eq!(ProviderType::from("openrouter"), ProviderType::OpenRouter);
        assert_eq!(ProviderType::from("groq"), ProviderType::Groq);
        assert_eq!(ProviderType::from("xai"), ProviderType::XAI);
        assert_eq!(ProviderType::from("v0"), ProviderType::V0);
    }

    #[test]
    fn test_provider_type_from_str_custom() {
        assert_eq!(
            ProviderType::from("custom-provider"),
            ProviderType::Custom("custom-provider".to_string())
        );
        assert_eq!(
            ProviderType::from("my-local-llm"),
            ProviderType::Custom("my-local-llm".to_string())
        );
    }

    #[test]
    fn test_provider_type_display() {
        assert_eq!(format!("{}", ProviderType::OpenAI), "openai");
        assert_eq!(format!("{}", ProviderType::Anthropic), "anthropic");
        assert_eq!(format!("{}", ProviderType::Bedrock), "bedrock");
        assert_eq!(format!("{}", ProviderType::OpenRouter), "openrouter");
        assert_eq!(format!("{}", ProviderType::VertexAI), "vertex_ai");
        assert_eq!(format!("{}", ProviderType::Azure), "azure");
        assert_eq!(format!("{}", ProviderType::AzureAI), "azure_ai");
        assert_eq!(format!("{}", ProviderType::DeepSeek), "deepseek");
        assert_eq!(format!("{}", ProviderType::DeepInfra), "deepinfra");
        assert_eq!(format!("{}", ProviderType::V0), "v0");
        assert_eq!(format!("{}", ProviderType::MetaLlama), "meta_llama");
        assert_eq!(format!("{}", ProviderType::Mistral), "mistral");
        assert_eq!(format!("{}", ProviderType::Moonshot), "moonshot");
        assert_eq!(format!("{}", ProviderType::Groq), "groq");
        assert_eq!(format!("{}", ProviderType::XAI), "xai");
        assert_eq!(format!("{}", ProviderType::Cloudflare), "cloudflare");
    }

    #[test]
    fn test_provider_type_display_custom() {
        let custom = ProviderType::Custom("my-custom-provider".to_string());
        assert_eq!(format!("{}", custom), "my-custom-provider");
    }

    #[test]
    fn test_provider_type_clone() {
        let original = ProviderType::OpenAI;
        let cloned = original.clone();
        assert_eq!(original, cloned);

        let custom = ProviderType::Custom("test".to_string());
        let custom_cloned = custom.clone();
        assert_eq!(custom, custom_cloned);
    }

    #[test]
    fn test_provider_type_equality() {
        assert_eq!(ProviderType::OpenAI, ProviderType::OpenAI);
        assert_ne!(ProviderType::OpenAI, ProviderType::Anthropic);
        assert_eq!(
            ProviderType::Custom("test".to_string()),
            ProviderType::Custom("test".to_string())
        );
        assert_ne!(
            ProviderType::Custom("test1".to_string()),
            ProviderType::Custom("test2".to_string())
        );
    }

    #[test]
    fn test_provider_type_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(ProviderType::OpenAI);
        set.insert(ProviderType::Anthropic);
        set.insert(ProviderType::Custom("custom".to_string()));

        assert!(set.contains(&ProviderType::OpenAI));
        assert!(set.contains(&ProviderType::Anthropic));
        assert!(set.contains(&ProviderType::Custom("custom".to_string())));
        assert!(!set.contains(&ProviderType::Bedrock));
    }

    #[test]
    fn test_provider_type_serialization() {
        let provider = ProviderType::OpenAI;
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(json, "\"OpenAI\"");

        let custom = ProviderType::Custom("my-provider".to_string());
        let custom_json = serde_json::to_string(&custom).unwrap();
        assert!(custom_json.contains("Custom"));
        assert!(custom_json.contains("my-provider"));
    }

    #[test]
    fn test_provider_type_deserialization() {
        let provider: ProviderType = serde_json::from_str("\"OpenAI\"").unwrap();
        assert_eq!(provider, ProviderType::OpenAI);

        let anthropic: ProviderType = serde_json::from_str("\"Anthropic\"").unwrap();
        assert_eq!(anthropic, ProviderType::Anthropic);
    }

    #[test]
    fn test_provider_type_roundtrip_serialization() {
        let providers = vec![
            ProviderType::OpenAI,
            ProviderType::Anthropic,
            ProviderType::Bedrock,
            ProviderType::Custom("test".to_string()),
        ];

        for provider in providers {
            let json = serde_json::to_string(&provider).unwrap();
            let deserialized: ProviderType = serde_json::from_str(&json).unwrap();
            assert_eq!(provider, deserialized);
        }
    }

    #[test]
    fn test_provider_type_debug() {
        let provider = ProviderType::OpenAI;
        let debug_str = format!("{:?}", provider);
        assert_eq!(debug_str, "OpenAI");

        let custom = ProviderType::Custom("test".to_string());
        let custom_debug = format!("{:?}", custom);
        assert!(custom_debug.contains("Custom"));
        assert!(custom_debug.contains("test"));
    }

    #[test]
    fn test_provider_type_from_display_consistency() {
        for provider in all_non_custom_provider_types() {
            let display = format!("{}", provider);
            let parsed = ProviderType::from(display.as_str());
            assert_eq!(
                provider, parsed,
                "Display/From roundtrip failed for {:?}",
                provider
            );
        }
    }

    #[test]
    fn test_provider_type_all_variants_covered() {
        for provider in all_non_custom_provider_types() {
            let provider_str = provider.to_string();
            let provider_type = ProviderType::from(provider_str.as_str());
            assert!(
                !matches!(provider_type, ProviderType::Custom(_)),
                "Provider '{}' should not be Custom",
                provider_str
            );
            assert_eq!(
                provider_type, provider,
                "Expected '{}' to map to {:?}, but got {:?}",
                provider_str, provider, provider_type
            );
        }
    }

    #[test]
    fn test_provider_type_case_insensitive() {
        let cases = vec![
            ("OPENAI", ProviderType::OpenAI),
            ("OpenAI", ProviderType::OpenAI),
            ("openai", ProviderType::OpenAI),
            ("OpenAi", ProviderType::OpenAI),
            ("ANTHROPIC", ProviderType::Anthropic),
            ("Anthropic", ProviderType::Anthropic),
            ("GROQ", ProviderType::Groq),
            ("Groq", ProviderType::Groq),
        ];

        for (input, expected) in cases {
            assert_eq!(
                ProviderType::from(input),
                expected,
                "Case-insensitive parsing failed for '{}'",
                input
            );
        }
    }

    #[test]
    fn test_provider_type_openai_like_aliases() {
        assert_eq!(
            ProviderType::from("openai_like"),
            ProviderType::OpenAICompatible
        );
        assert_eq!(
            ProviderType::from("openai-like"),
            ProviderType::OpenAICompatible
        );
    }
}
