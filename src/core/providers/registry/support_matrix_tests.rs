use super::{
    DEFAULT_CATALOG_RUNTIME_PROVIDERS, PROVIDER_CATALOG, ProviderRouteSurface, canonical_selector,
    provider_type_registry, selector_has_matrix_entry, support_state_for_surface,
    supports_provider_surface,
};
use crate::core::providers::registry::{AuthType, get_definition};

#[test]
fn support_matrix_covers_registry_and_catalog_selectors() {
    for entry in provider_type_registry() {
        assert!(
            selector_has_matrix_entry(entry.canonical_name),
            "missing support matrix row for {}",
            entry.canonical_name
        );
    }

    for selector in PROVIDER_CATALOG.keys() {
        assert!(
            selector_has_matrix_entry(selector),
            "missing support matrix fallback for catalog selector {selector}"
        );
    }
}

#[test]
fn default_completion_catalog_routes_are_marked_supported() {
    for selector in DEFAULT_CATALOG_RUNTIME_PROVIDERS {
        assert!(
            supports_provider_surface(selector, ProviderRouteSurface::CompletionChat),
            "{selector} should support completion() chat"
        );
        assert!(
            supports_provider_surface(selector, ProviderRouteSurface::CompletionChatStream),
            "{selector} should support completion() streaming"
        );
    }
}

#[test]
fn sdk_matrix_rejects_google_chat_until_adapter_exists() {
    assert!(supports_provider_surface(
        "openai",
        ProviderRouteSurface::SdkChat
    ));
    assert!(supports_provider_surface(
        "anthropic",
        ProviderRouteSurface::SdkChatStream
    ));
    assert!(!supports_provider_surface(
        "google",
        ProviderRouteSurface::SdkChat
    ));
    assert!(!supports_provider_surface(
        "gemini",
        ProviderRouteSurface::SdkChat
    ));
}

#[test]
fn completion_matrix_matches_default_router_support() {
    for surface in [
        ProviderRouteSurface::CompletionChat,
        ProviderRouteSurface::CompletionChatStream,
    ] {
        assert!(supports_provider_surface("azure", surface));
        assert!(!supports_provider_surface("bedrock", surface));
    }
    assert_eq!(
        supports_provider_surface("azure_ai", ProviderRouteSurface::CompletionChat),
        cfg!(feature = "providers-extra")
    );
}

#[test]
fn catalog_fallback_is_http_chat_only() {
    assert!(supports_provider_surface(
        "cerebras",
        ProviderRouteSurface::HttpChat
    ));
    assert!(supports_provider_surface(
        "cerebras",
        ProviderRouteSurface::HttpChatStream
    ));
    assert!(!supports_provider_surface(
        "cerebras",
        ProviderRouteSurface::CompletionChat
    ));
    assert!(!supports_provider_surface(
        "cerebras",
        ProviderRouteSurface::SdkChat
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
        assert!(supports_provider_surface(
            canonical,
            ProviderRouteSurface::HttpChat
        ));
        assert!(supports_provider_surface(
            canonical,
            ProviderRouteSurface::HttpChatStream
        ));
        for unsupported in [
            ProviderRouteSurface::HttpEmbeddings,
            ProviderRouteSurface::HttpImageGeneration,
            ProviderRouteSurface::SdkChat,
            ProviderRouteSurface::SdkChatStream,
            ProviderRouteSurface::SdkEmbeddings,
            ProviderRouteSurface::CompletionChat,
            ProviderRouteSurface::CompletionChatStream,
        ] {
            assert!(!supports_provider_surface(canonical, unsupported));
        }
        for alias in aliases {
            assert_eq!(canonical_selector(alias), canonical);
            assert_eq!(
                support_state_for_surface(alias, ProviderRouteSurface::HttpChat),
                support_state_for_surface(canonical, ProviderRouteSurface::HttpChat)
            );
        }
    }

    for wrong in ["ai-21", "huggingface_inference", "base-ten", "unknown"] {
        assert!(
            !selector_has_matrix_entry(wrong),
            "{wrong} must fail closed"
        );
    }
}
