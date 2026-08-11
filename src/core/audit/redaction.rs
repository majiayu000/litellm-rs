use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

use super::events::AuditEvent;
use super::types::UserAction;

pub(super) fn redact_event(patterns: &[Regex], mut event: AuditEvent) -> AuditEvent {
    redact_optional_string(patterns, &mut event.request_id);
    redact_optional_string(patterns, &mut event.user_id);
    redact_optional_string(patterns, &mut event.api_key_id);
    redact_optional_string(patterns, &mut event.team_id);
    event.message = redact_string(patterns, &event.message);
    redact_optional_string(patterns, &mut event.source);

    if let Some(request) = &mut event.request {
        request.request_id = redact_string(patterns, &request.request_id);
        request.method = redact_string(patterns, &request.method);
        request.path = redact_string(patterns, &request.path);
        redact_string_map(patterns, &mut request.query_params);
        redact_string_map(patterns, &mut request.headers);
        redact_optional_string(patterns, &mut request.body);
        redact_optional_string(patterns, &mut request.client_ip);
        redact_optional_string(patterns, &mut request.user_agent);
    }

    if let Some(response) = &mut event.response {
        response.request_id = redact_string(patterns, &response.request_id);
        redact_string_map(patterns, &mut response.headers);
        redact_optional_string(patterns, &mut response.body);
    }

    if let Some(UserAction::Custom(action)) = &mut event.action {
        *action = redact_string(patterns, action);
    }

    let metadata = std::mem::take(&mut event.metadata);
    event.metadata = metadata
        .into_iter()
        .map(|(key, mut value)| {
            redact_json_value(patterns, &mut value);
            (redact_string(patterns, &key), value)
        })
        .collect();

    event
}

pub(super) fn redact_string(patterns: &[Regex], value: &str) -> String {
    let mut result = value.to_string();
    for pattern in patterns {
        result = pattern.replace_all(&result, "[REDACTED]").to_string();
    }
    result
}

fn redact_optional_string(patterns: &[Regex], value: &mut Option<String>) {
    if let Some(value) = value {
        *value = redact_string(patterns, value);
    }
}

fn redact_string_map(patterns: &[Regex], values: &mut HashMap<String, String>) {
    *values = std::mem::take(values)
        .into_iter()
        .map(|(key, value)| {
            (
                redact_string(patterns, &key),
                redact_string(patterns, &value),
            )
        })
        .collect();
}

fn redact_json_value(patterns: &[Regex], value: &mut Value) {
    match value {
        Value::String(text) => *text = redact_string(patterns, text),
        Value::Array(values) => {
            for value in values {
                redact_json_value(patterns, value);
            }
        }
        Value::Object(values) => {
            let original = std::mem::take(values);
            *values = original
                .into_iter()
                .map(|(key, mut value)| {
                    redact_json_value(patterns, &mut value);
                    (redact_string(patterns, &key), value)
                })
                .collect();
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}
