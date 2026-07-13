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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operation_matrix_selects_the_matching_service_authority() {
        let cases = [
            (
                "model",
                "invoke",
                BedrockService::Runtime,
                "https://bedrock-runtime.us-east-1.amazonaws.com/model/model/invoke",
            ),
            (
                "model",
                "invoke-with-response-stream",
                BedrockService::Runtime,
                "https://bedrock-runtime.us-east-1.amazonaws.com/model/model/invoke-with-response-stream",
            ),
            (
                "model",
                "converse",
                BedrockService::Runtime,
                "https://bedrock-runtime.us-east-1.amazonaws.com/model/model/converse",
            ),
            (
                "model",
                "converse-stream",
                BedrockService::Runtime,
                "https://bedrock-runtime.us-east-1.amazonaws.com/model/model/converse-stream",
            ),
            (
                "",
                "list-foundation-models",
                BedrockService::Control,
                "https://bedrock.us-east-1.amazonaws.com/foundation-models",
            ),
            (
                "",
                "model-invocation-job",
                BedrockService::Control,
                "https://bedrock.us-east-1.amazonaws.com/model-invocation-job",
            ),
            (
                "",
                "model-invocation-job/job-1",
                BedrockService::Control,
                "https://bedrock.us-east-1.amazonaws.com/model-invocation-job/job-1",
            ),
            (
                "",
                "model-invocation-job/job-1/stop",
                BedrockService::Control,
                "https://bedrock.us-east-1.amazonaws.com/model-invocation-job/job-1/stop",
            ),
            (
                "",
                "agents/a/agentAliases/b/sessions/c/text",
                BedrockService::AgentRuntime,
                "https://bedrock-agent-runtime.us-east-1.amazonaws.com/agents/a/agentAliases/b/sessions/c/text",
            ),
            (
                "",
                "knowledgebases/kb/retrieve",
                BedrockService::AgentRuntime,
                "https://bedrock-agent-runtime.us-east-1.amazonaws.com/knowledgebases/kb/retrieve",
            ),
            (
                "",
                "guardrail/g/version/1/apply",
                BedrockService::Runtime,
                "https://bedrock-runtime.us-east-1.amazonaws.com/guardrail/g/version/1/apply",
            ),
        ];

        for (model_id, operation, expected_service, expected_url) in cases {
            let target = request_target("us-east-1", model_id, operation)
                .unwrap_or_else(|error| panic!("target should build: {error}"));
            assert_eq!(target.service, expected_service);
            assert_eq!(target.url, expected_url);
        }
    }

    #[test]
    fn unknown_or_incomplete_operations_fail_closed() {
        let error = request_target("us-east-1", "model", "custom-operation")
            .err()
            .unwrap_or_else(|| panic!("unsupported operation must fail before request building"));
        assert!(matches!(error, ProviderError::InvalidRequest { .. }));
        assert!(request_target("us-east-1", "", "invoke").is_err());
        assert!(request_target("us-east-1", "", "https://example.com/path").is_err());
        assert!(request_target("us-east-1", "", "guardrail/g/version//apply").is_err());
    }

    #[test]
    fn model_and_service_arn_identifiers_are_single_segments() {
        let model_arn = "arn:aws:bedrock:us-east-1:123:inference-profile/us.model:0";
        let model = request_target("us-east-1", model_arn, "invoke")
            .unwrap_or_else(|error| panic!("model ARN target should build: {error}"));
        assert!(model.url.contains("inference-profile%2Fus.model%3A0"));

        let batch_arn = "arn:aws:bedrock:us-east-1:123:model-invocation-job/job-1";
        let batch = request_target(
            "us-east-1",
            "",
            &format!("model-invocation-job/{batch_arn}/stop"),
        )
        .unwrap_or_else(|error| panic!("batch ARN target should build: {error}"));
        assert!(batch.url.contains("model-invocation-job/arn%3A"));
        assert!(batch.url.contains("%2Fjob-1/stop"));

        let guardrail_arn = "arn:aws:bedrock:us-east-1:123:guardrail/guard-1";
        let guardrail = request_target(
            "us-east-1",
            "",
            &format!("guardrail/{guardrail_arn}/version/DRAFT/apply"),
        )
        .unwrap_or_else(|error| panic!("guardrail ARN target should build: {error}"));
        assert!(guardrail.url.contains("guardrail/arn%3A"));
        assert!(guardrail.url.contains("%2Fguard-1/version/DRAFT/apply"));
    }
}
