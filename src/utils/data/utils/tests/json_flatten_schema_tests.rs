use crate::utils::data::utils::DataUtils;
use serde_json::json;

// ==================== JSON Flattening Tests ====================

#[test]
fn test_json_flattening() {
    let data = json!({
        "a": 1,
        "b": {
            "c": 2,
            "d": {
                "e": 3
            }
        },
        "f": [1, 2, 3]
    });

    let flattened = DataUtils::flatten_json(&data, None);
    assert_eq!(flattened.get("a"), Some(&json!(1)));
    assert_eq!(flattened.get("b.c"), Some(&json!(2)));
    assert_eq!(flattened.get("b.d.e"), Some(&json!(3)));
    assert_eq!(flattened.get("f.0"), Some(&json!(1)));
}

#[test]
fn test_json_flattening_with_prefix() {
    let data = json!({"key": "value"});
    let flattened = DataUtils::flatten_json(&data, Some("prefix".to_string()));
    assert_eq!(flattened.get("prefix.key"), Some(&json!("value")));
}

#[test]
fn test_json_flattening_empty_object() {
    let data = json!({});
    let flattened = DataUtils::flatten_json(&data, None);
    assert!(flattened.is_empty());
}

#[test]
fn test_json_flattening_primitive() {
    let data = json!("string");
    let flattened = DataUtils::flatten_json(&data, Some("key".to_string()));
    assert_eq!(flattened.get("key"), Some(&json!("string")));
}

#[test]
fn test_json_flattening_array_only() {
    let data = json!([1, 2, 3]);
    let flattened = DataUtils::flatten_json(&data, None);
    assert_eq!(flattened.get("0"), Some(&json!(1)));
    assert_eq!(flattened.get("1"), Some(&json!(2)));
    assert_eq!(flattened.get("2"), Some(&json!(3)));
}

#[test]
fn test_json_flattening_nested_arrays() {
    let data = json!({
        "arr": [[1, 2], [3, 4]]
    });
    let flattened = DataUtils::flatten_json(&data, None);
    assert_eq!(flattened.get("arr.0.0"), Some(&json!(1)));
    assert_eq!(flattened.get("arr.0.1"), Some(&json!(2)));
    assert_eq!(flattened.get("arr.1.0"), Some(&json!(3)));
    assert_eq!(flattened.get("arr.1.1"), Some(&json!(4)));
}

// ==================== JSON Schema Validation Tests ====================

#[test]
fn test_json_schema_validation() {
    let data = json!({
        "name": "test",
        "age": 25
    });

    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "number"}
        },
        "required": ["name"]
    });

    assert!(DataUtils::validate_json_schema(&data, &schema).is_ok());

    let invalid_data = json!({
        "age": "not a number"
    });

    assert!(DataUtils::validate_json_schema(&invalid_data, &schema).is_err());
}

#[test]
fn test_json_schema_validation_missing_required() {
    let data = json!({
        "age": 25
    });

    let schema = json!({
        "type": "object",
        "properties": {
            "name": {"type": "string"},
            "age": {"type": "number"}
        },
        "required": ["name"]
    });

    assert!(DataUtils::validate_json_schema(&data, &schema).is_err());
}

#[test]
fn test_json_schema_validation_type_mismatch() {
    let data = json!("string");
    let schema = json!({"type": "number"});
    assert!(DataUtils::validate_json_schema(&data, &schema).is_err());

    let data = json!(123);
    let schema = json!({"type": "string"});
    assert!(DataUtils::validate_json_schema(&data, &schema).is_err());

    let data = json!([]);
    let schema = json!({"type": "object"});
    assert!(DataUtils::validate_json_schema(&data, &schema).is_err());
}

#[test]
fn test_json_schema_validation_all_types() {
    assert!(DataUtils::validate_json_schema(&json!(null), &json!({"type": "null"})).is_ok());
    assert!(DataUtils::validate_json_schema(&json!(true), &json!({"type": "boolean"})).is_ok());
    assert!(DataUtils::validate_json_schema(&json!(123), &json!({"type": "number"})).is_ok());
    assert!(DataUtils::validate_json_schema(&json!("test"), &json!({"type": "string"})).is_ok());
    assert!(DataUtils::validate_json_schema(&json!([]), &json!({"type": "array"})).is_ok());
    assert!(DataUtils::validate_json_schema(&json!({}), &json!({"type": "object"})).is_ok());
}

#[test]
fn test_json_schema_validation_nested() {
    let data = json!({
        "user": {
            "name": "test",
            "email": "test@example.com"
        }
    });

    let schema = json!({
        "type": "object",
        "properties": {
            "user": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "email": {"type": "string"}
                }
            }
        }
    });

    assert!(DataUtils::validate_json_schema(&data, &schema).is_ok());
}

#[test]
fn test_json_schema_validation_no_schema_type() {
    let data = json!({"key": "value"});
    let schema = json!({});
    assert!(DataUtils::validate_json_schema(&data, &schema).is_ok());
}

#[test]
fn test_json_schema_validation_non_object_schema() {
    let data = json!({"key": "value"});
    let schema = json!("not a schema");
    assert!(DataUtils::validate_json_schema(&data, &schema).is_ok());
}
