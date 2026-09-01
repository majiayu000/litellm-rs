use super::{
    DEFAULT_CATALOG_RUNTIME_PROVIDERS, LegacyAdapterSurface, PROVIDER_CATALOG, canonical_selector,
    legacy_adapter_availability, provider_type_registry, selector_has_legacy_adapter_entry,
    supports_legacy_adapter,
};
use crate::core::providers::registry::{AuthType, get_definition};

#[test]
fn legacy_adapter_matrix_is_distinct_from_canonical_runtime_capability() {
    assert!(!supports_legacy_adapter(
        "bedrock",
        LegacyAdapterSurface::CompletionChat
    ));

    let readme = include_str!("../../../../README.md")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(readme.contains("### Legacy adapter matrix"));
    assert!(readme.contains(
        "Canonical runtime support is derived from the selected deployment's `ProviderCapability`"
    ));
}

#[test]
fn legacy_adapter_matrix_covers_registry_and_catalog_selectors() {
    for entry in provider_type_registry() {
        assert!(
            selector_has_legacy_adapter_entry(entry.canonical_name),
            "missing legacy adapter matrix row for {}",
            entry.canonical_name
        );
    }

    for selector in PROVIDER_CATALOG.keys() {
        assert!(
            selector_has_legacy_adapter_entry(selector),
            "missing legacy adapter matrix fallback for catalog selector {selector}"
        );
    }
}

#[test]
fn default_catalog_legacy_completion_adapters_are_marked_supported() {
    for selector in DEFAULT_CATALOG_RUNTIME_PROVIDERS {
        assert!(
            supports_legacy_adapter(selector, LegacyAdapterSurface::CompletionChat),
            "{selector} should have a legacy completion() chat adapter"
        );
        assert!(
            supports_legacy_adapter(selector, LegacyAdapterSurface::CompletionChatStream),
            "{selector} should have a legacy completion() streaming adapter"
        );
    }
}

#[test]
fn legacy_sdk_adapter_matrix_rejects_google_chat_until_adapter_exists() {
    assert!(supports_legacy_adapter(
        "openai",
        LegacyAdapterSurface::SdkChat
    ));
    assert!(supports_legacy_adapter(
        "anthropic",
        LegacyAdapterSurface::SdkChatStream
    ));
    assert!(!supports_legacy_adapter(
        "google",
        LegacyAdapterSurface::SdkChat
    ));
    assert!(!supports_legacy_adapter(
        "gemini",
        LegacyAdapterSurface::SdkChat
    ));
}

#[test]
fn legacy_completion_adapter_matrix_records_pre_runtime_routes() {
    for surface in [
        LegacyAdapterSurface::CompletionChat,
        LegacyAdapterSurface::CompletionChatStream,
    ] {
        assert!(supports_legacy_adapter("azure", surface));
        assert!(!supports_legacy_adapter("bedrock", surface));
    }
    assert_eq!(
        supports_legacy_adapter("azure_ai", LegacyAdapterSurface::CompletionChat),
        cfg!(feature = "providers-extra")
    );
}

#[test]
fn catalog_legacy_adapter_fallback_is_http_chat_only() {
    assert!(supports_legacy_adapter(
        "cerebras",
        LegacyAdapterSurface::HttpChat
    ));
    assert!(supports_legacy_adapter(
        "cerebras",
        LegacyAdapterSurface::HttpChatStream
    ));
    assert!(!supports_legacy_adapter(
        "cerebras",
        LegacyAdapterSurface::CompletionChat
    ));
    assert!(!supports_legacy_adapter(
        "cerebras",
        LegacyAdapterSurface::SdkChat
    ));
}

#[test]
fn enterprise_rerank_routes_are_declared_explicitly() {
    assert!(supports_legacy_adapter(
        "oci",
        LegacyAdapterSurface::HttpRerank
    ));
    assert!(supports_legacy_adapter(
        "watsonx",
        LegacyAdapterSurface::HttpRerank
    ));
    assert!(supports_legacy_adapter(
        "voyage",
        LegacyAdapterSurface::HttpRerank
    ));
    assert!(!supports_legacy_adapter(
        "sagemaker",
        LegacyAdapterSurface::HttpRerank
    ));
    assert!(!supports_legacy_adapter(
        "cerebras",
        LegacyAdapterSurface::HttpRerank
    ));
}

#[test]
fn selector_aliases_resolve_to_canonical_matrix_entries() {
    assert_eq!(canonical_selector("azure-openai"), "azure");
    assert_eq!(canonical_selector("google_vertex"), "vertex_ai");
    assert_eq!(canonical_selector("aws_bedrock"), "bedrock");
    assert_eq!(canonical_selector("openai-like"), "openai_compatible");
    assert_eq!(canonical_selector("together-ai"), "together_ai");
    assert_eq!(canonical_selector("fireworks-ai"), "fireworks_ai");
    assert_eq!(canonical_selector("aiml-api"), "aiml_api");
    assert_eq!(canonical_selector("zhipuai"), "zhipu");
    assert_eq!(canonical_selector("zai"), "zai");
}

#[test]
fn missing_text_provider_selectors_are_exact_http_only_catalog_routes() {
    let cases = [
        (
            "ai21",
            "https://api.ai21.com/studio/v1",
            "AI21_API_KEY",
            &["ai21_chat", "ai21-chat"][..],
        ),
        (
            "huggingface",
            "https://router.huggingface.co/v1",
            "HF_TOKEN",
            &["hugging_face", "hugging-face"][..],
        ),
        (
            "baseten",
            "https://inference.baseten.co/v1",
            "BASETEN_API_KEY",
            &[][..],
        ),
    ];

    for (canonical, endpoint, env, aliases) in cases {
        let definition = get_definition(canonical).expect("catalog definition should exist");
        assert_eq!(definition.base_url, endpoint);
        assert_eq!(definition.auth_env_var, env);
        assert_eq!(definition.auth_type, AuthType::Bearer);
        assert!(definition.alternate_auth_env_vars.is_empty());
        assert!(!definition.skip_api_key);
        assert_eq!(definition.model_prefix, None);
        assert!(supports_legacy_adapter(
            canonical,
            LegacyAdapterSurface::HttpChat
        ));
        assert!(supports_legacy_adapter(
            canonical,
            LegacyAdapterSurface::HttpChatStream
        ));
        for unsupported in [
            LegacyAdapterSurface::HttpEmbeddings,
            LegacyAdapterSurface::HttpRerank,
            LegacyAdapterSurface::HttpImageGeneration,
            LegacyAdapterSurface::SdkChat,
            LegacyAdapterSurface::SdkChatStream,
            LegacyAdapterSurface::SdkEmbeddings,
            LegacyAdapterSurface::CompletionChat,
            LegacyAdapterSurface::CompletionChatStream,
        ] {
            assert!(!supports_legacy_adapter(canonical, unsupported));
        }
        for alias in aliases {
            assert_eq!(canonical_selector(alias), canonical);
            assert_eq!(
                legacy_adapter_availability(alias, LegacyAdapterSurface::HttpChat),
                legacy_adapter_availability(canonical, LegacyAdapterSurface::HttpChat)
            );
        }
    }

    for wrong in ["ai-21", "huggingface_inference", "base-ten", "unknown"] {
        assert!(
            !selector_has_legacy_adapter_entry(wrong),
            "{wrong} must fail closed"
        );
    }
}
