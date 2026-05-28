//! Bedrock model-specific request parameter policy.
//!
//! The policy layer fails before transport when a Bedrock model cannot accept a
//! requested parameter. This prevents silently dropping unsupported settings.

use crate::core::providers::bedrock::parse_bedrock_model_id;
use crate::core::providers::unified_provider::ProviderError;
use crate::core::types::chat::ChatRequest;
use crate::core::types::thinking::{ThinkingConfig, ThinkingEffort};
use serde_json::{Map, Value, json};

const DEFAULT_TEMPERATURE: f32 = 1.0;
const DEFAULT_TOP_P: f32 = 0.999;
const FLOAT_EPSILON: f32 = 0.000_001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SamplingPolicy {
    Allow,
    ForbidNonDefault,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThinkingPolicy {
    Unsupported,
    AnthropicAdaptive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::core::providers::bedrock) struct BedrockParameterPolicy {
    model_label: &'static str,
    sampling: SamplingPolicy,
    thinking: ThinkingPolicy,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::core::providers::bedrock) struct BedrockChatParameterFields {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub stop_sequences: Option<Vec<String>>,
    pub top_k: Option<Value>,
    pub thinking: Option<Value>,
}

impl BedrockChatParameterFields {
    pub fn additional_model_request_fields(&self) -> Option<Value> {
        let mut fields = Map::new();
        if let Some(top_k) = &self.top_k {
            fields.insert("top_k".to_string(), top_k.clone());
        }
        if let Some(thinking) = &self.thinking {
            fields.insert("thinking".to_string(), thinking.clone());
        }
        (!fields.is_empty()).then(|| Value::Object(fields))
    }
}

pub(in crate::core::providers::bedrock) fn bedrock_parameter_policy(
    model_id: &str,
) -> BedrockParameterPolicy {
    if is_claude_opus_47_model_id(model_id) {
        BedrockParameterPolicy {
            model_label: "anthropic.claude-opus-4-7",
            sampling: SamplingPolicy::ForbidNonDefault,
            thinking: ThinkingPolicy::AnthropicAdaptive,
        }
    } else {
        BedrockParameterPolicy {
            model_label: "default Bedrock model",
            sampling: SamplingPolicy::Allow,
            thinking: ThinkingPolicy::Unsupported,
        }
    }
}

pub(in crate::core::providers::bedrock) fn has_bedrock_model_parameter_overrides(
    request: &ChatRequest,
) -> bool {
    request.thinking.is_some()
        || request.reasoning_effort.is_some()
        || request.extra_params.contains_key("thinking")
        || get_extra_param(request, "top_k", "topK").is_some()
        || get_extra_param(
            request,
            "additionalModelRequestFields",
            "additional_model_request_fields",
        )
        .is_some()
}

pub(in crate::core::providers::bedrock) fn serialize_bedrock_chat_parameters(
    request: &ChatRequest,
) -> Result<BedrockChatParameterFields, ProviderError> {
    let policy = bedrock_parameter_policy(&request.model);
    validate_sampling_policy(request, policy)?;

    let top_k = serialize_top_k(request, policy)?;
    let thinking = serialize_thinking(request, policy)?;

    Ok(BedrockChatParameterFields {
        max_tokens: request.max_completion_tokens.or(request.max_tokens),
        temperature: request.temperature.map(f64::from),
        top_p: request.top_p.map(f64::from),
        stop_sequences: request.stop.clone(),
        top_k,
        thinking,
    })
}

fn validate_sampling_policy(
    request: &ChatRequest,
    policy: BedrockParameterPolicy,
) -> Result<(), ProviderError> {
    if policy.sampling != SamplingPolicy::ForbidNonDefault {
        return Ok(());
    }

    if let Some(temperature) = request.temperature
        && !float_matches_default(temperature, DEFAULT_TEMPERATURE)
    {
        return Err(forbidden_parameter(
            policy,
            "temperature",
            "Claude Opus 4.7 on Bedrock only accepts the default temperature; omit temperature or set it to 1.0",
        ));
    }

    if let Some(top_p) = request.top_p
        && !float_matches_default(top_p, DEFAULT_TOP_P)
    {
        return Err(forbidden_parameter(
            policy,
            "top_p",
            "Claude Opus 4.7 on Bedrock only accepts the default top_p; omit top_p or set it to 0.999",
        ));
    }

    if get_extra_param(request, "top_k", "topK").is_some()
        || additional_model_field(request, "top_k")?.is_some()
        || additional_model_field(request, "topK")?.is_some()
    {
        return Err(forbidden_parameter(
            policy,
            "top_k",
            "Claude Opus 4.7 on Bedrock does not accept top_k; omit the field",
        ));
    }

    Ok(())
}

fn serialize_top_k(
    request: &ChatRequest,
    policy: BedrockParameterPolicy,
) -> Result<Option<Value>, ProviderError> {
    let top_k = get_extra_param(request, "top_k", "topK")
        .map(|value| normalize_top_k(value, "top_k"))
        .transpose()?;
    let additional_top_k = additional_model_field(request, "top_k")?
        .or(additional_model_field(request, "topK")?)
        .map(|value| normalize_top_k(value, "additionalModelRequestFields.top_k"))
        .transpose()?;

    match (top_k, additional_top_k) {
        (Some(_), Some(_)) => Err(ProviderError::invalid_request(
            "bedrock",
            "Duplicate Bedrock top_k parameter in extra_params and additionalModelRequestFields",
        )),
        (Some(value), None) | (None, Some(value)) => {
            if policy.sampling == SamplingPolicy::ForbidNonDefault {
                return Err(forbidden_parameter(
                    policy,
                    "top_k",
                    "Claude Opus 4.7 on Bedrock does not accept top_k; omit the field",
                ));
            }
            Ok(Some(value))
        }
        (None, None) => Ok(None),
    }
}

fn serialize_thinking(
    request: &ChatRequest,
    policy: BedrockParameterPolicy,
) -> Result<Option<Value>, ProviderError> {
    if request.extra_params.contains_key("thinking") {
        return Err(ProviderError::invalid_request(
            "bedrock",
            "Use ChatRequest.thinking for Bedrock thinking settings; raw extra_params.thinking is not accepted",
        ));
    }

    if additional_model_field(request, "thinking")?.is_some() {
        return Err(ProviderError::invalid_request(
            "bedrock",
            "Use ChatRequest.thinking for Bedrock thinking settings; raw additionalModelRequestFields.thinking is not accepted",
        ));
    }

    let Some(thinking) = &request.thinking else {
        if request.reasoning_effort.is_some() {
            return Err(ProviderError::invalid_request(
                "bedrock",
                "Bedrock reasoning_effort requires ChatRequest.thinking.enabled=true",
            ));
        }
        return Ok(None);
    };

    if !thinking.enabled {
        if thinking.budget_tokens.is_some()
            || thinking.effort.is_some()
            || request.reasoning_effort.is_some()
            || !thinking.extra_params.is_empty()
            || !thinking.include_thinking
        {
            return Err(ProviderError::invalid_request(
                "bedrock",
                "Bedrock thinking settings must be enabled before setting effort, budget_tokens, include_thinking, or extra_params",
            ));
        }
        return Ok(None);
    }

    match policy.thinking {
        ThinkingPolicy::Unsupported => Err(ProviderError::invalid_request(
            "bedrock",
            format!(
                "Bedrock model {} does not support local thinking serialization yet",
                request.model
            ),
        )),
        ThinkingPolicy::AnthropicAdaptive => {
            serialize_anthropic_adaptive_thinking(request, thinking)
        }
    }
}

fn serialize_anthropic_adaptive_thinking(
    request: &ChatRequest,
    thinking: &ThinkingConfig,
) -> Result<Option<Value>, ProviderError> {
    if thinking.budget_tokens.is_some() {
        return Err(ProviderError::invalid_request(
            "bedrock",
            format!(
                "Bedrock model {} uses adaptive thinking and does not support budget_tokens",
                request.model
            ),
        ));
    }
    if !thinking.include_thinking {
        return Err(ProviderError::invalid_request(
            "bedrock",
            format!(
                "Bedrock model {} cannot represent include_thinking=false for adaptive thinking",
                request.model
            ),
        ));
    }
    if !thinking.extra_params.is_empty() {
        return Err(ProviderError::invalid_request(
            "bedrock",
            format!(
                "Bedrock model {} does not support raw thinking extra_params",
                request.model
            ),
        ));
    }

    let effort = merge_thinking_effort(thinking.effort, request.reasoning_effort.as_deref())?;
    let mut value = Map::new();
    value.insert("type".to_string(), json!("adaptive"));
    if let Some(effort) = effort {
        value.insert("effort".to_string(), json!(effort));
    }
    Ok(Some(Value::Object(value)))
}

fn merge_thinking_effort(
    config_effort: Option<ThinkingEffort>,
    request_effort: Option<&str>,
) -> Result<Option<&'static str>, ProviderError> {
    let config_value = config_effort.map(|effort| effort.as_str());
    let request_value = request_effort.map(validate_reasoning_effort).transpose()?;

    match (config_value, request_value) {
        (Some(config_value), Some(request_value)) if config_value != request_value => {
            Err(ProviderError::invalid_request(
                "bedrock",
                format!(
                    "Conflicting Bedrock thinking effort values: thinking.effort={config_value}, reasoning_effort={request_value}"
                ),
            ))
        }
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn validate_reasoning_effort(effort: &str) -> Result<&'static str, ProviderError> {
    match effort {
        "low" => Ok("low"),
        "medium" => Ok("medium"),
        "high" => Ok("high"),
        other => Err(ProviderError::invalid_request(
            "bedrock",
            format!(
                "Unsupported Bedrock reasoning_effort '{other}'; expected low, medium, or high"
            ),
        )),
    }
}

fn normalize_top_k(value: &Value, field_name: &str) -> Result<Value, ProviderError> {
    let top_k = value.as_u64().ok_or_else(|| {
        ProviderError::invalid_request(
            "bedrock",
            format!("Bedrock {field_name} must be an unsigned integer"),
        )
    })?;

    if top_k > 500 {
        return Err(ProviderError::invalid_request(
            "bedrock",
            format!("Bedrock {field_name} must be less than or equal to 500"),
        ));
    }

    Ok(json!(top_k))
}

fn additional_model_field<'a>(
    request: &'a ChatRequest,
    field_name: &str,
) -> Result<Option<&'a Value>, ProviderError> {
    let Some(fields) = get_extra_param(
        request,
        "additionalModelRequestFields",
        "additional_model_request_fields",
    ) else {
        return Ok(None);
    };

    let object = fields.as_object().ok_or_else(|| {
        ProviderError::invalid_request("bedrock", "additionalModelRequestFields must be an object")
    })?;

    validate_additional_model_fields(object)?;
    Ok(object.get(field_name))
}

fn validate_additional_model_fields(fields: &Map<String, Value>) -> Result<(), ProviderError> {
    for key in fields.keys() {
        match key.as_str() {
            "top_k" | "topK" | "thinking" => {}
            other => {
                return Err(ProviderError::invalid_request(
                    "bedrock",
                    format!("Unsupported Bedrock additionalModelRequestFields field: {other}"),
                ));
            }
        }
    }
    Ok(())
}

fn get_extra_param<'a>(request: &'a ChatRequest, snake: &str, camel: &str) -> Option<&'a Value> {
    request
        .extra_params
        .get(snake)
        .or_else(|| request.extra_params.get(camel))
}

fn forbidden_parameter(
    policy: BedrockParameterPolicy,
    parameter: &str,
    detail: &str,
) -> ProviderError {
    ProviderError::invalid_request(
        "bedrock",
        format!(
            "{} parameter policy forbids non-default {parameter}: {detail}",
            policy.model_label
        ),
    )
}

fn float_matches_default(actual: f32, expected: f32) -> bool {
    (actual - expected).abs() <= FLOAT_EPSILON
}

fn is_claude_opus_47_model_id(model_id: &str) -> bool {
    let parsed = parse_bedrock_model_id(model_id);
    parsed
        .metadata_lookup_ids
        .iter()
        .any(|id| id == "anthropic.claude-opus-4-7")
}
