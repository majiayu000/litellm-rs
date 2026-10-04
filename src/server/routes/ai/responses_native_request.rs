//! The native request billing boundary. Keep the inference wire body intact.
use crate::core::pricing_service::PricingUsage;
use crate::core::types::responses::Usage;
use crate::utils::error::gateway_error::GatewayError;
use serde_json::Value;

pub(super) struct BillingScope {
    pub count_input: bool,
    pub file_search_calls: Option<u32>,
}

/// Validate the billing boundary once; inference fields remain unchanged.
pub(super) fn validate(body: &Value) -> Result<BillingScope, GatewayError> {
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
    let mut file_search = false;
    if let Some(tools) = body.get("tools").filter(|value| !value.is_null()) {
        let tools = tools
            .as_array()
            .ok_or_else(|| GatewayError::validation("tools must be an array"))?;
        for tool in tools {
            if !matches!(
                tool.get("type").and_then(Value::as_str),
                Some("function" | "custom" | "file_search")
            ) {
                return Err(GatewayError::validation(
                    "Native Responses supports function/custom tools and bounded file_search only; other hosted tool charges are not implemented",
                ));
            }
            file_search |= tool.get("type").and_then(Value::as_str) == Some("file_search");
        }
    }
    let file_search_calls = if file_search {
        if body.get("background") == Some(&Value::Bool(true)) {
            return Err(GatewayError::validation(
                "Background file_search billing recovery is not supported",
            ));
        }
        Some(
            body.get("max_tool_calls")
                .and_then(Value::as_u64)
                .and_then(|calls| u32::try_from(calls).ok())
                .filter(|calls| *calls > 0)
                .ok_or_else(|| {
                    GatewayError::validation(
                        "file_search requires a positive integer max_tool_calls budget bound",
                    )
                })?,
        )
    } else {
        None
    };
    Ok(BillingScope {
        count_input: inspect_input(body.get("input").unwrap_or(&Value::Null))? || file_search,
        file_search_calls,
    })
}

/// Count terminal calls, never SSE progress events or the request's maximum.
pub(super) fn pricing_usage(
    value: &Value,
    usage: &Usage,
    maximum: Option<u32>,
) -> Option<PricingUsage> {
    let mut pricing = PricingUsage::from(usage);
    let Some(maximum) = maximum else {
        return Some(pricing);
    };
    let output = value.get("output")?.as_array()?;
    let mut ids = std::collections::HashSet::new();
    for item in output {
        if item.get("type")?.as_str()? != "file_search_call" {
            continue;
        }
        // Failed/in-progress calls have no verified billable-unit contract.
        if item.get("status")?.as_str()? != "completed" {
            return None;
        }
        let id = item.get("id")?.as_str()?;
        if id.is_empty() || !ids.insert(id) {
            return None;
        }
    }
    let calls = u32::try_from(ids.len()).ok()?;
    if calls > maximum {
        return None;
    }
    pricing.file_search_requests = Some(calls);
    Some(pricing)
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
