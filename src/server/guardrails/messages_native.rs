//! Check native Messages text while preserving opaque payloads and protocol IDs.
use super::{Enforcement, GuardrailDecisionSink, GuardrailEngine, enforce_with_sink};
use crate::utils::error::gateway_error::GatewayError;
use serde_json::Value;

fn opaque(kind: Option<&str>, key: &str) -> bool {
    (key == "data" && matches!(kind, Some("base64" | "redacted_thinking")))
        || (key == "signature" && matches!(kind, Some("thinking" | "signature_delta")))
}

pub(crate) fn messages_projection(value: &Value) -> String {
    fn collect(value: &Value, parts: &mut Vec<String>) {
        match value {
            Value::String(text) => {
                parts.push(text.clone());
                if let Ok(decoded) = serde_json::from_str::<Value>(text) {
                    collect(&decoded, parts);
                }
            }
            Value::Array(values) => {
                let joined: String = values
                    .iter()
                    .filter_map(|v| v.get("text").and_then(Value::as_str))
                    .collect();
                if !joined.is_empty() {
                    parts.push(joined);
                }
                for value in values {
                    collect(value, parts);
                }
            }
            Value::Object(values) => {
                let kind = values.get("type").and_then(Value::as_str);
                for (key, value) in values {
                    parts.push(key.clone());
                    if !opaque(kind, key) {
                        collect(value, parts);
                    }
                }
            }
            _ => {}
        }
    }
    let mut parts = Vec::new();
    collect(value, &mut parts);
    parts.join(super::FRAGMENT_SEPARATOR)
}

pub(crate) async fn apply_native_messages(
    engine: &GuardrailEngine,
    mut value: Value,
    output: bool,
    sink: &GuardrailDecisionSink,
) -> Result<Value, GatewayError> {
    if !output && !engine.input_checks_enabled() {
        return Ok(value);
    }
    let surface = if output { "output" } else { "input" };
    let text = messages_projection(&value);
    let result = if output {
        engine.check_output(&text).await
    } else {
        engine.check_input(&text).await
    };
    match enforce_with_sink(result, surface, Some(sink))? {
        Enforcement::Pass => Ok(value),
        Enforcement::Mask => {
            mask(engine, &mut value, surface, false)?;
            let text = messages_projection(&value);
            let result = if output {
                engine.check_output(&text).await
            } else {
                engine.check_input(&text).await
            };
            match enforce_with_sink(result, surface, Some(sink))? {
                Enforcement::Pass => Ok(value),
                Enforcement::Mask => Err(super::projection_error(surface)),
            }
        }
    }
}

fn mask(
    engine: &GuardrailEngine,
    value: &mut Value,
    surface: &str,
    structural: bool,
) -> Result<(), GatewayError> {
    match value {
        Value::String(text) => {
            let mut masked = text.clone();
            if let Ok(mut decoded) = serde_json::from_str::<Value>(text) {
                let original = decoded.clone();
                mask(engine, &mut decoded, surface, structural)?;
                if decoded != original {
                    masked = decoded.to_string();
                }
            }
            let changed = super::mask_text(engine, &mut masked)? || masked != *text;
            if changed && structural {
                return Err(super::projection_error(surface));
            }
            *text = masked;
        }
        Value::Array(values) => {
            for value in values {
                mask(engine, value, surface, structural)?;
            }
        }
        Value::Object(values) => {
            let kind = values
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_owned);
            for (key, value) in values {
                if opaque(kind.as_deref(), key) {
                    continue;
                }
                let mut masked_key = key.clone();
                if super::mask_text(engine, &mut masked_key)? {
                    return Err(super::projection_error(surface));
                }
                let structural = structural
                    || matches!(
                        key.as_str(),
                        "model"
                            | "type"
                            | "role"
                            | "id"
                            | "tool_use_id"
                            | "name"
                            | "url"
                            | "file_id"
                            | "media_type"
                    );
                mask(engine, value, surface, structural)?;
            }
        }
        _ => {}
    }
    Ok(())
}
