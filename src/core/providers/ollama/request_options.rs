use super::config::OllamaConfig;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;

const INTEGER_OPTIONS: [&str; 2] = ["num_ctx", "num_predict"];
const FLOAT_OPTIONS: [&str; 1] = ["repeat_penalty"];
const I64_MIN_AS_F64: f64 = -9_223_372_036_854_775_808.0;
const I64_MAX_EXCLUSIVE_AS_F64: f64 = 9_223_372_036_854_775_808.0;

pub(super) fn merge_request_options(
    config: &OllamaConfig,
    options: &mut serde_json::Map<String, serde_json::Value>,
    request: &ChatRequest,
) -> Result<(), ProviderError> {
    for name in INTEGER_OPTIONS {
        if name == "num_predict"
            && request
                .max_completion_tokens
                .or(request.max_tokens)
                .is_some()
        {
            continue;
        }
        let Some(value) = request.extra_params.get(name) else {
            continue;
        };
        let integer = value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok()))
            .or_else(|| {
                value
                    .is_f64()
                    .then(|| value.as_f64())
                    .flatten()
                    .and_then(|value| {
                        (value.is_finite()
                            && value.fract() == 0.0
                            && (I64_MIN_AS_F64..I64_MAX_EXCLUSIVE_AS_F64).contains(&value))
                        .then_some(value as i64)
                    })
            });
        let valid = match (name, integer) {
            ("num_ctx", Some(value)) => value > 0 && u32::try_from(value).is_ok(),
            (_, Some(_)) => true,
            (_, None) => false,
        };
        if !valid {
            return Err(ProviderError::invalid_request(
                "ollama",
                format!("native Ollama option {name} must be a valid integer"),
            ));
        }
        let integer = integer.expect("validated integer option");
        if name == "num_ctx"
            && config
                .num_ctx
                .is_some_and(|configured_max| integer as u64 > u64::from(configured_max))
        {
            return Err(ProviderError::invalid_request(
                "ollama",
                format!(
                    "native Ollama option num_ctx exceeds the configured maximum of {}",
                    config.num_ctx.expect("checked configured num_ctx")
                ),
            ));
        }
        options.insert(name.to_string(), serde_json::json!(integer));
    }

    for name in FLOAT_OPTIONS {
        let Some(value) = request.extra_params.get(name) else {
            continue;
        };
        let valid = value
            .as_f64()
            .is_some_and(|value| value.is_finite() && (value as f32).is_finite());
        if !valid {
            return Err(ProviderError::invalid_request(
                "ollama",
                format!("native Ollama option {name} must be a finite number"),
            ));
        }
        options.insert(name.to_string(), value.clone());
    }

    Ok(())
}
