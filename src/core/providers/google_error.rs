//! Shared Google API error-envelope mapping for Gemini and Vertex surfaces.

use serde_json::Value;

use super::unified_provider::ProviderError;

pub(crate) fn map_google_error_envelope(provider: &'static str, response: &Value) -> ProviderError {
    if let Some(error) = response.get("error") {
        let explicit_code = error
            .get("code")
            .and_then(Value::as_u64)
            .and_then(|code| u16::try_from(code).ok());
        let code = explicit_code.unwrap_or(500);
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Unknown error");
        let status = error.get("status").and_then(Value::as_str).unwrap_or("");

        return match (code, status) {
            (401, _) | (_, "UNAUTHENTICATED") => ProviderError::authentication(provider, message),
            (403, _) | (_, "PERMISSION_DENIED") => {
                let status = explicit_code
                    .filter(|code| (400..600).contains(code))
                    .unwrap_or(403);
                ProviderError::api_error(provider, status, message)
            }
            (400, _) | (_, "INVALID_ARGUMENT") => ProviderError::invalid_request(provider, message),
            (404, _) | (_, "NOT_FOUND") => ProviderError::model_not_found(provider, message),
            (429, _) | (_, "RESOURCE_EXHAUSTED") => ProviderError::RateLimit {
                provider,
                message: message.to_string(),
                retry_after: retry_after(error),
                rpm_limit: None,
                tpm_limit: None,
                current_usage: None,
            },
            (503, _) | (_, "UNAVAILABLE") => {
                ProviderError::provider_unavailable(provider, "Service unavailable")
            }
            (_, "FAILED_PRECONDITION") => ProviderError::invalid_request(provider, message),
            (_, "UNIMPLEMENTED") => ProviderError::NotSupported {
                provider,
                feature: message.to_string(),
            },
            _ => ProviderError::api_error(provider, code, message),
        };
    }

    if let Some(message) = response.get("message").and_then(Value::as_str) {
        return ProviderError::api_error(provider, 500, message);
    }

    if let Some(finish_reason) = response
        .get("candidates")
        .and_then(Value::as_array)
        .and_then(|candidates| candidates.first())
        .and_then(|candidate| candidate.get("finishReason"))
        .and_then(Value::as_str)
    {
        return match finish_reason {
            "SAFETY" => {
                ProviderError::invalid_request(provider, "Content blocked by safety filters")
            }
            "RECITATION" => {
                ProviderError::invalid_request(provider, "Content blocked due to recitation")
            }
            "MAX_TOKENS" => ProviderError::invalid_request(provider, "Maximum token limit reached"),
            "STOP" => ProviderError::api_error(provider, 200, "Generation completed"),
            _ => ProviderError::api_error(
                provider,
                500,
                format!("Unknown finish reason: {finish_reason}"),
            ),
        };
    }

    ProviderError::api_error(provider, 500, "Unknown API error")
}

fn retry_after(error: &Value) -> Option<u64> {
    error
        .get("retry_after")
        .and_then(Value::as_u64)
        .or_else(|| {
            error
                .get("details")
                .and_then(Value::as_array)
                .and_then(|details| {
                    details
                        .iter()
                        .find_map(|detail| detail.get("retry_after").and_then(Value::as_u64))
                })
        })
}
