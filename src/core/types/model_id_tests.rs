use super::model_id::ModelIdRef;

#[test]
fn model_id_ref_exposes_an_unqualified_id_losslessly() {
    let unqualified = ModelIdRef::parse("gpt-5.5");
    assert_eq!(unqualified.raw(), "gpt-5.5");
    assert_eq!(unqualified.provider(), None);
    assert_eq!(unqualified.model(), "gpt-5.5");
}

#[test]
fn model_id_ref_splits_a_single_qualifier_at_the_first_slash() {
    let qualified = ModelIdRef::parse("openai/gpt-5.5");
    assert_eq!(qualified.raw(), "openai/gpt-5.5");
    assert_eq!(qualified.provider(), Some("openai"));
    assert_eq!(qualified.model(), "gpt-5.5");
}

#[test]
fn model_id_ref_preserves_the_nested_remainder() {
    let nested = ModelIdRef::parse("openai/organization/model");
    assert_eq!(nested.provider(), Some("openai"));
    assert_eq!(nested.model(), "organization/model");

    let provider_native = ModelIdRef::parse("BAAI/bge-m3");
    assert_eq!(provider_native.raw(), "BAAI/bge-m3");
    assert_eq!(provider_native.provider(), Some("BAAI"));
    assert_eq!(provider_native.model(), "bge-m3");
}

#[test]
fn model_id_ref_exposes_empty_segments_without_validation() {
    let empty = ModelIdRef::parse("");
    assert_eq!(empty.provider(), None);
    assert_eq!(empty.model(), "");

    let empty_model = ModelIdRef::parse("openai/");
    assert_eq!(empty_model.provider(), Some("openai"));
    assert_eq!(empty_model.model(), "");

    let empty_first_segment = ModelIdRef::parse("/gpt-5.5");
    assert_eq!(empty_first_segment.provider(), Some(""));
    assert_eq!(empty_first_segment.model(), "gpt-5.5");

    let both_empty = ModelIdRef::parse("/");
    assert_eq!(both_empty.provider(), Some(""));
    assert_eq!(both_empty.model(), "");
}

#[test]
fn model_id_ref_preserves_raw_case_and_delimiters() {
    let raw = "OPENAI//Organization/Model";
    let parsed = ModelIdRef::parse(raw);
    assert_eq!(parsed.raw(), raw);
    assert_eq!(parsed.provider(), Some("OPENAI"));
    assert_eq!(parsed.model(), "/Organization/Model");
}
