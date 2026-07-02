use crate::utils::data::utils::DataUtils;
use serde_json::json;

// ==================== JSON Merging Tests ====================

#[test]
fn test_json_merging() {
    let mut base = json!({
        "a": 1,
        "b": {
            "c": 2
        }
    });

    let overlay = json!({
        "b": {
            "d": 3
        },
        "e": 4
    });

    DataUtils::merge_json_objects(&mut base, &overlay).unwrap();

    assert_eq!(base["a"], json!(1));
    assert_eq!(base["b"]["c"], json!(2));
    assert_eq!(base["b"]["d"], json!(3));
    assert_eq!(base["e"], json!(4));
}

#[test]
fn test_json_merging_overwrite() {
    let mut base = json!({
        "key": "original"
    });

    let overlay = json!({
        "key": "overwritten"
    });

    DataUtils::merge_json_objects(&mut base, &overlay).unwrap();
    assert_eq!(base["key"], json!("overwritten"));
}

#[test]
fn test_json_merging_deep_nested() {
    let mut base = json!({
        "level1": {
            "level2": {
                "level3": {
                    "value": 1
                }
            }
        }
    });

    let overlay = json!({
        "level1": {
            "level2": {
                "level3": {
                    "new_value": 2
                }
            }
        }
    });

    DataUtils::merge_json_objects(&mut base, &overlay).unwrap();
    assert_eq!(base["level1"]["level2"]["level3"]["value"], json!(1));
    assert_eq!(base["level1"]["level2"]["level3"]["new_value"], json!(2));
}

#[test]
fn test_json_merging_non_objects() {
    let mut base = json!([1, 2, 3]);
    let overlay = json!({"key": "value"});
    assert!(DataUtils::merge_json_objects(&mut base, &overlay).is_err());

    let mut base = json!({"key": "value"});
    let overlay = json!([1, 2, 3]);
    assert!(DataUtils::merge_json_objects(&mut base, &overlay).is_err());
}

#[test]
fn test_json_merging_empty() {
    let mut base = json!({});
    let overlay = json!({"key": "value"});
    DataUtils::merge_json_objects(&mut base, &overlay).unwrap();
    assert_eq!(base["key"], json!("value"));

    let mut base = json!({"key": "value"});
    let overlay = json!({});
    DataUtils::merge_json_objects(&mut base, &overlay).unwrap();
    assert_eq!(base["key"], json!("value"));
}

// ==================== Nested Value Extraction Tests ====================

#[test]
fn test_nested_value_extraction() {
    let data = json!({
        "level1": {
            "level2": {
                "value": "found"
            }
        },
        "array": [1, 2, {"key": "value"}]
    });

    let value = DataUtils::extract_nested_value(&data, &["level1", "level2", "value"]);
    assert_eq!(value, Some(&json!("found")));

    let array_value = DataUtils::extract_nested_value(&data, &["array", "2", "key"]);
    assert_eq!(array_value, Some(&json!("value")));

    let missing = DataUtils::extract_nested_value(&data, &["missing", "path"]);
    assert_eq!(missing, None);
}

#[test]
fn test_extract_nested_value_empty_path() {
    let data = json!({"key": "value"});
    let result = DataUtils::extract_nested_value(&data, &[]);
    assert_eq!(result, Some(&data));
}

#[test]
fn test_extract_nested_value_array_index_out_of_bounds() {
    let data = json!({"array": [1, 2, 3]});
    let result = DataUtils::extract_nested_value(&data, &["array", "10"]);
    assert_eq!(result, None);
}

#[test]
fn test_extract_nested_value_invalid_array_index() {
    let data = json!({"array": [1, 2, 3]});
    let result = DataUtils::extract_nested_value(&data, &["array", "not_a_number"]);
    assert_eq!(result, None);
}

#[test]
fn test_extract_nested_value_from_primitive() {
    let data = json!("string");
    let result = DataUtils::extract_nested_value(&data, &["key"]);
    assert_eq!(result, None);
}

// ==================== Set Nested Value Tests ====================

#[test]
fn test_set_nested_value() {
    let mut data = json!({});
    DataUtils::set_nested_value(&mut data, &["a", "b", "c"], json!(123)).unwrap();
    assert_eq!(data["a"]["b"]["c"], json!(123));
}

#[test]
fn test_set_nested_value_overwrite() {
    let mut data = json!({"key": "old"});
    DataUtils::set_nested_value(&mut data, &["key"], json!("new")).unwrap();
    assert_eq!(data["key"], json!("new"));
}

#[test]
fn test_set_nested_value_empty_path() {
    let mut data = json!({});
    let result = DataUtils::set_nested_value(&mut data, &[], json!(123));
    assert!(result.is_err());
}

#[test]
fn test_set_nested_value_in_non_object() {
    let mut data = json!([1, 2, 3]);
    let result = DataUtils::set_nested_value(&mut data, &["key"], json!(123));
    assert!(result.is_err());
}

#[test]
fn test_set_nested_value_creates_intermediate() {
    let mut data = json!({});
    DataUtils::set_nested_value(&mut data, &["a", "b", "c"], json!("value")).unwrap();
    assert!(data["a"].is_object());
    assert!(data["a"]["b"].is_object());
    assert_eq!(data["a"]["b"]["c"], json!("value"));
}
