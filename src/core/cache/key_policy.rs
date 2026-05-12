//! Stable cache key policy helpers.

use serde::Serialize;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Increment this when request dimensions included in cache keys change.
///
/// Bumping this value intentionally cold-starts older Redis/cache entries so
/// responses produced under an older request identity policy are not reused.
pub const CACHE_KEY_SCHEMA_VERSION: &str = "v4";

/// Generate a stable SHA-256 digest for any serializable value.
pub fn stable_digest<T: Serialize>(value: &T) -> String {
    let value = serde_json::to_value(value).unwrap_or(Value::Null);
    stable_digest_value(&value)
}

/// Generate a stable SHA-256 digest for an already materialized JSON value.
pub fn stable_digest_value(value: &Value) -> String {
    let canonical = canonical_json_string(value);
    let digest = Sha256::digest(canonical.as_bytes());
    hex::encode(digest)
}

/// Convert a JSON value to canonical JSON by sorting object keys and removing
/// fields that cannot affect model output identity.
pub fn canonical_json_string(value: &Value) -> String {
    let canonical = canonicalize_json_value(value, &mut Vec::new());
    serde_json::to_string(&canonical).unwrap_or_else(|_| "null".to_string())
}

/// Parse and canonicalize a JSON string.
pub fn canonical_json_str(raw_json: &str) -> String {
    match serde_json::from_str::<Value>(raw_json) {
        Ok(value) => canonical_json_string(&value),
        Err(_) => raw_json.to_string(),
    }
}

/// Returns true when a serialized field should be excluded from cache identity.
pub fn is_non_deterministic_field(field: &str) -> bool {
    matches!(
        field,
        "timestamp"
            | "request_id"
            | "trace_id"
            | "span_id"
            | "created_at"
            | "updated_at"
            | "id"
            | "stream"
            | "stream_options"
    )
}

/// Format a stable cache key using the shared schema version namespace.
pub fn versioned_key(prefix: &str, namespace: Option<&str>, digest: &str) -> String {
    match namespace {
        Some(namespace) => format!("{prefix}:{namespace}:{CACHE_KEY_SCHEMA_VERSION}:{digest}"),
        None => format!("{prefix}:{CACHE_KEY_SCHEMA_VERSION}:{digest}"),
    }
}

fn should_exclude_field(path: &[String], field: &str) -> bool {
    if !is_non_deterministic_field(field) {
        return false;
    }

    path.is_empty() || (path.len() == 1 && path[0] == "extra_body")
}

fn canonicalize_json_value(value: &Value, path: &mut Vec<String>) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = Map::new();
            for (key, value) in map {
                if should_exclude_field(path, key) {
                    continue;
                }
                path.push(key.clone());
                let canonical = canonicalize_json_value(value, path);
                path.pop();
                sorted.insert(key.clone(), canonical);
            }
            Value::Object(sorted)
        }
        Value::Array(values) => {
            path.push("[]".to_string());
            let canonical = values
                .iter()
                .map(|value| canonicalize_json_value(value, path))
                .collect();
            path.pop();
            Value::Array(canonical)
        }
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_json_sorts_object_keys() {
        assert_eq!(
            canonical_json_str(r#"{"b":2,"a":1}"#),
            canonical_json_str(r#"{"a":1,"b":2}"#)
        );
    }

    #[test]
    fn canonical_json_filters_transport_fields() {
        let without_transport = canonical_json_str(r#"{"message":"hello"}"#);
        let with_transport =
            canonical_json_str(r#"{"message":"hello","stream":true,"request_id":"abc"}"#);

        assert_eq!(without_transport, with_transport);
    }

    #[test]
    fn canonical_json_preserves_nested_id_fields() {
        let canonical = canonical_json_string(&json!({
            "request_id": "abc",
            "response_format": {
                "json_schema": {
                    "schema": {
                        "properties": {
                            "id": { "type": "string" }
                        }
                    }
                }
            },
            "tools": [{
                "function": {
                    "parameters": {
                        "properties": {
                            "id": { "type": "number" }
                        }
                    }
                }
            }]
        }));
        let value: Value = serde_json::from_str(&canonical).unwrap();

        assert!(value.get("request_id").is_none());
        assert_eq!(
            value["response_format"]["json_schema"]["schema"]["properties"]["id"]["type"],
            "string"
        );
        assert_eq!(
            value["tools"][0]["function"]["parameters"]["properties"]["id"]["type"],
            "number"
        );
    }

    #[test]
    fn canonical_json_keeps_nested_id_identity_distinct() {
        let string_id_schema = json!({
            "response_format": {
                "json_schema": {
                    "schema": {
                        "properties": {
                            "id": { "type": "string" }
                        }
                    }
                }
            }
        });
        let integer_id_schema = json!({
            "response_format": {
                "json_schema": {
                    "schema": {
                        "properties": {
                            "id": { "type": "integer" }
                        }
                    }
                }
            }
        });

        assert_ne!(
            canonical_json_string(&string_id_schema),
            canonical_json_string(&integer_id_schema)
        );
    }

    #[test]
    fn canonical_json_filters_flattened_extra_body_transport_fields() {
        let without_request_id = canonical_json_string(&json!({
            "extra_body": {
                "provider_specific": "value"
            }
        }));
        let with_request_id = canonical_json_string(&json!({
            "extra_body": {
                "provider_specific": "value",
                "request_id": "abc"
            }
        }));

        assert_eq!(without_request_id, with_request_id);
    }

    #[test]
    fn canonical_json_preserves_nested_output_schema_transport_names() {
        let canonical = canonical_json_string(&json!({
            "response_format": {
                "json_schema": {
                    "schema": {
                        "properties": {
                            "timestamp": { "type": "string" },
                            "request_id": { "type": "string" },
                            "created_at": { "type": "string" },
                            "updated_at": { "type": "string" }
                        }
                    }
                }
            }
        }));
        let value: Value = serde_json::from_str(&canonical).unwrap();

        assert_eq!(
            value["response_format"]["json_schema"]["schema"]["properties"]["timestamp"]["type"],
            "string"
        );
        assert_eq!(
            value["response_format"]["json_schema"]["schema"]["properties"]["request_id"]["type"],
            "string"
        );
        assert_eq!(
            value["response_format"]["json_schema"]["schema"]["properties"]["created_at"]["type"],
            "string"
        );
        assert_eq!(
            value["response_format"]["json_schema"]["schema"]["properties"]["updated_at"]["type"],
            "string"
        );
    }

    #[test]
    fn canonical_json_keeps_nested_timestamp_identity_distinct() {
        let string_timestamp_schema = json!({
            "response_format": {
                "json_schema": {
                    "schema": {
                        "properties": {
                            "timestamp": { "type": "string" }
                        }
                    }
                }
            }
        });
        let integer_timestamp_schema = json!({
            "response_format": {
                "json_schema": {
                    "schema": {
                        "properties": {
                            "timestamp": { "type": "integer" }
                        }
                    }
                }
            }
        });

        assert_ne!(
            canonical_json_string(&string_timestamp_schema),
            canonical_json_string(&integer_timestamp_schema)
        );
    }

    #[test]
    fn canonical_json_preserves_array_element_identity_fields() {
        let id_a = json!([{ "id": "a" }]);
        let id_b = json!([{ "id": "b" }]);

        assert_ne!(canonical_json_string(&id_a), canonical_json_string(&id_b));
    }

    #[test]
    fn stable_digest_is_deterministic_and_sha256_sized() {
        let digest = stable_digest_value(&json!({"b": 2, "a": 1}));

        assert_eq!(digest, stable_digest_value(&json!({"a": 1, "b": 2})));
        assert_eq!(digest.len(), 64);
    }

    #[test]
    fn versioned_key_includes_schema_version() {
        assert_eq!(
            versioned_key("chat", Some("gpt-4"), "abc"),
            "chat:gpt-4:v4:abc"
        );
        assert_eq!(versioned_key("raw", None, "abc"), "raw:v4:abc");
    }
}
