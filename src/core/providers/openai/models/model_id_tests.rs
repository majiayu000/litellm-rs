use super::{OpenAIModelResolution, get_openai_registry};

#[test]
fn provider_policy_resolves_only_exact_openai_catalog_identities() {
    let registry = get_openai_registry();

    for wire_id in ["azure/gpt-5.5", "azure_ai/gpt-5.5"] {
        let OpenAIModelResolution::Resolved(model) = registry.resolve_model_policy(wire_id) else {
            panic!("{wire_id} should resolve to an exact OpenAI catalog identity");
        };
        assert_eq!(model.wire_id(), wire_id);
        assert_eq!(model.public_id(), "gpt-5.5");
        assert_eq!(model.catalog_id(), "gpt-5.5");
    }

    for wire_id in [
        "openai/gpt-future-unknown",
        "azure/gpt-5.5-2026-08-01",
        "azure_ai/gpt-5.5-2026-08-01",
    ] {
        let OpenAIModelResolution::ExplicitOpenAIUnknown {
            wire_id: unresolved_wire_id,
            public_id,
        } = registry.resolve_model_policy(wire_id)
        else {
            panic!("{wire_id} should be an explicit unknown OpenAI identity");
        };
        assert_eq!(unresolved_wire_id, wire_id);
        assert!(public_id.starts_with("gpt-"));
    }

    for wire_id in ["azure_ai/Phi-4", "anthropic/gpt-5.5"] {
        assert!(matches!(
            registry.resolve_model_policy(wire_id),
            OpenAIModelResolution::NotApplicable
        ));
    }
}
