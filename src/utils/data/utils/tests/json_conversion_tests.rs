use crate::utils::data::utils::DataUtils;
use serde_json::{Value, json};

// ==================== JSON Conversion Tests ====================

#[test]
fn test_json_operations() {
    let data = json!({
        "name": "test",
        "value": 42,
        "nested": {
            "inner": "data"
        }
    });

    let dict = DataUtils::convert_to_dict(&data).unwrap();
    assert!(dict.contains_key("name"));
    assert!(dict.contains_key("nested"));
}

#[test]
fn test_convert_to_dict_non_object() {
    let array = json!([1, 2, 3]);
    let result = DataUtils::convert_to_dict(&array);
    assert!(result.is_err());

    let string = json!("test");
    let result = DataUtils::convert_to_dict(&string);
    assert!(result.is_err());

    let number = json!(42);
    let result = DataUtils::convert_to_dict(&number);
    assert!(result.is_err());

    let null = json!(null);
    let result = DataUtils::convert_to_dict(&null);
    assert!(result.is_err());
}

#[test]
fn test_convert_to_dict_empty_object() {
    let empty = json!({});
    let dict = DataUtils::convert_to_dict(&empty).unwrap();
    assert!(dict.is_empty());
}

#[test]
fn test_convert_list_to_dict() {
    let list = vec![
        json!({"name": "item1"}),
        json!({"name": "item2"}),
        json!("not an object"),
        json!(123),
    ];

    let dicts = DataUtils::convert_list_to_dict(&list);
    assert_eq!(dicts.len(), 2); // Only objects are converted
    assert_eq!(dicts[0].get("name").unwrap(), &json!("item1"));
    assert_eq!(dicts[1].get("name").unwrap(), &json!("item2"));
}

#[test]
fn test_convert_list_to_dict_empty() {
    let list: Vec<Value> = vec![];
    let dicts = DataUtils::convert_list_to_dict(&list);
    assert!(dicts.is_empty());
}

#[test]
fn test_convert_list_to_dict_no_objects() {
    let list = vec![json!("string"), json!(123), json!(null)];
    let dicts = DataUtils::convert_list_to_dict(&list);
    assert!(dicts.is_empty());
}

// ==================== Jsonify Tools Tests ====================

#[test]
fn test_jsonify_tools_objects() {
    let tools = vec![
        json!({"type": "function", "name": "test1"}),
        json!({"type": "function", "name": "test2"}),
    ];

    let result = DataUtils::jsonify_tools(&tools).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].get("name").unwrap(), &json!("test1"));
}

#[test]
fn test_jsonify_tools_json_strings() {
    let tools = vec![
        json!(r#"{"type": "function", "name": "test1"}"#),
        json!(r#"{"type": "function", "name": "test2"}"#),
    ];

    let result = DataUtils::jsonify_tools(&tools).unwrap();
    assert_eq!(result.len(), 2);
}

#[test]
fn test_jsonify_tools_invalid_json_string() {
    let tools = vec![json!("not valid json")];
    let result = DataUtils::jsonify_tools(&tools);
    assert!(result.is_err());
}

#[test]
fn test_jsonify_tools_non_object_json_string() {
    let tools = vec![json!(r#"[1, 2, 3]"#)];
    let result = DataUtils::jsonify_tools(&tools);
    assert!(result.is_err());
}

#[test]
fn test_jsonify_tools_invalid_type() {
    let tools = vec![json!(123)];
    let result = DataUtils::jsonify_tools(&tools);
    assert!(result.is_err());

    let tools = vec![json!([1, 2, 3])];
    let result = DataUtils::jsonify_tools(&tools);
    assert!(result.is_err());
}

#[test]
fn test_jsonify_tools_empty() {
    let tools: Vec<Value> = vec![];
    let result = DataUtils::jsonify_tools(&tools).unwrap();
    assert!(result.is_empty());
}
