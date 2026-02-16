#[cfg(test)]
use crate::utils::data::utils::DataUtils;
use serde_json::{Map, Value, json};
use uuid::Uuid;

// ==================== Base64 Tests ====================

#[test]
fn test_base64_operations() {
    let original = "Hello, World!";
    let encoded = DataUtils::encode_base64(original);
    assert!(DataUtils::is_base64_encoded(&encoded));

    let decoded = DataUtils::decode_base64(&encoded).unwrap();
    assert_eq!(decoded, original);

    assert!(!DataUtils::is_base64_encoded("not base64!"));
}

#[test]
fn test_base64_empty_string() {
    let empty = "";
    let encoded = DataUtils::encode_base64(empty);
    let decoded = DataUtils::decode_base64(&encoded).unwrap();
    assert_eq!(decoded, empty);
}

#[test]
fn test_base64_special_characters() {
    let special = "Hello\nWorld\t🌍";
    let encoded = DataUtils::encode_base64(special);
    let decoded = DataUtils::decode_base64(&encoded).unwrap();
    assert_eq!(decoded, special);
}

#[test]
fn test_get_base64_string() {
    let plain = "Hello";
    let result = DataUtils::get_base64_string(plain);
    // Returns empty if not base64
    assert!(result.is_empty() || DataUtils::is_base64_encoded(&result));
}

#[test]
fn test_get_base64_string_with_valid_base64() {
    let encoded = DataUtils::encode_base64("test");
    let result = DataUtils::get_base64_string(&encoded);
    assert_eq!(result, encoded);
}

#[test]
fn test_decode_base64_invalid() {
    let invalid = "not-valid-base64!!!";
    let result = DataUtils::decode_base64(invalid);
    assert!(result.is_err());
}

#[test]
fn test_base64_binary_data() {
    let binary = "\x00\x01\x02\x7F";
    let encoded = DataUtils::encode_base64(binary);
    let decoded = DataUtils::decode_base64(&encoded).unwrap();
    assert_eq!(decoded, binary);
}

#[test]
fn test_base64_long_string() {
    let long_string = "A".repeat(10000);
    let encoded = DataUtils::encode_base64(&long_string);
    assert!(DataUtils::is_base64_encoded(&encoded));
    let decoded = DataUtils::decode_base64(&encoded).unwrap();
    assert_eq!(decoded, long_string);
}

#[test]
fn test_is_base64_edge_cases() {
    // Very short strings
    assert!(!DataUtils::is_base64_encoded("a"));
    assert!(!DataUtils::is_base64_encoded("ab"));

    // Valid base64 padding
    let valid = DataUtils::encode_base64("a"); // Should be "YQ=="
    assert!(DataUtils::is_base64_encoded(&valid));
}

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

// ==================== Cleanup None Values Tests ====================

#[test]
fn test_cleanup_none_values() {
    let mut map = Map::new();
    map.insert("key1".to_string(), json!("value1"));
    map.insert("key2".to_string(), json!(null));
    map.insert("key3".to_string(), json!(123));

    DataUtils::cleanup_none_values(&mut map);

    assert_eq!(map.len(), 2);
    assert!(map.contains_key("key1"));
    assert!(!map.contains_key("key2"));
    assert!(map.contains_key("key3"));
}

#[test]
fn test_cleanup_none_values_all_null() {
    let mut map = Map::new();
    map.insert("a".to_string(), json!(null));
    map.insert("b".to_string(), json!(null));

    DataUtils::cleanup_none_values(&mut map);
    assert!(map.is_empty());
}

#[test]
fn test_cleanup_none_values_no_null() {
    let mut map = Map::new();
    map.insert("a".to_string(), json!("value"));
    map.insert("b".to_string(), json!(123));

    DataUtils::cleanup_none_values(&mut map);
    assert_eq!(map.len(), 2);
}

#[test]
fn test_deep_cleanup_none_values() {
    let mut data = json!({
        "key1": "value1",
        "key2": null,
        "nested": {
            "inner1": "value",
            "inner2": null,
            "deeper": {
                "deep1": null,
                "deep2": "keep"
            }
        },
        "array": [1, null, {"a": null, "b": "value"}]
    });

    DataUtils::deep_cleanup_none_values(&mut data);

    assert!(data.get("key1").is_some());
    assert!(data.get("key2").is_none());
    assert!(data["nested"].get("inner1").is_some());
    assert!(data["nested"].get("inner2").is_none());
    assert!(data["nested"]["deeper"].get("deep1").is_none());
    assert!(data["nested"]["deeper"].get("deep2").is_some());
}

#[test]
fn test_deep_cleanup_none_values_array() {
    let mut data = json!([
        {"a": 1, "b": null},
        {"c": null, "d": 2}
    ]);

    DataUtils::deep_cleanup_none_values(&mut data);

    assert!(data[0].get("a").is_some());
    assert!(data[0].get("b").is_none());
    assert!(data[1].get("c").is_none());
    assert!(data[1].get("d").is_some());
}

#[test]
fn test_deep_cleanup_none_values_primitive() {
    let mut data = json!("string");
    DataUtils::deep_cleanup_none_values(&mut data);
    assert_eq!(data, json!("string"));

    let mut data = json!(123);
    DataUtils::deep_cleanup_none_values(&mut data);
    assert_eq!(data, json!(123));
}

// ==================== UUID Tests ====================

#[test]
fn test_uuid_generation() {
    let uuid1 = DataUtils::generate_uuid();
    let uuid2 = DataUtils::generate_uuid();
    assert_ne!(uuid1, uuid2);
    assert!(Uuid::parse_str(&uuid1).is_ok());

    let short_id = DataUtils::generate_short_id();
    assert_eq!(short_id.len(), 8);
}

#[test]
fn test_uuid_format() {
    let uuid = DataUtils::generate_uuid();
    // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
    assert_eq!(uuid.len(), 36);
    assert_eq!(uuid.chars().nth(8), Some('-'));
    assert_eq!(uuid.chars().nth(13), Some('-'));
    assert_eq!(uuid.chars().nth(18), Some('-'));
    assert_eq!(uuid.chars().nth(23), Some('-'));
}

#[test]
fn test_short_id_uniqueness() {
    let mut ids = std::collections::HashSet::new();
    for _ in 0..100 {
        let id = DataUtils::generate_short_id();
        assert!(ids.insert(id), "Short IDs should be unique");
    }
}

#[test]
fn test_short_id_alphanumeric() {
    let id = DataUtils::generate_short_id();
    assert!(id.chars().all(|c| c.is_alphanumeric() || c == '-'));
}

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

// ==================== String Utilities Tests ====================

#[test]
fn test_string_utilities() {
    assert_eq!(
        DataUtils::truncate_string("Hello, World!", 10),
        "Hello, ..."
    );
    assert_eq!(DataUtils::truncate_string("Short", 10), "Short");

    assert_eq!(
        DataUtils::clean_whitespace("  Hello   world  "),
        "Hello world"
    );

    assert_eq!(DataUtils::word_count("Hello world test"), 3);
    assert_eq!(DataUtils::character_count("Hello 🌍"), 7);
}

#[test]
fn test_truncate_string_exact_length() {
    assert_eq!(DataUtils::truncate_string("Hello", 5), "Hello");
}

#[test]
fn test_truncate_string_one_over() {
    assert_eq!(DataUtils::truncate_string("Hello!", 5), "He...");
}

#[test]
fn test_truncate_string_empty() {
    assert_eq!(DataUtils::truncate_string("", 10), "");
}

#[test]
fn test_truncate_string_very_short_limit() {
    assert_eq!(DataUtils::truncate_string("Hello", 3), "...");
}

#[test]
fn test_clean_whitespace_multiple_spaces() {
    assert_eq!(DataUtils::clean_whitespace("  a    b     c  "), "a b c");
}

#[test]
fn test_clean_whitespace_tabs_and_newlines() {
    assert_eq!(
        DataUtils::clean_whitespace("  hello\t\nworld  "),
        "hello world"
    );
}

#[test]
fn test_word_count_empty() {
    assert_eq!(DataUtils::word_count(""), 0);
}

#[test]
fn test_word_count_only_spaces() {
    assert_eq!(DataUtils::word_count("     "), 0);
}

#[test]
fn test_word_count_single_word() {
    assert_eq!(DataUtils::word_count("hello"), 1);
}

#[test]
fn test_character_count_empty() {
    assert_eq!(DataUtils::character_count(""), 0);
}

#[test]
fn test_character_count_unicode() {
    assert_eq!(DataUtils::character_count("你好世界"), 4);
    assert_eq!(DataUtils::character_count("🎉🎊🎁"), 3);
}

#[test]
fn test_sanitize_for_json() {
    let input = "Hello\n\"World\"\t\\test";
    let sanitized = DataUtils::sanitize_for_json(input);
    assert!(!sanitized.contains('\n'));
    assert!(!sanitized.contains('\t'));
}

#[test]
fn test_sanitize_for_json_empty() {
    assert_eq!(DataUtils::sanitize_for_json(""), "");
}

#[test]
fn test_sanitize_for_json_already_clean() {
    let input = "Hello World";
    assert_eq!(DataUtils::sanitize_for_json(input), input);
}

// ==================== URL Extraction Tests ====================

#[test]
fn test_url_extraction() {
    let text = "Check out https://example.com and http://test.org/path?query=1";
    let urls = DataUtils::extract_urls_from_text(text);
    assert_eq!(urls.len(), 2);
    assert!(urls.contains(&"https://example.com".to_string()));
    assert!(urls.contains(&"http://test.org/path?query=1".to_string()));
}

#[test]
fn test_url_extraction_no_urls() {
    let text = "This text has no URLs";
    let urls = DataUtils::extract_urls_from_text(text);
    assert!(urls.is_empty());
}

#[test]
fn test_url_extraction_empty_text() {
    let urls = DataUtils::extract_urls_from_text("");
    assert!(urls.is_empty());
}

#[test]
fn test_url_extraction_multiple_same() {
    let text = "Visit https://example.com and https://example.com again";
    let urls = DataUtils::extract_urls_from_text(text);
    // May contain duplicates depending on implementation
    assert!(urls.contains(&"https://example.com".to_string()));
}

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

// ==================== JSON Utilities Tests ====================

#[test]
fn test_json_utilities() {
    let data = json!({"test": "value"});

    let pretty = DataUtils::pretty_print_json(&data).unwrap();
    assert!(pretty.contains("  "));

    let compact = DataUtils::compact_json(&data).unwrap();
    assert!(!compact.contains("  "));

    let hash = DataUtils::hash_json(&data).unwrap();
    assert_eq!(hash.len(), 64); // SHA-256 hex string length

    let size = DataUtils::json_size_bytes(&data);
    assert!(size > 0);
}

#[test]
fn test_pretty_print_json_nested() {
    let data = json!({"a": {"b": {"c": 1}}});
    let pretty = DataUtils::pretty_print_json(&data).unwrap();
    assert!(pretty.contains("\n"));
    assert!(pretty.contains("  "));
}

#[test]
fn test_compact_json_no_whitespace() {
    let data = json!({"a": 1, "b": 2});
    let compact = DataUtils::compact_json(&data).unwrap();
    assert!(!compact.contains('\n'));
    assert!(!compact.contains("  "));
}

#[test]
fn test_hash_json_consistent() {
    let data = json!({"key": "value"});
    let hash1 = DataUtils::hash_json(&data).unwrap();
    let hash2 = DataUtils::hash_json(&data).unwrap();
    assert_eq!(hash1, hash2);
}

#[test]
fn test_hash_json_different_data() {
    let data1 = json!({"key": "value1"});
    let data2 = json!({"key": "value2"});
    let hash1 = DataUtils::hash_json(&data1).unwrap();
    let hash2 = DataUtils::hash_json(&data2).unwrap();
    assert_ne!(hash1, hash2);
}

#[test]
fn test_json_size_bytes() {
    let small = json!({});
    let large = json!({"key": "value".repeat(1000)});
    assert!(DataUtils::json_size_bytes(&large) > DataUtils::json_size_bytes(&small));
}

#[test]
fn test_deep_clone_json() {
    let data = json!({"key": {"nested": [1, 2, 3]}});
    let cloned = DataUtils::deep_clone_json(&data);
    assert_eq!(data, cloned);
}

#[test]
fn test_deep_clone_json_independence() {
    let data = json!({"key": "value"});
    let mut cloned = DataUtils::deep_clone_json(&data);
    cloned["key"] = json!("modified");
    assert_ne!(data, cloned);
}
