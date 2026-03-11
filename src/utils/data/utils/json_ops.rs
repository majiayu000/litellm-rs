use crate::core::providers::unified_provider::ProviderError;
use serde_json::{Map, Value};
use std::collections::HashMap;

pub struct JsonOps;

impl JsonOps {
    pub fn convert_to_dict(data: &Value) -> Result<Map<String, Value>, ProviderError> {
        match data {
            Value::Object(map) => Ok(map.clone()),
            _ => Err(ProviderError::InvalidRequest {
                provider: "unknown",
                message: "Data is not a JSON object".to_string(),
            }),
        }
    }

    pub fn convert_list_to_dict(list: &[Value]) -> Vec<Map<String, Value>> {
        list.iter()
            .filter_map(|item| {
                if let Value::Object(map) = item {
                    Some(map.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn jsonify_tools(tools: &[Value]) -> Result<Vec<Map<String, Value>>, ProviderError> {
        let mut jsonified_tools = Vec::new();

        for tool in tools {
            match tool {
                Value::Object(map) => {
                    jsonified_tools.push(map.clone());
                }
                Value::String(s) => {
                    let parsed: Value =
                        serde_json::from_str(s).map_err(|e| ProviderError::InvalidRequest {
                            provider: "unknown",
                            message: format!("Failed to parse tool JSON string: {}", e),
                        })?;

                    if let Value::Object(map) = parsed {
                        jsonified_tools.push(map);
                    } else {
                        return Err(ProviderError::InvalidRequest {
                            provider: "unknown",
                            message: "Tool JSON string must represent an object".to_string(),
                        });
                    }
                }
                _ => {
                    return Err(ProviderError::InvalidRequest {
                        provider: "unknown",
                        message: "Tool must be an object or JSON string".to_string(),
                    });
                }
            }
        }

        Ok(jsonified_tools)
    }

    pub fn cleanup_none_values(data: &mut Map<String, Value>) {
        data.retain(|_, v| !v.is_null());
    }

    pub fn deep_cleanup_none_values(data: &mut Value) {
        match data {
            Value::Object(map) => {
                map.retain(|_, v| !v.is_null());
                for (_, v) in map.iter_mut() {
                    Self::deep_cleanup_none_values(v);
                }
            }
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    Self::deep_cleanup_none_values(item);
                }
            }
            _ => {}
        }
    }

    pub fn merge_json_objects(base: &mut Value, overlay: &Value) -> Result<(), ProviderError> {
        match (base, overlay) {
            (Value::Object(base_map), Value::Object(overlay_map)) => {
                for (key, value) in overlay_map {
                    if let Some(base_value) = base_map.get_mut(key) {
                        if base_value.is_object() && value.is_object() {
                            Self::merge_json_objects(base_value, value)?;
                        } else {
                            *base_value = value.clone();
                        }
                    } else {
                        base_map.insert(key.clone(), value.clone());
                    }
                }
                Ok(())
            }
            _ => Err(ProviderError::InvalidRequest {
                provider: "unknown",
                message: "Both values must be JSON objects for merging".to_string(),
            }),
        }
    }

    pub fn extract_nested_value<'a>(data: &'a Value, path: &[&str]) -> Option<&'a Value> {
        let mut current = data;

        for segment in path {
            match current {
                Value::Object(map) => {
                    if let Some(next_value) = map.get(*segment) {
                        current = next_value;
                    } else {
                        return None;
                    }
                }
                Value::Array(arr) => {
                    if let Ok(index) = segment.parse::<usize>() {
                        if let Some(next_value) = arr.get(index) {
                            current = next_value;
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }

        Some(current)
    }

    pub fn set_nested_value(
        data: &mut Value,
        path: &[&str],
        value: Value,
    ) -> Result<(), ProviderError> {
        if path.is_empty() {
            return Err(ProviderError::InvalidRequest {
                provider: "unknown",
                message: "Path cannot be empty".to_string(),
            });
        }

        let mut current = data;
        let last_segment = path[path.len() - 1];

        for segment in &path[..path.len() - 1] {
            match current {
                Value::Object(map) => {
                    if !map.contains_key(*segment) {
                        map.insert(segment.to_string(), Value::Object(Map::new()));
                    }
                    current = map.get_mut(*segment).unwrap();
                }
                _ => {
                    return Err(ProviderError::InvalidRequest {
                        provider: "unknown",
                        message: "Cannot set nested value in non-object".to_string(),
                    });
                }
            }
        }

        if let Value::Object(map) = current {
            map.insert(last_segment.to_string(), value);
            Ok(())
        } else {
            Err(ProviderError::InvalidRequest {
                provider: "unknown",
                message: "Cannot set value in non-object".to_string(),
            })
        }
    }

    pub fn flatten_json(data: &Value, prefix: Option<String>) -> HashMap<String, Value> {
        let mut result = HashMap::new();
        let current_prefix = prefix.unwrap_or_default();

        match data {
            Value::Object(map) => {
                for (key, value) in map {
                    let new_key = if current_prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", current_prefix, key)
                    };

                    match value {
                        Value::Object(_) | Value::Array(_) => {
                            let nested = Self::flatten_json(value, Some(new_key));
                            result.extend(nested);
                        }
                        _ => {
                            result.insert(new_key, value.clone());
                        }
                    }
                }
            }
            Value::Array(arr) => {
                for (index, value) in arr.iter().enumerate() {
                    let new_key = if current_prefix.is_empty() {
                        index.to_string()
                    } else {
                        format!("{}.{}", current_prefix, index)
                    };

                    match value {
                        Value::Object(_) | Value::Array(_) => {
                            let nested = Self::flatten_json(value, Some(new_key));
                            result.extend(nested);
                        }
                        _ => {
                            result.insert(new_key, value.clone());
                        }
                    }
                }
            }
            _ => {
                result.insert(current_prefix, data.clone());
            }
        }

        result
    }

    pub fn validate_json_schema(data: &Value, schema: &Value) -> Result<(), ProviderError> {
        match (data, schema) {
            (_, Value::Object(schema_map)) => {
                if let Some(type_value) = schema_map.get("type")
                    && let Some(expected_type) = type_value.as_str()
                {
                    let data_type = match data {
                        Value::Null => "null",
                        Value::Bool(_) => "boolean",
                        Value::Number(_) => "number",
                        Value::String(_) => "string",
                        Value::Array(_) => "array",
                        Value::Object(_) => "object",
                    };

                    if data_type != expected_type {
                        return Err(ProviderError::InvalidRequest {
                            provider: "unknown",
                            message: format!(
                                "Expected type '{}', got '{}'",
                                expected_type, data_type
                            ),
                        });
                    }
                }

                if let (Value::Object(data_map), Some(Value::Object(properties))) =
                    (data, schema_map.get("properties"))
                {
                    for (prop_name, prop_schema) in properties {
                        if let Some(prop_data) = data_map.get(prop_name) {
                            Self::validate_json_schema(prop_data, prop_schema)?;
                        }
                    }

                    if let Some(Value::Array(required)) = schema_map.get("required") {
                        for required_prop in required {
                            if let Some(prop_name) = required_prop.as_str()
                                && !data_map.contains_key(prop_name)
                            {
                                return Err(ProviderError::InvalidRequest {
                                    provider: "unknown",
                                    message: format!(
                                        "Required property '{}' is missing",
                                        prop_name
                                    ),
                                });
                            }
                        }
                    }
                }

                Ok(())
            }
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ==================== convert_to_dict Tests ====================

    #[test]
    fn test_convert_to_dict_object() {
        let data = json!({"key": "value"});
        let result = JsonOps::convert_to_dict(&data);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().get("key").unwrap(), "value");
    }

    #[test]
    fn test_convert_to_dict_empty_object() {
        let data = json!({});
        let result = JsonOps::convert_to_dict(&data);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_convert_to_dict_not_object() {
        let data = json!([1, 2, 3]);
        let result = JsonOps::convert_to_dict(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_convert_to_dict_string() {
        let data = json!("not an object");
        let result = JsonOps::convert_to_dict(&data);
        assert!(result.is_err());
    }

    // ==================== convert_list_to_dict Tests ====================

    #[test]
    fn test_convert_list_to_dict_objects() {
        let list = vec![json!({"a": 1}), json!({"b": 2})];
        let result = JsonOps::convert_list_to_dict(&list);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_convert_list_to_dict_mixed() {
        let list = vec![json!({"a": 1}), json!("string"), json!({"b": 2})];
        let result = JsonOps::convert_list_to_dict(&list);
        assert_eq!(result.len(), 2); // Only objects
    }

    #[test]
    fn test_convert_list_to_dict_no_objects() {
        let list = vec![json!(1), json!("string"), json!(true)];
        let result = JsonOps::convert_list_to_dict(&list);
        assert!(result.is_empty());
    }

    #[test]
    fn test_convert_list_to_dict_empty() {
        let list: Vec<Value> = vec![];
        let result = JsonOps::convert_list_to_dict(&list);
        assert!(result.is_empty());
    }

    // ==================== jsonify_tools Tests ====================

    #[test]
    fn test_jsonify_tools_objects() {
        let tools = vec![json!({"name": "tool1"}), json!({"name": "tool2"})];
        let result = JsonOps::jsonify_tools(&tools).unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_jsonify_tools_json_strings() {
        let tools = vec![Value::String(r#"{"name": "tool1"}"#.to_string())];
        let result = JsonOps::jsonify_tools(&tools).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].get("name").unwrap(), "tool1");
    }

    #[test]
    fn test_jsonify_tools_invalid_json_string() {
        let tools = vec![Value::String("not valid json".to_string())];
        let result = JsonOps::jsonify_tools(&tools);
        assert!(result.is_err());
    }

    #[test]
    fn test_jsonify_tools_non_object_json_string() {
        let tools = vec![Value::String("[1, 2, 3]".to_string())];
        let result = JsonOps::jsonify_tools(&tools);
        assert!(result.is_err());
    }

    #[test]
    fn test_jsonify_tools_invalid_type() {
        let tools = vec![json!(123)];
        let result = JsonOps::jsonify_tools(&tools);
        assert!(result.is_err());
    }

    // ==================== cleanup_none_values Tests ====================

    #[test]
    fn test_cleanup_none_values_removes_nulls() {
        let mut map = Map::new();
        map.insert("key1".to_string(), json!("value"));
        map.insert("key2".to_string(), Value::Null);
        map.insert("key3".to_string(), json!(123));

        JsonOps::cleanup_none_values(&mut map);

        assert_eq!(map.len(), 2);
        assert!(!map.contains_key("key2"));
    }

    #[test]
    fn test_cleanup_none_values_keeps_non_null() {
        let mut map = Map::new();
        map.insert("key1".to_string(), json!("value"));
        map.insert("key2".to_string(), json!(false));
        map.insert("key3".to_string(), json!(0));

        JsonOps::cleanup_none_values(&mut map);

        assert_eq!(map.len(), 3);
    }

    #[test]
    fn test_cleanup_none_values_all_nulls() {
        let mut map = Map::new();
        map.insert("key1".to_string(), Value::Null);
        map.insert("key2".to_string(), Value::Null);

        JsonOps::cleanup_none_values(&mut map);

        assert!(map.is_empty());
    }

    // ==================== deep_cleanup_none_values Tests ====================

    #[test]
    fn test_deep_cleanup_nested_nulls() {
        let mut data = json!({
            "level1": {
                "level2": null,
                "keep": "value"
            },
            "remove": null
        });

        JsonOps::deep_cleanup_none_values(&mut data);

        let obj = data.as_object().unwrap();
        assert!(!obj.contains_key("remove"));
        let level1 = obj.get("level1").unwrap().as_object().unwrap();
        assert!(!level1.contains_key("level2"));
        assert!(level1.contains_key("keep"));
    }

    #[test]
    fn test_deep_cleanup_array_with_objects() {
        let mut data = json!([
            {"key": "value", "null_key": null},
            {"other": 123}
        ]);

        JsonOps::deep_cleanup_none_values(&mut data);

        let arr = data.as_array().unwrap();
        let first = arr[0].as_object().unwrap();
        assert!(!first.contains_key("null_key"));
        assert!(first.contains_key("key"));
    }

    // ==================== merge_json_objects Tests ====================

    #[test]
    fn test_merge_json_objects_simple() {
        let mut base = json!({"a": 1});
        let overlay = json!({"b": 2});

        JsonOps::merge_json_objects(&mut base, &overlay).unwrap();

        assert_eq!(base["a"], 1);
        assert_eq!(base["b"], 2);
    }

    #[test]
    fn test_merge_json_objects_override() {
        let mut base = json!({"key": "original"});
        let overlay = json!({"key": "overridden"});

        JsonOps::merge_json_objects(&mut base, &overlay).unwrap();

        assert_eq!(base["key"], "overridden");
    }

    #[test]
    fn test_merge_json_objects_nested() {
        let mut base = json!({"outer": {"inner": 1}});
        let overlay = json!({"outer": {"inner2": 2}});

        JsonOps::merge_json_objects(&mut base, &overlay).unwrap();

        assert_eq!(base["outer"]["inner"], 1);
        assert_eq!(base["outer"]["inner2"], 2);
    }

    #[test]
    fn test_merge_json_objects_non_objects() {
        let mut base = json!([1, 2, 3]);
        let overlay = json!({"key": "value"});

        let result = JsonOps::merge_json_objects(&mut base, &overlay);
        assert!(result.is_err());
    }

    // ==================== extract_nested_value Tests ====================

    #[test]
    fn test_extract_nested_value_simple() {
        let data = json!({"key": "value"});
        let result = JsonOps::extract_nested_value(&data, &["key"]);
        assert_eq!(result.unwrap(), "value");
    }

    #[test]
    fn test_extract_nested_value_deep() {
        let data = json!({"level1": {"level2": {"level3": "found"}}});
        let result = JsonOps::extract_nested_value(&data, &["level1", "level2", "level3"]);
        assert_eq!(result.unwrap(), "found");
    }

    #[test]
    fn test_extract_nested_value_array_index() {
        let data = json!({"arr": [10, 20, 30]});
        let result = JsonOps::extract_nested_value(&data, &["arr", "1"]);
        assert_eq!(result.unwrap(), 20);
    }

    #[test]
    fn test_extract_nested_value_not_found() {
        let data = json!({"key": "value"});
        let result = JsonOps::extract_nested_value(&data, &["nonexistent"]);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_nested_value_empty_path() {
        let data = json!({"key": "value"});
        let result = JsonOps::extract_nested_value(&data, &[]);
        assert_eq!(result.unwrap(), &data);
    }

    // ==================== set_nested_value Tests ====================

    #[test]
    fn test_set_nested_value_simple() {
        let mut data = json!({});
        JsonOps::set_nested_value(&mut data, &["key"], json!("value")).unwrap();
        assert_eq!(data["key"], "value");
    }

    #[test]
    fn test_set_nested_value_deep() {
        let mut data = json!({});
        JsonOps::set_nested_value(&mut data, &["a", "b", "c"], json!(123)).unwrap();
        assert_eq!(data["a"]["b"]["c"], 123);
    }

    #[test]
    fn test_set_nested_value_empty_path() {
        let mut data = json!({});
        let result = JsonOps::set_nested_value(&mut data, &[], json!("value"));
        assert!(result.is_err());
    }

    #[test]
    fn test_set_nested_value_override() {
        let mut data = json!({"key": "old"});
        JsonOps::set_nested_value(&mut data, &["key"], json!("new")).unwrap();
        assert_eq!(data["key"], "new");
    }

    // ==================== flatten_json Tests ====================

    #[test]
    fn test_flatten_json_simple() {
        let data = json!({"a": 1, "b": 2});
        let result = JsonOps::flatten_json(&data, None);
        assert_eq!(result.get("a").unwrap(), &json!(1));
        assert_eq!(result.get("b").unwrap(), &json!(2));
    }

    #[test]
    fn test_flatten_json_nested() {
        let data = json!({"outer": {"inner": "value"}});
        let result = JsonOps::flatten_json(&data, None);
        assert_eq!(result.get("outer.inner").unwrap(), &json!("value"));
    }

    #[test]
    fn test_flatten_json_array() {
        let data = json!({"arr": [1, 2, 3]});
        let result = JsonOps::flatten_json(&data, None);
        assert_eq!(result.get("arr.0").unwrap(), &json!(1));
        assert_eq!(result.get("arr.1").unwrap(), &json!(2));
        assert_eq!(result.get("arr.2").unwrap(), &json!(3));
    }

    #[test]
    fn test_flatten_json_with_prefix() {
        let data = json!({"key": "value"});
        let result = JsonOps::flatten_json(&data, Some("prefix".to_string()));
        assert_eq!(result.get("prefix.key").unwrap(), &json!("value"));
    }

    // ==================== validate_json_schema Tests ====================

    #[test]
    fn test_validate_json_schema_type_string() {
        let data = json!("hello");
        let schema = json!({"type": "string"});
        assert!(JsonOps::validate_json_schema(&data, &schema).is_ok());
    }

    #[test]
    fn test_validate_json_schema_type_number() {
        let data = json!(123);
        let schema = json!({"type": "number"});
        assert!(JsonOps::validate_json_schema(&data, &schema).is_ok());
    }

    #[test]
    fn test_validate_json_schema_type_object() {
        let data = json!({"key": "value"});
        let schema = json!({"type": "object"});
        assert!(JsonOps::validate_json_schema(&data, &schema).is_ok());
    }

    #[test]
    fn test_validate_json_schema_type_mismatch() {
        let data = json!("string");
        let schema = json!({"type": "number"});
        assert!(JsonOps::validate_json_schema(&data, &schema).is_err());
    }

    #[test]
    fn test_validate_json_schema_required_present() {
        let data = json!({"name": "test", "age": 25});
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "number"}
            },
            "required": ["name", "age"]
        });
        assert!(JsonOps::validate_json_schema(&data, &schema).is_ok());
    }

    #[test]
    fn test_validate_json_schema_required_missing() {
        let data = json!({"name": "test"});
        let schema = json!({
            "type": "object",
            "properties": {
                "name": {"type": "string"},
                "age": {"type": "number"}
            },
            "required": ["name", "age"]
        });
        assert!(JsonOps::validate_json_schema(&data, &schema).is_err());
    }

    #[test]
    fn test_validate_json_schema_nested_property() {
        let data = json!({"user": {"name": "test"}});
        let schema = json!({
            "type": "object",
            "properties": {
                "user": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"}
                    }
                }
            }
        });
        assert!(JsonOps::validate_json_schema(&data, &schema).is_ok());
    }
}
