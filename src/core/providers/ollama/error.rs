//! Error types for Ollama provider.

use crate::core::providers::base::HttpErrorMapper;
pub use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::tools::{ResponseFormat, ToolChoice};

/// Ollama error type (alias to unified ProviderError)
pub type OllamaError = ProviderError;

pub(super) fn parse_http_json_response(
    status: u16,
    body: &[u8],
) -> Result<serde_json::Value, ProviderError> {
    if !(200..300).contains(&status) {
        return Err(HttpErrorMapper::map_status_code(
            "ollama",
            status,
            &String::from_utf8_lossy(body),
        ));
    }
    serde_json::from_slice(body).map_err(|error| {
        ProviderError::api_error("ollama", 500, format!("Failed to parse response: {error}"))
    })
}

pub(super) fn parse_tool_arguments(raw: &str) -> Result<serde_json::Value, ProviderError> {
    let arguments: serde_json::Value = serde_json::from_str(raw).map_err(|error| {
        ProviderError::invalid_request(
            "ollama",
            format!("tool call arguments must be valid JSON: {error}"),
        )
    })?;
    if !arguments.is_object() {
        return Err(ProviderError::invalid_request(
            "ollama",
            "tool call arguments must be a JSON object",
        ));
    }
    Ok(arguments)
}

pub(super) fn inline_image_data(url: &str) -> Result<String, ProviderError> {
    url.strip_prefix("data:")
        .and_then(|data| {
            let (metadata, payload) = data.split_once(',')?;
            metadata.ends_with(";base64").then_some(payload.to_string())
        })
        .filter(|payload| !payload.is_empty())
        .ok_or_else(|| {
            ProviderError::invalid_request(
                "ollama",
                "native Ollama image URLs must be non-empty base64 data URLs",
            )
        })
}

pub(super) fn should_send_tools(tool_choice: Option<&ToolChoice>) -> Result<bool, ProviderError> {
    match tool_choice {
        None => Ok(true),
        Some(ToolChoice::String(choice)) if choice == "auto" => Ok(true),
        Some(ToolChoice::String(choice)) if choice == "none" => Ok(false),
        Some(_) => Err(ProviderError::invalid_request(
            "ollama",
            "native Ollama supports only tool_choice auto or none",
        )),
    }
}

pub(super) fn response_format_value(
    format: Option<&ResponseFormat>,
) -> Result<Option<serde_json::Value>, ProviderError> {
    let Some(format) = format else {
        return Ok(None);
    };
    match format.format_type.as_str() {
        "text" => Ok(None),
        "json_object" => Ok(Some(serde_json::Value::String("json".to_string()))),
        "json_schema" => {
            let schema = format.json_schema.as_ref().ok_or_else(|| {
                ProviderError::invalid_request(
                    "ollama",
                    "json_schema response format needs a schema",
                )
            })?;
            let schema = schema
                .as_object()
                .filter(|wrapper| wrapper.contains_key("name") || wrapper.contains_key("strict"))
                .and_then(|wrapper| wrapper.get("schema"))
                .unwrap_or(schema);
            Ok(Some(schema.clone()))
        }
        other => Err(ProviderError::invalid_request(
            "ollama",
            format!("unsupported response format: {other}"),
        )),
    }
}
