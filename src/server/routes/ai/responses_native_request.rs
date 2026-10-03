//! The native request billing boundary. Keep the inference wire body intact.
use crate::utils::error::gateway_error::GatewayError;
use serde_json::Value;

/// Returns whether opaque input needs the provider's processed-input token count.
pub(super) fn validate(body: &Value) -> Result<bool, GatewayError> {
    for field in ["prompt", "conversation", "container", "context_management"] {
        if body.get(field).is_some_and(|value| !value.is_null()) {
            return Err(GatewayError::validation(format!(
                "Native Responses {field} is not supported: its additional context or hosted charges cannot yet be reserved"
            )));
        }
    }
    if body
        .get("service_tier")
        .is_some_and(|value| !value.is_null() && value != "default")
    {
        return Err(GatewayError::validation(
            "Native Responses currently supports default service_tier pricing only",
        ));
    }
    if let Some(tools) = body.get("tools").filter(|value| !value.is_null()) {
        let tools = tools
            .as_array()
            .ok_or_else(|| GatewayError::validation("tools must be an array"))?;
        for tool in tools {
            if !matches!(
                tool.get("type").and_then(Value::as_str),
                Some("function" | "custom")
            ) {
                return Err(GatewayError::validation(
                    "Native Responses currently supports client function/custom tools only; hosted tool charges and their request budget bounds are not implemented",
                ));
            }
        }
    }
    inspect_input(body.get("input").unwrap_or(&Value::Null))
}

fn inspect_input(value: &Value) -> Result<bool, GatewayError> {
    match value {
        Value::Array(items) => items
            .iter()
            .try_fold(false, |count, item| Ok(inspect_input(item)? || count)),
        Value::Object(object) => {
            let kind = object.get("type").and_then(Value::as_str);
            if kind == Some("item_reference") {
                return Err(GatewayError::validation(
                    "Native Responses item references require an owner binding and are not supported",
                ));
            }
            let mut count = matches!(
                kind,
                Some("input_image" | "input_file" | "item_reference" | "compaction")
            ) || object.contains_key("encrypted_content");
            if matches!(kind, Some("input_audio" | "input_video")) {
                return Err(GatewayError::validation(
                    "Native Responses audio/video input pricing is not implemented",
                ));
            }
            if matches!(kind, Some("input_image" | "input_file")) {
                let remote_image = object
                    .get("image_url")
                    .and_then(Value::as_str)
                    .is_some_and(|url| !url.starts_with("data:"));
                if remote_image || object.get("file_url").is_some_and(|value| !value.is_null()) {
                    return Err(GatewayError::validation(
                        "Native Responses remote image/file URLs cannot provide a stable token reservation; use inline data or an uploaded file_id",
                    ));
                }
            }
            for value in object.values() {
                count |= inspect_input(value)?;
            }
            Ok(count)
        }
        _ => Ok(false),
    }
}

/// The official input_tokens endpoint accepts this subset of Responses parameters.
/// This projection is never used for the generation request.
pub(super) fn count_body(body: &Value) -> Value {
    let mut count = serde_json::Map::new();
    for key in [
        "model",
        "input",
        "instructions",
        "parallel_tool_calls",
        "personality",
        "previous_response_id",
        "reasoning",
        "text",
        "tool_choice",
        "tools",
        "truncation",
    ] {
        if let Some(value) = body.get(key) {
            count.insert(key.into(), value.clone());
        }
    }
    Value::Object(count)
}
