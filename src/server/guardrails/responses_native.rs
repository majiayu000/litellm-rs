//! Inspect native fields without round-tripping through the chat schema.
use serde_json::Value;

use super::{Enforcement, GuardrailDecisionSink, GuardrailEngine, enforce_with_sink};
use crate::utils::error::gateway_error::GatewayError;

pub(crate) fn native_responses_projection(value: &Value) -> String {
    let mut fragments = Vec::new();
    super::responses_scan::collect_json_projection(value, &mut fragments);
    adjacent_text(value, &mut fragments);
    fragments.join(super::FRAGMENT_SEPARATOR)
}

fn adjacent_text(value: &Value, fragments: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            let text: String = items
                .iter()
                .filter_map(|item| item.get("text")?.as_str())
                .collect();
            if !text.is_empty() {
                fragments.push(text);
            }
            for item in items {
                adjacent_text(item, fragments);
            }
        }
        Value::Object(items) => {
            for item in items.values() {
                adjacent_text(item, fragments);
            }
        }
        _ => {}
    }
}

pub(crate) async fn apply_native_responses(
    engine: &GuardrailEngine,
    mut value: Value,
    output: bool,
    sink: &GuardrailDecisionSink,
) -> Result<Value, GatewayError> {
    if !output && !engine.input_checks_enabled() {
        return Ok(value);
    }
    let surface = if output { "output" } else { "input" };
    let projection = native_responses_projection(&value);
    let result = if output {
        engine.check_output(&projection).await
    } else {
        engine.check_input(&projection).await
    };
    match enforce_with_sink(result, surface, Some(sink))? {
        Enforcement::Pass => Ok(value),
        Enforcement::Mask => {
            mask_value(engine, &mut value, surface, false)?;
            let projection = native_responses_projection(&value);
            let result = if output {
                engine.check_output(&projection).await
            } else {
                engine.check_input(&projection).await
            };
            match enforce_with_sink(result, surface, Some(sink))? {
                Enforcement::Pass => Ok(value),
                Enforcement::Mask => Err(super::projection_error(surface)),
            }
        }
    }
}

fn mask_value(
    engine: &GuardrailEngine,
    value: &mut Value,
    surface: &str,
    structural: bool,
) -> Result<(), GatewayError> {
    match value {
        Value::String(text) => {
            let mut masked = text.clone();
            if let Ok(mut decoded) = serde_json::from_str::<Value>(text) {
                mask_value(engine, &mut decoded, surface, structural)?;
                if decoded != serde_json::from_str::<Value>(text).unwrap_or(Value::Null) {
                    masked = decoded.to_string();
                }
            }
            let changed = super::mask_text(engine, &mut masked)? || masked != *text;
            if changed && structural {
                return Err(super::projection_error(surface));
            }
            *text = masked;
        }
        Value::Array(items) => {
            for item in items {
                mask_value(engine, item, surface, structural)?;
            }
        }
        Value::Object(items) => {
            for (key, item) in items {
                let mut masked_key = key.clone();
                if super::mask_text(engine, &mut masked_key)? {
                    return Err(super::projection_error(surface));
                }
                // These identifiers control routing, tool correlation, and native protocol semantics.
                let structural = structural
                    || matches!(
                        key.as_str(),
                        "model"
                            | "type"
                            | "role"
                            | "id"
                            | "previous_response_id"
                            | "call_id"
                            | "name"
                            | "status"
                            | "object"
                            | "encrypted_content"
                    );
                mask_value(engine, item, surface, structural)?;
            }
        }
        _ => {}
    }
    Ok(())
}
