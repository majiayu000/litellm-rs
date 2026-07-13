use crate::core::providers::unified_provider::ProviderError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BedrockService {
    Runtime,
    Control,
    AgentRuntime,
}

impl BedrockService {
    pub(super) fn base_url(self, region: &str) -> String {
        let service = match self {
            Self::Runtime => "bedrock-runtime",
            Self::Control => "bedrock",
            Self::AgentRuntime => "bedrock-agent-runtime",
        };
        format!("https://{service}.{region}.amazonaws.com")
    }
}

pub(super) struct BedrockRequestTarget {
    pub(super) service: BedrockService,
    pub(super) url: String,
}

pub(super) fn request_target(
    region: &str,
    model_id: &str,
    operation: &str,
) -> Result<BedrockRequestTarget, ProviderError> {
    let operation = operation.trim_matches('/');
    let (service, path) = match operation {
        "invoke" | "invoke-with-response-stream" | "converse" | "converse-stream" => {
            if model_id.is_empty() {
                return Err(ProviderError::invalid_request(
                    "bedrock",
                    format!("model ID is required for operation '{operation}'"),
                ));
            }
            (
                BedrockService::Runtime,
                format!(
                    "model/{}/{}",
                    encode_model_id_path_segment(model_id),
                    operation
                ),
            )
        }
        "list-foundation-models" => (BedrockService::Control, "foundation-models".to_string()),
        path if path == "model-invocation-job" || path.starts_with("model-invocation-job/") => {
            (BedrockService::Control, batch_operation_path(path)?)
        }
        path if path.starts_with("agents/") || path.starts_with("knowledgebases/") => {
            (BedrockService::AgentRuntime, path.to_string())
        }
        path if path.starts_with("guardrail/") => {
            (BedrockService::Runtime, guardrail_operation_path(path)?)
        }
        _ => {
            return Err(ProviderError::invalid_request(
                "bedrock",
                format!("unsupported Bedrock operation '{operation}'"),
            ));
        }
    };

    Ok(BedrockRequestTarget {
        service,
        url: format!("{}/{}", service.base_url(region), path),
    })
}

fn encode_model_id_path_segment(model_id: &str) -> String {
    url::form_urlencoded::byte_serialize(model_id.as_bytes()).collect()
}

fn batch_operation_path(path: &str) -> Result<String, ProviderError> {
    let Some(rest) = path.strip_prefix("model-invocation-job") else {
        return Err(invalid_operation_path(path));
    };
    if rest.is_empty() {
        return Ok("model-invocation-job".to_string());
    }
    let Some(rest) = rest.strip_prefix('/') else {
        return Err(invalid_operation_path(path));
    };
    let (identifier, suffix) = rest
        .strip_suffix("/stop")
        .map_or((rest, ""), |identifier| (identifier, "/stop"));
    if identifier.is_empty() {
        return Err(invalid_operation_path(path));
    }
    Ok(format!(
        "model-invocation-job/{}{}",
        encode_model_id_path_segment(identifier),
        suffix
    ))
}

fn guardrail_operation_path(path: &str) -> Result<String, ProviderError> {
    let rest = path
        .strip_prefix("guardrail/")
        .ok_or_else(|| invalid_operation_path(path))?;
    let (identifier, version) = rest
        .split_once("/version/")
        .ok_or_else(|| invalid_operation_path(path))?;
    let version = version
        .strip_suffix("/apply")
        .ok_or_else(|| invalid_operation_path(path))?;
    if identifier.is_empty() || version.is_empty() {
        return Err(invalid_operation_path(path));
    }
    Ok(format!(
        "guardrail/{}/version/{}/apply",
        encode_model_id_path_segment(identifier),
        encode_model_id_path_segment(version)
    ))
}

fn invalid_operation_path(path: &str) -> ProviderError {
    ProviderError::invalid_request(
        "bedrock",
        format!("invalid Bedrock operation path '{path}'"),
    )
}
