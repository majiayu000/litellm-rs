//! Output projection for guardrail checks.

use crate::core::models::openai::ChatCompletionResponse;
use crate::utils::error::gateway_error::GatewayError;

pub(super) fn response_payload(
    response: &ChatCompletionResponse,
    separator: &str,
) -> Result<String, GatewayError> {
    let value = serde_json::to_value(&response.choices).map_err(|cause| {
        GatewayError::Internal(format!(
            "output guardrail could not serialize response: {cause}"
        ))
    })?;
    let mut fragments = Vec::new();
    collect_output_strings(&value, &mut fragments);
    Ok(fragments.join(separator))
}

fn collect_output_strings(value: &serde_json::Value, fragments: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) => fragments.push(text.clone()),
        serde_json::Value::Array(values) => {
            for value in values {
                collect_output_strings(value, fragments);
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                fragments.push(key.clone());
                collect_output_strings(value, fragments);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_envelope_is_not_treated_as_maskable_content() {
        let response = ChatCompletionResponse {
            id: "user@example.com".to_string(),
            object: "chat.completion".to_string(),
            created: 0,
            model: "10.0.0.1".to_string(),
            system_fingerprint: Some("second@example.com".to_string()),
            choices: Vec::new(),
            usage: None,
        };

        let payload =
            response_payload(&response, "\n---\n").expect("response choices should serialize");

        assert!(payload.is_empty());
    }
}
