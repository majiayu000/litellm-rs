use crate::utils::data::utils::DataUtils;
use serde_json::json;

// ==================== JSON Extraction from String Tests ====================

#[test]
fn test_json_extraction_from_string() {
    let text = "Here is some JSON: {\"key\": \"value\"} and more text";
    let extracted = DataUtils::extract_json_from_string(text);
    assert_eq!(extracted, Some(json!({"key": "value"})));

    let no_json = "This has no JSON content";
    let no_extracted = DataUtils::extract_json_from_string(no_json);
    assert_eq!(no_extracted, None);
}

#[test]
fn test_json_extraction_from_string_array() {
    let text = "Array: [1, 2, 3] in text";
    let extracted = DataUtils::extract_json_from_string(text);
    assert_eq!(extracted, Some(json!([1, 2, 3])));
}

#[test]
fn test_json_extraction_from_string_nested() {
    let text = "Nested: {\"outer\": {\"inner\": \"value\"}}";
    let extracted = DataUtils::extract_json_from_string(text);
    assert_eq!(extracted, Some(json!({"outer": {"inner": "value"}})));
}

#[test]
fn test_json_extraction_from_string_empty() {
    let extracted = DataUtils::extract_json_from_string("");
    assert_eq!(extracted, None);
}
