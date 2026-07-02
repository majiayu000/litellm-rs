use crate::utils::data::utils::DataUtils;
use serde_json::json;

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
