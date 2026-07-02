use crate::utils::data::utils::DataUtils;
use serde_json::{Map, json};

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
