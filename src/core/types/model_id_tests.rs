use super::model_id::ModelIdRef;

#[test]
fn model_id_ref_preserves_wire_identity_and_qualifier_boundaries() {
    let qualified = ModelIdRef::parse("openai/gpt-5.5");
    assert_eq!(qualified.raw(), "openai/gpt-5.5");
    assert_eq!(qualified.provider(), Some("openai"));
    assert_eq!(qualified.model(), "gpt-5.5");
    assert_eq!(qualified.for_provider("openai"), Some("gpt-5.5"));
    assert_eq!(qualified.for_provider("anthropic"), None);

    let unqualified = ModelIdRef::parse("gpt-5.5");
    assert_eq!(unqualified.provider(), None);
    assert_eq!(unqualified.for_provider("openai"), Some("gpt-5.5"));

    let nested = ModelIdRef::parse("openai/organization/model");
    assert_eq!(nested.model(), "organization/model");

    for malformed in ["", "openai/", "/gpt-5.5"] {
        assert_eq!(ModelIdRef::parse(malformed).for_provider("openai"), None);
    }
}
