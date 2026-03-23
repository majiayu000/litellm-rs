//! MCP tool argument validation
//!
//! Provides both a built-in lightweight validator and an optional full
//! JSON Schema validator (via the `mcp-validation` feature / `jsonschema` crate).

use serde_json::Value;

#[cfg(test)]
use super::tools::PropertySchema;
use super::tools::ToolInputSchema;

/// Validate arguments using the `jsonschema` crate (full JSON Schema Draft 2020-12).
///
/// Returns `Ok(())` if valid, or `Err(errors)` listing each violation.
#[cfg(feature = "mcp-validation")]
pub fn validate_jsonschema(schema: &ToolInputSchema, args: &Value) -> Result<(), Vec<String>> {
    let schema_value = schema.to_json_schema();
    let validator = match jsonschema::validator_for(&schema_value) {
        Ok(v) => v,
        Err(e) => {
            return Err(vec![format!("invalid JSON Schema: {}", e)]);
        }
    };
    let errors: Vec<String> = validator.iter_errors(args).map(|e| e.to_string()).collect();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Built-in lightweight argument validation.
///
/// Checks: required field presence, property type matching, additional
/// properties enforcement, and enum value constraints.
pub fn validate_builtin(schema: &ToolInputSchema, args: &Value) -> Result<(), Vec<String>> {
    let obj = match args.as_object() {
        Some(o) => o,
        None => {
            return Err(vec![format!(
                "expected object arguments, got {}",
                value_type_name(args)
            )]);
        }
    };

    let mut errors = Vec::new();

    // Check required fields
    for field in &schema.required {
        if !obj.contains_key(field) {
            errors.push(format!("missing required field '{}'", field));
        }
    }

    // Check property types and enum constraints
    for (key, value) in obj {
        if let Some(prop_schema) = schema.properties.get(key) {
            if let Some(type_err) = check_type(key, value, &prop_schema.property_type) {
                errors.push(type_err);
            }

            // Enum constraint
            if let Some(ref allowed) = prop_schema.enum_values
                && let Some(s) = value.as_str()
                && !allowed.iter().any(|v| v == s)
            {
                errors.push(format!(
                    "field '{}' value '{}' not in allowed values [{}]",
                    key,
                    s,
                    allowed.join(", ")
                ));
            }
        } else if schema.additional_properties == Some(false) {
            errors.push(format!("unexpected additional property '{}'", key));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Return a human-readable type name for a JSON value.
pub(crate) fn value_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Check whether `value` matches the expected JSON Schema `type_name`.
/// Returns `Some(error_message)` on mismatch, `None` on match.
fn check_type(field: &str, value: &Value, type_name: &str) -> Option<String> {
    let ok = match type_name {
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        "array" => value.is_array(),
        "object" => value.is_object(),
        _ => true, // unknown type, pass through
    };
    if ok {
        None
    } else {
        Some(format!(
            "field '{}' expected type '{}', got '{}'",
            field,
            type_name,
            value_type_name(value)
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_builtin_valid() {
        let schema = ToolInputSchema::object()
            .with_property("name", PropertySchema::string(), true)
            .with_property("age", PropertySchema::integer(), false);

        let args = serde_json::json!({"name": "Alice", "age": 30});
        assert!(validate_builtin(&schema, &args).is_ok());
    }

    #[test]
    fn test_validate_builtin_missing_required() {
        let schema =
            ToolInputSchema::object().with_property("name", PropertySchema::string(), true);

        let args = serde_json::json!({});
        let err = validate_builtin(&schema, &args).unwrap_err();
        assert!(err[0].contains("missing required field 'name'"));
    }

    #[test]
    fn test_validate_builtin_wrong_type() {
        let schema =
            ToolInputSchema::object().with_property("count", PropertySchema::integer(), true);

        let args = serde_json::json!({"count": "not_a_number"});
        let err = validate_builtin(&schema, &args).unwrap_err();
        assert!(err[0].contains("expected type 'integer'"));
    }

    #[test]
    fn test_validate_builtin_non_object() {
        let schema = ToolInputSchema::object();

        let args = serde_json::json!("a string");
        let err = validate_builtin(&schema, &args).unwrap_err();
        assert!(err[0].contains("expected object"));
    }

    #[cfg(feature = "mcp-validation")]
    #[test]
    fn test_validate_jsonschema_valid() {
        let schema =
            ToolInputSchema::object().with_property("name", PropertySchema::string(), true);

        let args = serde_json::json!({"name": "Alice"});
        assert!(validate_jsonschema(&schema, &args).is_ok());
    }

    #[cfg(feature = "mcp-validation")]
    #[test]
    fn test_validate_jsonschema_missing_required() {
        let schema =
            ToolInputSchema::object().with_property("name", PropertySchema::string(), true);

        let args = serde_json::json!({});
        let err = validate_jsonschema(&schema, &args).unwrap_err();
        assert!(!err.is_empty());
    }

    #[cfg(feature = "mcp-validation")]
    #[test]
    fn test_validate_jsonschema_wrong_type() {
        let schema =
            ToolInputSchema::object().with_property("count", PropertySchema::integer(), true);

        let args = serde_json::json!({"count": "not_a_number"});
        let err = validate_jsonschema(&schema, &args).unwrap_err();
        assert!(!err.is_empty());
    }
}
