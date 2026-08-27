use super::get_openai_registry;

#[test]
fn registry_resolves_only_exact_openai_catalog_identities() {
    let registry = get_openai_registry();

    for wire_id in ["gpt-5.5", "openai/gpt-5.5"] {
        let model = registry
            .resolve_model(wire_id)
            .unwrap_or_else(|| panic!("{wire_id} should resolve exactly"));
        assert_eq!(model.wire_id(), wire_id);
        assert_eq!(model.public_id(), "gpt-5.5");
        assert_eq!(model.catalog_id(), "gpt-5.5");
    }

    let snapshot = registry
        .resolve_model("openai/gpt-5.5-2026-04-23")
        .expect("an explicitly catalogued snapshot should resolve");
    assert_eq!(snapshot.wire_id(), "openai/gpt-5.5-2026-04-23");
    assert_eq!(snapshot.canonical_base_id(), Some("gpt-5.5"));
}

#[test]
fn registry_preserves_exact_already_qualified_catalog_keys() {
    let registry = get_openai_registry();
    let resolved = registry
        .resolve_model("openai/container")
        .expect("an exact already-qualified OpenAI catalog key should resolve");

    assert_eq!(resolved.wire_id(), "openai/container");
    assert_eq!(resolved.public_id(), "container");
    assert_eq!(resolved.catalog_id(), "openai/container");

    let mixed_provider = registry
        .resolve_model("OPENAI/container")
        .expect("only the provider segment should be case-insensitive");
    assert_eq!(mixed_provider.wire_id(), "OPENAI/container");
    assert_eq!(mixed_provider.public_id(), "container");
    assert_eq!(mixed_provider.catalog_id(), "openai/container");

    assert!(registry.resolve_model("OPENAI/Container").is_none());
}

#[test]
fn every_slash_bearing_exact_catalog_key_resolves_to_itself() {
    let registry = get_openai_registry();
    let slash_bearing_ids: Vec<_> = registry
        .get_all_models()
        .into_iter()
        .map(|model| model.id)
        .filter(|model_id| model_id.contains('/'))
        .collect();

    assert!(
        !slash_bearing_ids.is_empty(),
        "the OpenAI catalog should exercise slash-bearing exact keys"
    );
    for model_id in slash_bearing_ids {
        let resolved = registry
            .resolve_model(&model_id)
            .unwrap_or_else(|| panic!("exact catalog key {model_id} should resolve"));
        assert_eq!(resolved.wire_id(), model_id);
        assert_eq!(resolved.catalog_id(), model_id);
    }
}

#[test]
fn registry_rejects_provider_native_and_non_exact_openai_ids() {
    let registry = get_openai_registry();

    for wire_id in [
        "azure/gpt-5.5",
        "azure_ai/gpt-5.5",
        "azure_ai/deployments/team/model",
        "anthropic/gpt-5.5",
        "openai/gpt-future-unknown",
        "openai/gpt-5.5-2026-08-01",
        "openai/gpt-5.50",
        "openai/gpt-5.5-prologue",
        "openai/ft:gpt-5.5:org:custom",
        "gpt-5.5-2026-08-01",
        "gpt-5.50",
        "gpt-5.5-prologue",
        "ft:gpt-5.5:org:custom",
        "openai/",
        "/gpt-5.5",
    ] {
        assert!(
            registry.resolve_model(wire_id).is_none(),
            "{wire_id} must not resolve without an exact OpenAI identity"
        );
    }
}
